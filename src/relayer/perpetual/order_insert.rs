use crate::config::*;
use crate::redislib::redis_db;
use crate::redislib::redisdb_orderinsert::*;
use crate::relayer::*;

pub fn orderinsert(order: TraderOrder, order_entry_status: bool) -> TraderOrder {
    let mut ordertx = order.clone();
    let ordertx_return = ordertx.clone();
    let threadpool_max_order_insert_pool = THREADPOOL_MAX_ORDER_INSERT.lock().unwrap();
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

            if ordertx.order_type == OrderType::MARKET {
                // update order json array and entry sequence in redisdb
                let (lend_nonce, entrysequence): (u128, u128) = orderinsert_pipeline();
                // update latest nonce in order transaction
                ordertx.entry_nonce = lend_nonce;
                ordertx.entry_sequence = entrysequence;
            }

            //insert data into redis for sorting
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
                    orderinsert_pipeline_pending(
                        ordertx.clone(),
                        String::from("TraderOrder_LimitOrder_Pending_FOR_Long"),
                    );
                }
                PositionType::SHORT => {
                    orderinsert_pipeline_pending(
                        ordertx.clone(),
                        String::from("TraderOrder_LimitOrder_Pending_FOR_Short"),
                    );
                }
            }
            // redis_db::set(&ordertx.uuid.to_string(), &ordertx.serialize());
            pending_trader_order_insert_sql_query(ordertx);
        }
    });
    drop(threadpool_max_order_insert_pool);
    ordertx_return
}
