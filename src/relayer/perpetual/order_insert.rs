use crate::config::*;
use crate::redislib::redis_db;
use crate::redislib::redisdb_orderinsert::{orderinsert_pipeline, orderinsert_pipeline_second};
use crate::relayer::*;

pub fn orderinsert(order: TraderOrder) -> TraderOrder {
    let mut ordertx = order.clone();
    let mut order_entry_status: bool = false;
    let current_price = get_localdb("CurrentPrice");

    match ordertx.order_type {
        OrderType::LIMIT => match ordertx.position_type {
            PositionType::LONG => {
                if ordertx.entryprice >= current_price {
                    order_entry_status = true;
                    ordertx.order_status = OrderStatus::FILLED;
                    ordertx.entryprice = current_price;
                } else {
                    ordertx.order_status = OrderStatus::PENDING;
                }
            }
            PositionType::SHORT => {
                if ordertx.entryprice <= current_price {
                    order_entry_status = true;
                    ordertx.order_status = OrderStatus::FILLED;
                    ordertx.entryprice = current_price;
                } else {
                    ordertx.order_status = OrderStatus::PENDING;
                }
            }
        },
        OrderType::MARKET => {
            ordertx.order_status = OrderStatus::FILLED;
            order_entry_status = true;
        }
        _ => {}
    }

    let threadpool_max_order_insert_pool = THREADPOOL_MAX_ORDER_INSERT.lock().unwrap();
    let ordertx_return = ordertx.clone();
    threadpool_max_order_insert_pool.execute(move || {
        if order_entry_status {
            let mut cmd_array: Vec<String> = Vec::new();

            // position type wise TotalLongPositionSize and liquidation sorting
            match ordertx.position_type {
                PositionType::LONG => {
                    cmd_array.push(String::from("TotalLongPositionSize"));
                    cmd_array.push(String::from("TraderOrderbyLiquidationPriceFORLong"));
                }
                PositionType::SHORT => {
                    cmd_array.push(String::from("TotalShortPositionSize"));
                    cmd_array.push(String::from("TraderOrderbyLiquidationPriceFORShort"));
                }
            }
            // total position size for funding rate
            cmd_array.push(String::from("TotalPoolPositionSize"));

            //insert data into redis for sorting
            let (lend_nonce, entrysequence): (u128, u128) = orderinsert_pipeline(ordertx.clone());

            // update latest nonce in order transaction
            ordertx.entry_nonce = lend_nonce;
            ordertx.entry_sequence = entrysequence;

            // update order json array and entry sequence in redisdb
            orderinsert_pipeline_second(ordertx.clone(), cmd_array);
            // update order data in postgreSQL
            new_trader_order_insert_sql_query(ordertx.clone());

            // undate recent order table and add candle data
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
        } else {
            // trader order set by timestamp
            match ordertx.position_type {
                PositionType::LONG => {
                    redis_db::zadd(
                        &"TraderOrder_LimitOrder_Pending_FOR_Long",
                        &ordertx.uuid.to_string(),       //value
                        &ordertx.entryprice.to_string(), //score
                    );
                }
                PositionType::SHORT => {
                    redis_db::zadd(
                        &"TraderOrder_LimitOrder_Pending_FOR_Short",
                        &ordertx.uuid.to_string(),       //value
                        &ordertx.entryprice.to_string(), //score
                    );
                }
            }
            redis_db::set(&ordertx.uuid.to_string(), &ordertx.serialize());
            pending_trader_order_insert_sql_query(ordertx);
        }
    });
    drop(threadpool_max_order_insert_pool);
    ordertx_return
}
