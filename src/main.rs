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
    ordertest::initprice();
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    let load_trader_data = TRADER_ORDER_DB.lock().unwrap();
    drop(load_trader_data);
    let load_lend_data = LEND_ORDER_DB.lock().unwrap();
    drop(load_lend_data);
    let load_pool_data = LEND_POOL_DB.lock().unwrap();
    drop(load_pool_data);
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
    // start_cronjobs();
    heartbeat();

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
