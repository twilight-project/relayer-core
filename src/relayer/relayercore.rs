use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::*;
use std::sync::{Arc, Mutex};
use stopwatch::Stopwatch;
lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_NORMAL_ORDER")));
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_URGENT_ORDER")));
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER")));
}
pub fn client_cmd_receiver() {
    if *KAFKA_STATUS == "Enabled" {
        match receive_from_kafka_queue(
            String::from("CLIENT-REQUEST"),
            String::from("client_cmd_receiver3"),
        ) {
            Ok(rpc_cmd_receiver) => {
                let mut i = 0;
                let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);
                loop {
                    i += 1;
                    let rpc_client_cmd_request = rpc_cmd_receiver1.lock().unwrap().recv().unwrap();
                    core_event_handler(rpc_client_cmd_request);
                }
            }
            Err(arg) => {
                println!("error in client_cmd_receiver: {:#?}", arg);
            }
        }
    }
}

pub fn core_event_handler(command: RpcCommand) {
    let command_clone = command.clone();
    match command {
        RpcCommand::CreateTraderOrder(rpc_request, metadata) => {
            let buffer = THREADPOOL_NORMAL_ORDER.lock().unwrap();
            buffer.execute(move || {
                let (orderdata, status) = TraderOrder::new_order(rpc_request.clone());
                let order_state = orderdata.orderinsert_localdb(status);
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let completed_order = trader_order_db.add(order_state, command_clone);
                drop(trader_order_db);
            });
            drop(buffer);
        }
        // RpcCommand::ExecuteTraderOrder(rpc_request, metadata) => {
        //     let buffer = THREADPOOL_URGENT_ORDER.lock().unwrap();
        //     buffer.execute(move || {
        //         let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
        //         let order_data = trader_order_db.get(rpc_request.uuid);
        //         drop(trader_order_db);
        //         let execution_price = rpc_request.execution_price.clone();
        //         match order_data {
        //             Ok(order) => {
        //                 match order.order_status {
        //                     OrderStatus::FILLED => {
        //                         let current_price = get_localdb("CurrentPrice");
        //                         if rpc_request.order_type == OrderType::MARKET {
        //                             let ordertx_caluculated = order.calculatepayment();
        //                         } else {
        //                             match order.position_type {
        //                                 PositionType::LONG => {
        //                                     if execution_price <= current_price {
        //                                         let order_caluculated = order.calculatepayment();
        //                                     } else {
        //                                         let order_caluculated = order
        //                                             .set_execution_price_for_limit_order(execution_price);
        //                                     }
        //                                 }
        //                                 PositionType::SHORT => {
        //                                     if execution_price >= current_price {
        //                                         let order_caluculated = order.calculatepayment();
        //                                     } else {
        //                                         let order_caluculated = order
        //                                             .set_execution_price_for_limit_order(execution_price);
        //                                     }
        //                                 }
        //                             }
        //                         }
        //                     }
        //                     _ => {
        //                         println!("order not found !!");
        //                     }
        //                 },
        //             }
        //             Err(arg) => {
        //                 println!("Error found:{:#?}", arg);
        //             }
        //         }
        //     });
        //     drop(buffer);
        // }
        RpcCommand::CreateLendOrder(rpc_request, metadata) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                println!("LendOrder data: {:#?}", rpc_request);
            });
            drop(buffer);
        }
        _ => {}
    }
}
