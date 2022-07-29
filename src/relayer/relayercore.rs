use crate::config::*; //KAFKA_STATUS
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
// use crate::kafkalib::kafkacmd::Message;
// use crate::relayer::RpcCommand;
// use crate::relayer::{ThreadPool, TraderOrder};
use crate::relayer::*;
use std::sync::{Arc, Mutex};
// use std::sync::Arc;
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
    match command {
        RpcCommand::CreateTraderOrder(rpc_command, metadata) => {
            let buffer = THREADPOOL_NORMAL_ORDER.lock().unwrap();
            buffer.execute(move || {
                let order = TraderOrder::new(
                    &rpc_command.account_id,
                    rpc_command.position_type,
                    rpc_command.order_type,
                    rpc_command.leverage,
                    rpc_command.initial_margin,
                    rpc_command.available_margin,
                    rpc_command.order_status,
                    rpc_command.entryprice,
                    rpc_command.execution_price,
                );
                let order = order.orderinsert(true);

                println!("order data: {:#?}", order);
                println!("meta data: {:#?}", metadata);
            });
        }
        _ => {}
    }
}
