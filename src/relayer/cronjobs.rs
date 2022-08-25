// use crate::aeronlib::types::init_aeron_queue;
// use crate::aeronlibmpsc::aeronqueue::rec_aeron_msg_direct;
// use crate::aeronlibmpsc::types::init_aeron_direct_queue;
// use crate::ordertest::generateorder;
use crate::pricefeederlib::price_feeder::receive_btc_price;
// use crate::redislib::redis_db;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
// use std::collections::HashMap;
// use std::time::SystemTime;
use std::{thread, time};

// use stopwatch::Stopwatch;

pub fn start_cronjobs() {
    // main thread for scheduler
    // thread::Builder::new()
    //     .name(String::from("cronjob scheduler"))
    //     .spawn(move || {
    //         let mut scheduler = Scheduler::with_tz(chrono::Utc);
    //         // scheduler
    //         //     .every(200000.seconds())
    //         //     .run(move || generateorder());

    //         // make backup of redis db in backup/redisdb folder every 5 sec //comments for local test
    //         // scheduler.every(500000.seconds()).run(move || {
    //         //     // scheduler.every(5.seconds()).run(move || {
    //         //     redis_db::save_redis_backup(format!(
    //         //         "aeron:backup/redisdb/dump_{}.rdb",
    //         //         SystemTime::now()
    //         //             .duration_since(SystemTime::UNIX_EPOCH)
    //         //             .unwrap()
    //         //             .as_millis()
    //         //     ))
    //         // });

    //         // funding update every 1 hour //comments for local test
    //         // scheduler.every(600.seconds()).run(move || {
    //         scheduler.every(1.hour()).run(move || {
    //             updatefundingrate(1.0);
    //         });

    //         let thread_handle = scheduler.watch_thread(time::Duration::from_millis(100));
    //         loop {
    //             thread::sleep(time::Duration::from_millis(100000000));
    //         }
    //     })
    //     .unwrap();

    // can't use scheduler because it allows minimum 1 second time to schedule any job
    // thread::Builder::new()
    //     .name(String::from("getsetlatestprice"))
    //     .spawn(move || loop {
    //         thread::sleep(time::Duration::from_millis(250));
    //         thread::spawn(move || {
    //             getsetlatestprice();
    //         });
    //     })
    //     .unwrap();

    // thread::spawn(move || loop {
    //     thread::sleep(time::Duration::from_millis(2000));
    //     update_candle_data();
    // });
    // thread::Builder::new()
    //     .name(String::from("get_latest_orderbook"))
    //     .spawn(move || loop {
    //         thread::sleep(time::Duration::from_millis(10000));
    //         thread::spawn(move || set_localdb_string("OrderBook", get_latest_orderbook()));
    //     })
    //     .unwrap();

    // thread::Builder::new()
    //     .name(String::from("json-RPC startserver"))
    //     .spawn(move || {
    //         startserver();
    //     })
    //     .unwrap();
    // thread::Builder::new()
    //     .name(String::from("json-RPC startserver_repl"))
    //     .spawn(move || {
    //         startserver_repl();
    //     })
    //     .unwrap();

    // thread::Builder::new()
    //     .name(String::from("BTC Binance Websocket Connection"))
    //     .spawn(move || {
    //         thread::sleep(time::Duration::from_millis(1000));
    //         receive_btc_price();
    //     })
    //     .unwrap();

    // QueueResolver::new(String::from("questdb_queue"));

    println!("Initialization done..................................");
}
