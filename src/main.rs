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

use config::*;
use db::*;
use redislib::*;
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
use uuid::Uuid;
#[macro_use]
extern crate lazy_static;
use std::sync::{mpsc, Arc, Mutex};

fn main() {
    dotenv::dotenv().expect("Failed loading dotenv");

    // to create kafka topics
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // kafkalib::kafka_topic::kafka_new_topic(&*RPC_CLIENT_REQUEST);
    // kafkalib::kafka_topic::kafka_new_topic(&*TRADERORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDPOOL_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*CORE_EVENT_LOG);
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());

    //load previous database
    let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
    // drop(load_trader_data);
    let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
    // drop(load_lend_data);
    let mut load_pool_data = LEND_POOL_DB.lock().unwrap();
    // drop(load_pool_data);

    let (data1, data2, data3): (OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) =
        load_backup_data();
    *load_trader_data = data1;
    *load_lend_data = data2;
    *load_pool_data = data3;
    drop(load_trader_data);
    drop(load_lend_data);
    drop(load_pool_data);
    ordertest::initprice();

    heartbeat();

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
