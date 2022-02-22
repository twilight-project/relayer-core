// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;
mod utils;
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

fn main() {
    let one_sec = time::Duration::from_millis(1000);
    // kafkalib::kafka_topic::kafka_new_topic("NewTraderOrderQueue");
    ordertest::initprice();
    thread::spawn(move || {
        consume_kafka(
            String::from("NewTraderOrderQueue"),
            String::from("newOrder"),
        );
    });
    thread::sleep(one_sec);
    let sw = Stopwatch::start_new();
    // start_cronjobs();
    startserver();
    println!("pool took {:#?}", sw.elapsed());
    thread::sleep(one_sec);
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
