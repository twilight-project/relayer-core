use crate::config::LIMITSTATUS;
use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::redislib::redis_db;
use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use std::thread;
// use stopwatch::Stopwatch;

pub fn check_pending_limit_order_on_price_ticker_update(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    // let sw1 = Stopwatch::start_new();

    // let current_price = redis_db::get("CurrentPrice").parse::<f64>().unwrap();

    let orderid_list_short = redis_db::zrangegetpendinglimitorderforshort(current_price);

    let orderid_list_long = redis_db::zrangegetpendinglimitorderforlong(current_price);

    for orderid in orderid_list_short {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
        update_limit_pendingorder(ordertx);
    }
    for orderid in orderid_list_long {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
        update_limit_pendingorder(ordertx);
    }

    drop(limit_lock);
    // println!("mutex took {:#?}", sw1.elapsed());
}
// error issues
pub fn update_limit_pendingorder(ordertx: TraderOrder) {
    let rt = ordertx.clone();

    match rt.position_type {
        PositionType::LONG => {
            redis_db::zdel(
                &"TraderOrder_LimitOrder_Pending_FOR_Long",
                &rt.uuid.to_string(),
            );
        }
        PositionType::SHORT => {
            redis_db::zdel(
                &"TraderOrder_LimitOrder_Pending_FOR_Short",
                &rt.uuid.to_string(),
            );
        }
    }

    let mut pending_order: TraderOrder = TraderOrder::new(
        &rt.account_id,
        rt.position_type,
        rt.order_type,
        rt.leverage,
        rt.initial_margin,
        rt.available_margin,
        OrderStatus::PENDING,
        rt.entryprice,
        rt.execution_price,
    );
    pending_order.uuid = rt.uuid;
    pending_order.newtraderorderinsert();

    let query = format!(
        "UPDATE public.pendinglimittraderorder
    SET order_status='{:#?}'
    WHERE uuid='{}';",
        OrderStatus::FILLED,
        &rt.uuid
    );
    thread::spawn(move || {
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        client.execute(&query, &[]).unwrap();
    });
}

pub fn getsetlatestprice() {
    let currentprice = redis_db::get("btc:price");
    let old_price = redis_db::get("CurrentPrice");
    if currentprice == old_price {
        // println!("Price update: same price");
    } else {
        redis_db::set("CurrentPrice", &currentprice);
        check_pending_limit_order_on_price_ticker_update(redis_db::get_type_f64("CurrentPrice"));
        // println!("Price update: not same price");
    }
}

// pub fn runloop_price_ticker() -> thread::JoinHandle<()> {
//     thread::spawn(move || loop {
//         getsetlatestprice();
//     })
// }
