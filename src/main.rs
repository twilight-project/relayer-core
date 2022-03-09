// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
mod aeronlib;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;
use crate::aeronlib::types::StreamId;
use crate::config::{LOCALDB, ORDERTEST, REDIS_POOL_CONNECTION, THREADPOOL};
use config::local_serial_core;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::redis_db;
use relayer::*;
use std::process::Command;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use crate::aeronlib::aeronqueue::*;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    ordertest::initprice();
    thread::sleep(time::Duration::from_millis(100));
    // thread::spawn(move || {
    start_cronjobs();
    // aeronlib::types::init_aeron_queue();
    // start_aeron_topic_consumer(StreamId::CreateTraderOrder);
    // });
    pricefeederlib::price_feeder::receive_btc_price();
    // let (sender, receiver) = mpsc::channel();
    // let receiver = Arc::new(Mutex::new(receiver));
    // thread::spawn(move || {
    //     aeronlib::publisher_aeron::pub_aeron(StreamId::CreateTraderOrder, receiver);
    // });

    // loop {
    //     thread::sleep(time::Duration::from_millis(1000));
    //     sender.send("helllo".to_string()).unwrap();

    //     thread::sleep(time::Duration::from_millis(1000));

    //     println!("{:#?}", rec_aeron_msg(StreamId::CreateTraderOrder));
    // }

    // thread::sleep(time::Duration::from_millis(100));
    // pricefeederlib::price_feeder::receive_btc_price();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
