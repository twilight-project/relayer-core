#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod db;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;

use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use config::local_serial_core;
use config::*;
use db::*;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::*;
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
use uuid::Uuid;
#[macro_use]
extern crate lazy_static;
use questdb::questdb::send_candledata_in_questdb;
use std::sync::{mpsc, Arc, Mutex};

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // kafkalib::kafka_topic::kafka_new_topic("CLIENT-REQUEST");
    // kafkalib::kafka_topic::kafka_new_topic("TraderOrderEventLog1");
    // kafkalib::kafka_topic::kafka_new_topic("LendOrderEventLog1");
    // kafkalib::kafka_topic::kafka_new_topic("LendPoolEventLog1");
    dotenv::dotenv().expect("Failed loading dotenv");
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    let load_trader_data = TRADER_ORDER_DB.lock().unwrap();
    drop(load_trader_data);
    let load_lend_data = LEND_ORDER_DB.lock().unwrap();
    drop(load_lend_data);
    let load_pool_data = LEND_POOL_DB.lock().unwrap();
    drop(load_pool_data);
    ordertest::initprice();
    thread::spawn(move || {
        client_cmd_receiver();
    });
    // thread::spawn(move || {
    //     let recever = Event::receive_event_from_kafka_queue(
    //         String::from("TraderOrderEventLog1"),
    //         String::from("client_event_receiver"),
    //     )
    //     .unwrap();
    //     let recever1 = recever.lock().unwrap();
    //     loop {
    //         let data = recever1.recv().unwrap();
    //         println!("{:#?}", data);
    //     }
    // });
    init_psql();
    ordertest::generatelendorder();
    thread::sleep(time::Duration::from_millis(100));
    start_cronjobs();
    // thread::sleep(time::Duration::from_millis(10000));
    // updatefundingrate(1.0);
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}

// use std::collections::HashSet;
use std::sync::RwLock;

// many reader locks can be held at once
// fn dmain() {
//     // // let lock = RwLock::new(5);
//     // let lock = Arc::new(RwLock::new(5));
//     // let lock_2 = Arc::clone(&lock);
//     // thread::spawn(move || {
//     //     thread::sleep(time::Duration::from_millis(600));
//     //     let mut w = lock.write().unwrap();
//     //     *w += 1;
//     //     println!("read w:{}", *w);
//     // });
//     // thread::spawn(move || {
//     //     thread::sleep(time::Duration::from_millis(500));
//     //     let r1 = lock_2.read().unwrap();
//     //     println!("delay 500 read");
//     //     println!("read r1:{}", *r1);
//     //     drop(r1);
//     //     thread::sleep(time::Duration::from_millis(1000));
//     //     let r2 = lock_2.read().unwrap();
//     //     println!("delay 1000 read");
//     //     println!("read r2:{}", *r2);
//     // });

//     let traderorder=TraderOrder::deserialize(&"{\"uuid\":\"22f940be-79ca-4365-8ec2-d93f3e8d6233\",\"account_id\":\"test order\",\"position_type\":\"LONG\",\"order_status\":\"FILLED\",\"order_type\":\"MARKET\",\"entryprice\":20000.0,\"execution_price\":20000.0,\"positionsize\":200000.0,\"leverage\":10.0,\"initial_margin\":1.0,\"available_margin\":1.0,\"timestamp\":{\"secs_since_epoch\":1657919055,\"nanos_since_epoch\":663796000},\"bankruptcy_price\":18181.81818181818,\"bankruptcy_value\":11.000000000000002,\"maintenance_margin\":4400.040000000001,\"liquidation_price\":45.568051327853006,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":3,\"exit_nonce\":0,\"entry_sequence\":1}".to_string());

//     let orderlog = OrderLog::new(traderorder.clone());
//     match OrderLog::insert_new_order_log(orderlog, traderorder.uuid.clone().to_string()) {
//         Ok(_) => {
//             println!("OrderLog successfully inserted");
//         }
//         Err(arg) => {
//             println!("OrderLog Error: {:#?}", arg);
//         }
//     }
//     let traderorder_clone = traderorder.clone();
//     thread::spawn(move || {
//         thread::sleep(time::Duration::from_millis(1000));

//         let ordertrader = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
//         // let ordertrader1 = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
//         // let ordertrader2 = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
//         // println!("order 11 :{:#?}", ordertrader2);
//         let mut orx = ordertrader.write().unwrap();
//         // orx.orderlog.push(Rcmd::new(OrderCommand::NewOrder));
//         orx.orderdata.leverage = 12.0;
//         println!("order:{:#?}", orx);
//     });

//     thread::spawn(move || {
//         // thread::sleep(time::Duration::from_millis(1000));
//         // let ordertrader = OrderLog::get_order(&traderorder.uuid.clone().to_string());
//         // let mut orx = ordertrader.read().unwrap();
//         // orx.orderdata.clone().orderinsert(true);
//         // orx.orderlog.push(Rcmd::new());
//         // orx.orderdata.leverage = 12.0;
//         // println!("order write :{:#?}", orx);
//         let order_read_only = OrderLog::get_order_readonly(&traderorder.uuid.clone().to_string());
//         match order_read_only {
//             Ok(value) => {
//                 println!("order get only :{:#?}", value);
//             }
//             Err(arg) => {
//                 println!("Error : {:#?}", arg);
//             }
//         }
//     });

//     thread::sleep(time::Duration::from_millis(10000));
// }

// fn main() {
//     let traderorder=TraderOrder::deserialize(&"{\"uuid\":\"22f940be-79ca-4365-8ec2-d93f3e8d6233\",\"account_id\":\"test order\",\"position_type\":\"LONG\",\"order_status\":\"FILLED\",\"order_type\":\"MARKET\",\"entryprice\":20000.0,\"execution_price\":20000.0,\"positionsize\":200000.0,\"leverage\":10.0,\"initial_margin\":1.0,\"available_margin\":1.0,\"timestamp\":{\"secs_since_epoch\":1657919055,\"nanos_since_epoch\":663796000},\"bankruptcy_price\":18181.81818181818,\"bankruptcy_value\":11.000000000000002,\"maintenance_margin\":4400.040000000001,\"liquidation_price\":45.568051327853006,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":3,\"exit_nonce\":0,\"entry_sequence\":1}".to_string());

//     // let mut order_db: OrderDB<TraderOrder> = LocalDB::new();
//     // order_db
//     //     .ordertable
//     //     .insert(traderorder.uuid, Arc::new(RwLock::new(traderorder)));
//     // println!("{:#?}", order_db);
// }
