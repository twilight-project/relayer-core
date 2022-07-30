// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
// mod aeronlib;
// mod aeronlibmpsc;
mod config;
mod db;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;

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
// use skiplist::OrderedSkipList;
use std::sync::{mpsc, Arc, Mutex};

use std::collections::HashSet;
fn main() {
    let xs = [1, 2, 3, 4, 5, 6];
    let xss = [1, 2];
    let mut set: HashSet<isize> = xs.iter().cloned().collect();
    set.retain(|&k| xss.contains(&k) == false);
    println!("{:#?}", set);
}

// fn main() {
//     // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
//     // kafkalib::kafka_topic::kafka_new_topic("CLIENT-REQUEST");
//     dotenv::dotenv().expect("Failed loading dotenv");
//     // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
//     ordertest::initprice();
//     // thread::spawn(move || {
//     //     client_cmd_receiver();
//     // });
//     // init_psql();
//     // ordertest::generatelendorder();
//     // thread::sleep(time::Duration::from_millis(100));
//     // start_cronjobs();
//     // thread::sleep(time::Duration::from_millis(10000));
//     // updatefundingrate(1.0);
//     let prices = 150;
//     let mut trader_lp_long = TRADER_LP_LONG.lock().unwrap();
//     trader_lp_long.add(
//         "b0747cfc-a94b-4ae4-8e17-b166701545a1".parse().unwrap(),
//         prices,
//     );
//     let prices = 200;
//     trader_lp_long.add(
//         "b0747cfc-a94b-4ae4-8e17-b166701545a2".parse().unwrap(),
//         prices,
//     );
//     let prices = 14;
//     trader_lp_long.add(
//         "b0747cfc-a94b-4ae4-8e17-b166701545a3".parse().unwrap(),
//         prices,
//     );
//     drop(trader_lp_long);
//     let sw = Stopwatch::start_new();
//     let threadpool = ThreadPool::new(30, String::from("testt"));
//     for i in 0..400000 {
//         // let f1 = format!("b0747cfc-a94b-4ae4-8e17-b1667015{}", i);
//         // let f2 = format!("{}.0", i, i);
//         // let uuid: Uuid = serde_json::from_str(&f1).unwrap();
//         // let price: f64 = serde_json::from_str(&f2).unwrap();
//         let price: i64 = i;
//         threadpool.execute(move || {
//             let mut trader_lp_long = TRADER_LP_LONG.lock().unwrap();
//             trader_lp_long.add(Uuid::new_v4(), price);
//             drop(trader_lp_long)
//         });
//     }
//     drop(threadpool);
//     let mut trader_lp_long = TRADER_LP_LONG.lock().unwrap();
//     trader_lp_long.sort();
//     // let sw3 = Stopwatch::start_new();

//     // let mut data = TRADER_LP_LONG.lock().unwrap();
//     // let key_of_two = data
//     //     .sorted_order
//     //     .iter()
//     //     .position(|&(x, _y)| x == "b0747cfc-a94b-4ae4-8e17-b166701545a2".parse().unwrap());
//     // // let key_of_two = data
//     // //     .sorted_order
//     // //     .contains(&("b0747cfc-a94b-4ae4-8e17-b166701545a2".parse().unwrap()));
//     // // let key_of_two = data
//     // //     .sorted_order
//     // //     .iter()
//     // //     .any(|&(x, _y)| x == "b0747cfc-a94b-4ae4-8e17-b166701545a2".parse().unwrap());
//     // let time_contain = sw3.elapsed();

//     // let array: Vec<(Uuid, i64)>;
//     // let data_clone = data.clone();
//     // let len = data.clone().len;
//     // if key_of_two.is_some() {
//     //     array = data
//     //         .sorted_order
//     //         .drain((key_of_two.unwrap())..(key_of_two.unwrap() + 1))
//     //         .collect();
//     // } else {
//     //     array = Vec::new();
//     // }
//     // drop(data);
//     // println!("Complete data:{:?}\n\n", trader_lp_long.read());
//     let sw2 = Stopwatch::start_new();
//     match trader_lp_long.remove("b0747cfc-a94b-4ae4-8e17-b166701545a2".parse().unwrap()) {
//         Ok(_x) => {}
//         Err(arg) => {
//             println!("ErrorCustom:{:#?}", arg);
//         }
//     }
//     let time_2 = sw2.elapsed();
//     let orderdata = trader_lp_long.read();
//     // println!("removed data:{:?}\n\n", orderdata.clone());

//     println!("\nmix:{:#?}", orderdata.sorted_order[0].1);
//     println!("\nmax:{:#?}", orderdata.sorted_order[orderdata.len - 1].1);
//     println!("\ntime_remove:{:#?}", time_2);
//     let time_taken = sw.elapsed();
//     println!("time : {:#?}", time_taken);
//     loop {
//         thread::sleep(time::Duration::from_millis(100000000));
//     }
// }

// use std::collections::HashSet;
use std::sync::RwLock;

// many reader locks can be held at once
fn dmain() {
    // // let lock = RwLock::new(5);
    // let lock = Arc::new(RwLock::new(5));
    // let lock_2 = Arc::clone(&lock);
    // thread::spawn(move || {
    //     thread::sleep(time::Duration::from_millis(600));
    //     let mut w = lock.write().unwrap();
    //     *w += 1;
    //     println!("read w:{}", *w);
    // });
    // thread::spawn(move || {
    //     thread::sleep(time::Duration::from_millis(500));
    //     let r1 = lock_2.read().unwrap();
    //     println!("delay 500 read");
    //     println!("read r1:{}", *r1);
    //     drop(r1);
    //     thread::sleep(time::Duration::from_millis(1000));
    //     let r2 = lock_2.read().unwrap();
    //     println!("delay 1000 read");
    //     println!("read r2:{}", *r2);
    // });

    let traderorder=TraderOrder::deserialize(&"{\"uuid\":\"22f940be-79ca-4365-8ec2-d93f3e8d6233\",\"account_id\":\"test order\",\"position_type\":\"LONG\",\"order_status\":\"FILLED\",\"order_type\":\"MARKET\",\"entryprice\":20000.0,\"execution_price\":20000.0,\"positionsize\":200000.0,\"leverage\":10.0,\"initial_margin\":1.0,\"available_margin\":1.0,\"timestamp\":{\"secs_since_epoch\":1657919055,\"nanos_since_epoch\":663796000},\"bankruptcy_price\":18181.81818181818,\"bankruptcy_value\":11.000000000000002,\"maintenance_margin\":4400.040000000001,\"liquidation_price\":45.568051327853006,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":3,\"exit_nonce\":0,\"entry_sequence\":1}".to_string());

    let orderlog = OrderLog::new(traderorder.clone());
    match OrderLog::insert_new_order_log(orderlog, traderorder.uuid.clone().to_string()) {
        Ok(_) => {
            println!("OrderLog successfully inserted");
        }
        Err(arg) => {
            println!("OrderLog Error: {:#?}", arg);
        }
    }
    let traderorder_clone = traderorder.clone();
    thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(1000));

        let ordertrader = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
        // let ordertrader1 = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
        // let ordertrader2 = OrderLog::get_order(&traderorder_clone.uuid.clone().to_string());
        // println!("order 11 :{:#?}", ordertrader2);
        let mut orx = ordertrader.write().unwrap();
        // orx.orderlog.push(Rcmd::new(OrderCommand::NewOrder));
        orx.orderdata.leverage = 12.0;
        println!("order:{:#?}", orx);
    });

    thread::spawn(move || {
        // thread::sleep(time::Duration::from_millis(1000));
        // let ordertrader = OrderLog::get_order(&traderorder.uuid.clone().to_string());
        // let mut orx = ordertrader.read().unwrap();
        // orx.orderdata.clone().orderinsert(true);
        // orx.orderlog.push(Rcmd::new());
        // orx.orderdata.leverage = 12.0;
        // println!("order write :{:#?}", orx);
        let order_read_only = OrderLog::get_order_readonly(&traderorder.uuid.clone().to_string());
        match order_read_only {
            Ok(value) => {
                println!("order get only :{:#?}", value);
            }
            Err(arg) => {
                println!("Error : {:#?}", arg);
            }
        }
    });

    thread::sleep(time::Duration::from_millis(10000));
}
