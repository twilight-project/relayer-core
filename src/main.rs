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
    // to create kafka topics
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // kafkalib::kafka_topic::kafka_new_topic(&*RPC_CLIENT_REQUEST);
    // kafkalib::kafka_topic::kafka_new_topic(&*TRADERORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDPOOL_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*CORE_EVENT_LOG);
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());

    heartbeat();

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
