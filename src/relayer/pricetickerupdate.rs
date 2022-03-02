use crate::config::LIMITSTATUS;
use crate::config::LIQUIDATIONORDERSTATUS;
use crate::config::LIQUIDATIONTICKERSTATUS;
use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::redislib::redis_db;
use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use crate::relayer::utils::*;

use std::thread;
use stopwatch::Stopwatch;

pub fn check_pending_limit_order_on_price_ticker_update(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    let sw1 = Stopwatch::start_new();

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
    let entry_nonce = redis_db::get_nonce_u128();
    thread::spawn(move || {
        for orderid in orderid_list_short {
            let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
            let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
            let entry_sequence = redis_db::incr_entry_sequence_by_one_trader_order();
            thread::spawn(move || {
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
        for orderid in orderid_list_long {
            let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
            let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
            let entry_sequence = redis_db::incr_entry_sequence_by_one_trader_order();
            thread::spawn(move || {
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
    });

    drop(limit_lock);
    println!("mutex took {:#?}", sw1.elapsed());
}
// error issues
pub fn update_limit_pendingorder(
    ordertx: TraderOrder,
    current_price: f64,
    entry_nonce: u128,
    entry_sequence: u128,
) {
    let rt = ordertx.clone();

    let pending_order: TraderOrder = TraderOrder::pending(
        &rt.account_id,
        rt.position_type,
        rt.order_type,
        rt.leverage,
        rt.initial_margin,
        rt.available_margin,
        OrderStatus::FILLED,
        current_price,
        rt.execution_price,
        rt.uuid,
        entry_nonce,    //need to update
        entry_sequence, //need to update
    );
    pending_order.pending_limit_traderorderinsert();

    thread::spawn(move || {
        let query = format!(
            "UPDATE public.pendinglimittraderorder
        SET order_status='{:#?}'
        WHERE uuid='{}';",
            OrderStatus::FILLED,
            &rt.uuid
        );
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        client.execute(&query, &[]).unwrap();
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
        redis_db::set("CurrentPrice", &currentprice.to_string());
        set_localdb("CurrentPrice", currentprice);
        thread::spawn(move || {
            check_pending_limit_order_on_price_ticker_update(currentprice.clone());
        });
        thread::spawn(move || {
            check_liquidating_orders_on_price_ticker_update(currentprice.clone());
        });
        // println!("Price update: not same price");
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

    for orderid in orderid_list_short {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
        thread::spawn(move || {
            liquidate_trader_order(ordertx, current_price);
        });
    }
    for orderid in orderid_list_long {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::LIQUIDATE);
        thread::spawn(move || {
            liquidate_trader_order(ordertx, current_price);
        });
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
