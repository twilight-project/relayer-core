// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
// mod aeronlib;
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
// use std::sync::mpsc;
// use std::sync::Arc;
// use std::sync::Mutex;

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

    // let (sender, receiver) = mpsc::channel();

    // let receiver = Arc::new(Mutex::new(receiver));

    // thread::spawn(move || {
    //     aeronlib::publisher_aeron::pub_aeron(Arc::clone(&receiver));
    // });
    // thread::spawn(move || loop {
    //     i = i + 1;
    //     sender
    //         .send(format!("hello siddharth, msg : {}", i).to_string())
    //         .unwrap();
    //     thread::sleep(time::Duration::from_millis(100));
    //     // thread::sleep(one_sec.clone());
    // });
    // // thread::spawn(move || {
    // aeronlib::subscriber_aeron::sub_aeron();
    // });
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
