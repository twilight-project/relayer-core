use crate::config::LIMITSTATUS;
use crate::config::LIQUIDATIONORDERSTATUS;
use crate::config::LIQUIDATIONTICKERSTATUS;
use crate::config::SETTLEMENTLIMITSTATUS;
use crate::config::*;
use crate::redislib::redis_db;
use crate::relayer::*;
use std::convert::{From, TryFrom};
use std::thread;
// use stopwatch::Stopwatch;

pub fn getsetlatestprice() {
    // btc:price is websocket price getting updated via websocket external feed
    // let rev_data: Vec<f64>;
    let (old_price, currentprice): (f64, f64);
    currentprice = redis_db::get_type_f64("btc:price");
    old_price = get_localdb("CurrentPrice");
    if currentprice == old_price {
        // println!("Price update: same price");
    } else {
        set_localdb("CurrentPrice", currentprice);
        redis_db::set("CurrentPrice", &currentprice.clone().to_string());
        let treadpool_pending_order = THREADPOOL_PRICE_CHECK_PENDING_ORDER.lock().unwrap();
        treadpool_pending_order.execute(move || {
            check_pending_limit_order_on_price_ticker_update(currentprice.clone());
        });
        let treadpool_liquidation_order = THREADPOOL_PRICE_CHECK_LIQUIDATION.lock().unwrap();
        treadpool_liquidation_order.execute(move || {
            check_liquidating_orders_on_price_ticker_update(currentprice.clone());
        });
        let treadpool_settling_order = THREADPOOL_PRICE_CHECK_SETTLE_PENDING.lock().unwrap();
        treadpool_settling_order.execute(move || {
            check_settling_limit_order_on_price_ticker_update(currentprice.clone());
        });
        // println!("Price update: not same price");
        let pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
        pool.execute(move || {
            insert_current_price_psql(currentprice.clone());
        });
        drop(treadpool_pending_order);
        drop(treadpool_liquidation_order);
        drop(treadpool_settling_order);
        drop(pool);
    }
}

pub fn check_pending_limit_order_on_price_ticker_update(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();

    let orderid_list_short = redis_db::zrangegetpendinglimitorderforshort(current_price);
    let orderid_list_long = redis_db::zrangegetpendinglimitorderforlong(current_price);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    if orderid_list_short_len > 0 {
        redis_db::zdel_bulk(
            "TraderOrder_LimitOrder_Pending_FOR_Short",
            orderid_list_short.clone(),
        );
    }
    if orderid_list_long_len > 0 {
        redis_db::zdel_bulk(
            "TraderOrder_LimitOrder_Pending_FOR_Long",
            orderid_list_long.clone(),
        );
    }
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    let mut thread_count: usize = (total_order_count) / 10;
    if thread_count > 10 {
        thread_count = 10;
    } else if thread_count == 0 {
        thread_count = 1;
    }

    if total_order_count > 0 {
        let entry_nonce = redis_db::get_nonce_u128();
        let mut ordertx_short: Vec<TraderOrder> = Vec::new();
        let mut ordertx_long: Vec<TraderOrder> = Vec::new();
        let mut starting_entry_sequence_short: u128 = 0;
        let mut starting_entry_sequence_long: u128 = 0;
        if orderid_list_short_len > 0 {
            ordertx_short = redis_db::mget_trader_order(orderid_list_short.clone()).unwrap();
            let bulk_entry_sequence_short =
                redis_db::incr_entry_sequence_bulk_trader_order(orderid_list_short_len);
            starting_entry_sequence_short =
                bulk_entry_sequence_short - u128::try_from(orderid_list_short_len).unwrap();
        }
        if orderid_list_long_len > 0 {
            ordertx_long = redis_db::mget_trader_order(orderid_list_long.clone()).unwrap();
            let bulk_entry_sequence_long =
                redis_db::incr_entry_sequence_bulk_trader_order(orderid_list_long_len);
            starting_entry_sequence_long =
                bulk_entry_sequence_long - u128::try_from(orderid_list_long_len).unwrap();
        }

        let local_threadpool: ThreadPool =
            ThreadPool::new(thread_count, String::from("limitorder_local_threadpool"));

        for ordertx in ordertx_short {
            starting_entry_sequence_short += 1;
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = starting_entry_sequence_short;
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }

        for ordertx in ordertx_long {
            starting_entry_sequence_long += 1;
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = starting_entry_sequence_long;
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
    }
    drop(limit_lock);
}

pub fn update_limit_pendingorder(
    ordertx: TraderOrder,
    current_price: f64,
    entry_nonce: u128,
    entry_sequence: u128,
) {
    //recalculate data for pending order with current entry price
    let pending_order: TraderOrder = TraderOrder::pending(
        &ordertx.account_id,
        ordertx.position_type,
        ordertx.order_type,
        ordertx.leverage,
        ordertx.initial_margin,
        ordertx.available_margin,
        OrderStatus::FILLED,
        current_price,
        ordertx.execution_price,
        ordertx.uuid,
        entry_nonce,
        entry_sequence,
    );
    // let ordertx = pending_order.pending_limit_traderorderinsert();
    let ordertx_new = orderinsert(pending_order, true);
    pending_trader_order_update_sql_query(ordertx_new.uuid.clone());
    let side = match ordertx_new.position_type {
        PositionType::SHORT => Side::SELL,
        PositionType::LONG => Side::BUY,
    };
    update_recent_orders(CloseTrade {
        side: side,
        positionsize: ordertx_new.positionsize,
        price: ordertx_new.entryprice,
        timestamp: std::time::SystemTime::now(),
    });
}

pub fn check_liquidating_orders_on_price_ticker_update(current_price: f64) {
    let liquidation_lock = LIQUIDATIONTICKERSTATUS.lock().unwrap();

    let orderid_list_short = redis_db::zrangegetliquidateorderforshort(current_price);

    let orderid_list_long = redis_db::zrangegetliquidateorderforlong(current_price);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    let mut thread_count: usize = (total_order_count) / 10;

    if total_order_count > 0 {
        if thread_count > 10 {
            thread_count = 10;
        } else if thread_count == 0 {
            thread_count = 1;
        }
        let local_threadpool: ThreadPool =
            ThreadPool::new(thread_count, String::from("liquidation_local_threadpool"));

        let entry_nonce = redis_db::get_nonce_u128();
        let mut ordertx_short: Vec<TraderOrder> = Vec::new();
        let mut ordertx_long: Vec<TraderOrder> = Vec::new();
        if orderid_list_short_len > 0 {
            ordertx_short = redis_db::mget_trader_order(orderid_list_short.clone()).unwrap();
        }
        if orderid_list_long_len > 0 {
            ordertx_long = redis_db::mget_trader_order(orderid_list_long.clone()).unwrap();
        }

        for ordertx in ordertx_short {
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
                liquidate_trader_order(ordertx, current_price);
            });
        }
        for ordertx in ordertx_long {
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
                liquidate_trader_order(ordertx, current_price);
            });
        }
    }
    drop(liquidation_lock);
}

pub fn liquidate_trader_order(order: TraderOrder, current_price: f64) {
    let order_lock = LIQUIDATIONORDERSTATUS.lock().unwrap();

    let mut ordertx = order.clone();
    ordertx = liquidateposition(ordertx, current_price);
    ordertx.order_status = OrderStatus::LIQUIDATE;
    ordertx.update_trader_order_table_into_db_on_funding_cycle(order.uuid.to_string(), true);

    drop(order_lock);
}

pub fn check_settling_limit_order_on_price_ticker_update(current_price: f64) {
    let limit_lock = SETTLEMENTLIMITSTATUS.lock().unwrap();
    // let sw1 = Stopwatch::start_new();

    // let current_price = get_localdb("CurrentPrice");

    let orderid_list_short = redis_db::zrangegetsettlinglimitorderforshort(current_price);
    // println!("short array:{:#?}", orderid_list_short);
    let orderid_list_long = redis_db::zrangegetsettlinglimitorderforlong(current_price);
    // println!("Long array:{:#?}", orderid_list_long);
    let total_order_count = orderid_list_short.len() + orderid_list_long.len();
    let mut thread_count: usize = (total_order_count) / 10;

    if total_order_count > 0 {
        if thread_count > 5 {
            thread_count = 5;
        } else if thread_count == 0 {
            thread_count = 1;
        }
        let local_threadpool: ThreadPool =
            ThreadPool::new(thread_count, String::from("local_threadpool"));

        if orderid_list_short.len() > 0 {
            let orderid_list_short_clone = orderid_list_short.clone();
            local_threadpool.execute(move || {
                redis_db::zdel_bulk(
                    "TraderOrder_Settelment_by_SHORT_Limit",
                    orderid_list_short_clone,
                );
            });
        }
        if orderid_list_long.len() > 0 {
            let orderid_list_long_clone = orderid_list_long.clone();
            local_threadpool.execute(move || {
                redis_db::zdel_bulk(
                    "TraderOrder_Settelment_by_LONG_Limit",
                    orderid_list_long_clone,
                );
            });
        }

        for orderid in orderid_list_short {
            local_threadpool.execute(move || {
                let current_price_clone = current_price.clone();
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                ordertx.calculatepayment_with_current_price(current_price_clone);
            });
        }
        for orderid in orderid_list_long {
            local_threadpool.execute(move || {
                let current_price_clone = current_price.clone();
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                ordertx.calculatepayment_with_current_price(current_price_clone);
            });
        }
    }

    drop(limit_lock);
    // println!("mutex took {:#?}", sw1.elapsed());
}

fn insert_current_price_psql(current_price: f64) {
    let query = format!("call insert_btcprice({});", current_price);
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
    client.execute(&query, &[]).unwrap();
    // drop(client);
}
