use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::redislib::redis_db;
use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use std::thread;

pub fn check_pending_limit_order_on_price_ticker_update() {
    let current_price = redis_db::get("CurrentPrice").parse::<f64>().unwrap();
    let orderid_list_short = redis_db::zrangegetpendinglimitorderforshort(current_price);
    let orderid_list_long = redis_db::zrangegetpendinglimitorderforlong(current_price);
    for orderid in orderid_list_short {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, ordertx.order_status);
        update_limit_pendingorder(ordertx);
    }
    for orderid in orderid_list_long {
        let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
        let state = println!("order of {} is {:#?}", ordertx.uuid, ordertx.order_status);
        update_limit_pendingorder(ordertx);
    }
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
        &rt.order_status, &rt.uuid
    );
    thread::spawn(move || {
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        client.execute(&query, &[]).unwrap();
    });

    // thread::spawn(move || {
    //     // trader order saved in redis, orderid as key
    //     redis_db::set(&rt.uuid.to_string(), &rt.serialize());

    //     redis_db::zadd(
    //         &"TraderOrder",
    //         &rt.uuid.to_string(),      //value
    //         &rt.timestamp.to_string(), //score
    //     );

    //     // update pool size when new order get inserted

    //     match rt.position_type {
    //         PositionType::LONG => {
    //             redis_db::incrbyfloat(&"TotalLongPositionSize", &rt.positionsize.to_string());
    //             // trader order set by liquidation_price for long
    //             redis_db::zadd(
    //                 &"TraderOrderbyLiquidationPriceFORLong",
    //                 &rt.uuid.to_string(),
    //                 &rt.liquidation_price.to_string(),
    //             );
    //         }
    //         PositionType::SHORT => {
    //             redis_db::incrbyfloat(&"TotalShortPositionSize", &rt.positionsize.to_string());
    //             // trader order set by liquidation_price for short
    //             redis_db::zadd(
    //                 &"TraderOrderbyLiquidationPriceFORShort",
    //                 &rt.uuid.to_string(),
    //                 &rt.liquidation_price.to_string(),
    //             );
    //         }
    //     }

    //     redis_db::incrbyfloat(&"TotalPoolPositionSize", &rt.positionsize.to_string());

    //     match rt.order_type {
    //         OrderType::LIMIT => match rt.position_type {
    //             PositionType::LONG => {
    //                 redis_db::zadd(
    //                     &"TraderOrderbyLONGLimit",
    //                     &rt.uuid.to_string(),
    //                     &rt.execution_price.to_string(),
    //                 );
    //             }
    //             PositionType::SHORT => {
    //                 redis_db::zadd(
    //                     &"TraderOrderbySHORTLimit",
    //                     &rt.uuid.to_string(),
    //                     &rt.execution_price.to_string(),
    //                 );
    //             }
    //         },
    //         _ => {}
    //     }
    // });
}
