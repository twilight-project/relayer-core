use crate::config::LIMITSTATUS;
use crate::config::LIQUIDATIONORDERSTATUS;
use crate::config::LIQUIDATIONTICKERSTATUS;
use crate::config::SETTLEMENTLIMITSTATUS;
use crate::config::{POSTGRESQL_POOL_CONNECTION, THREADPOOL, THREADPOOL_PSQL_SEQ_QUEUE};
use crate::redislib::redis_db;
use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use crate::relayer::utils::*;
use crate::relayer::ThreadPool;
use crate::relayer::{update_recent_orders, CloseTrade, Side};
use std::thread;
// use stopwatch::Stopwatch;

pub fn check_pending_limit_order_on_price_ticker_update(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    // let sw1 = Stopwatch::start_new();

    // let current_price = get_localdb("CurrentPrice");
    let orderid_list_short = redis_db::zrangegetpendinglimitorderforshort(current_price);

    let orderid_list_long = redis_db::zrangegetpendinglimitorderforlong(current_price);
    if orderid_list_short.len() > 0 {
        redis_db::zdel_bulk(
            "TraderOrder_LimitOrder_Pending_FOR_Short",
            orderid_list_short.clone(),
        );
    }
    if orderid_list_long.len() > 0 {
        redis_db::zdel_bulk(
            "TraderOrder_LimitOrder_Pending_FOR_Long",
            orderid_list_long.clone(),
        );
    }
    let total_order_count = orderid_list_short.len() + orderid_list_long.len();
    let mut thread_count: usize = (total_order_count) / 10;
    if thread_count > 5 {
        thread_count = 5;
    } else if thread_count == 0 {
        thread_count = 1;
    }
    if total_order_count > 0 {
        let entry_nonce = redis_db::get_nonce_u128();
        //get all order by mget command and then process all order
        let local_threadpool: ThreadPool = ThreadPool::new(thread_count);
        for orderid in orderid_list_short {
            local_threadpool.execute(move || {
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = redis_db::incr_entry_sequence_by_one_trader_order();
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
        for orderid in orderid_list_long {
            local_threadpool.execute(move || {
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = redis_db::incr_entry_sequence_by_one_trader_order();
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
    }
    // std::thread::sleep(std::time::Duration::from_millis(5000));
    drop(limit_lock);
    // println!("mutex took {:#?}", sw1.elapsed());
}
// error issues
pub fn update_limit_pendingorder(
    ordertx: TraderOrder,
    current_price: f64,
    entry_nonce: u128,
    entry_sequence: u128,
) {
    let ordertx_clone = ordertx.clone();

    let pending_order: TraderOrder = TraderOrder::pending(
        &ordertx_clone.account_id,
        ordertx_clone.position_type,
        ordertx_clone.order_type,
        ordertx_clone.leverage,
        ordertx_clone.initial_margin,
        ordertx_clone.available_margin,
        OrderStatus::FILLED,
        current_price,
        ordertx_clone.execution_price,
        ordertx_clone.uuid,
        entry_nonce,    //need to update
        entry_sequence, //need to update
    );
    let ordertx = pending_order.pending_limit_traderorderinsert();
    let ordertx_clone = ordertx.clone();
    let pool = THREADPOOL.lock().unwrap();
    pool.execute(move || {
        let query = format!(
            "UPDATE public.pendinglimittraderorder
        SET order_status='{:#?}'
        WHERE uuid='{}';",
            OrderStatus::FILLED,
            &ordertx_clone.uuid
        );
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        client.execute(&query, &[]).unwrap();
    });
    let side = match ordertx.position_type {
        PositionType::SHORT => Side::SELL,
        PositionType::LONG => Side::BUY,
    };
    update_recent_orders(CloseTrade {
        side: side,
        positionsize: ordertx.positionsize,
        price: ordertx.entryprice,
        timestamp: std::time::SystemTime::now(),
    });
}

pub fn getsetlatestprice() {
    // btc:price is websocket price getting updated via websocket external feed
    let rev_data: Vec<f64> = redis_db::mget_f64(vec!["CurrentPrice", "btc:price"]);
    let (old_price, currentprice) = (rev_data[0], rev_data[1]);
    // let currentprice = redis_db::get("btc:price");
    // let old_price = redis_db::get("CurrentPrice");
    if currentprice == old_price {
        // println!("Price update: same price");
    } else {
        set_localdb("CurrentPrice", currentprice);
        redis_db::set("CurrentPrice", &currentprice.clone().to_string());
        thread::spawn(move || {
            check_pending_limit_order_on_price_ticker_update(currentprice.clone());
        });
        thread::spawn(move || {
            check_liquidating_orders_on_price_ticker_update(currentprice.clone());
        });
        thread::spawn(move || {
            check_settling_limit_order_on_price_ticker_update(currentprice.clone());
        });
        // println!("Price update: not same price");
        let pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
        pool.execute(move || {
            insert_current_price_psql(currentprice.clone());
        });
        drop(pool);
    }
}
// pub fn runloop_price_ticker() -> thread::JoinHandle<()> {
//     thread::spawn(move || loop {
//         getsetlatestprice();
//     })
// }
pub fn check_liquidating_orders_on_price_ticker_update(current_price: f64) {
    let liquidation_lock = LIQUIDATIONTICKERSTATUS.lock().unwrap();

    let orderid_list_short = redis_db::zrangegetliquidateorderforshort(current_price);

    let orderid_list_long = redis_db::zrangegetliquidateorderforlong(current_price);
    let total_order_count = orderid_list_short.len() + orderid_list_long.len();
    let mut thread_count: usize = (total_order_count) / 10;

    if total_order_count > 0 {
        if thread_count > 5 {
            thread_count = 5;
        } else if thread_count == 0 {
            thread_count = 1;
        }
        let local_threadpool: ThreadPool = ThreadPool::new(thread_count);
        for orderid in orderid_list_short {
            local_threadpool.execute(move || {
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
                liquidate_trader_order(ordertx, current_price);
            });
        }
        for orderid in orderid_list_long {
            local_threadpool.execute(move || {
                let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
                let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
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
        let local_threadpool: ThreadPool = ThreadPool::new(thread_count);

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
