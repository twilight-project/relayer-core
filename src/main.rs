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
mod utils;
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
    let one_sec = time::Duration::from_millis(1000);
    // // kafkalib::kafka_topic::kafka_new_topic("NewTraderOrderQueue");
    ordertest::initprice();
    // thread::spawn(move || {
    //     consume_kafka(
    //         String::from("NewTraderOrderQueue"),
    //         String::from("newOrder"),
    //     );
    // });
    // thread::sleep(one_sec);
    // // start_cronjobs();
    thread::sleep(one_sec);
    thread::spawn(move || {
        // startserver();
        start_cronjobs();
    });
    // println!("pool took {:#?}", sw.elapsed());
    thread::sleep(one_sec);

    // thread::spawn(move || {
    //     start_aeron_topic_consumer(StreamId::AERONMSG);
    // });
    // thread::spawn(move || {
    //     thread::sleep(time::Duration::from_millis(10));
    //     start_aeron_topic_producer(StreamId::AERONMSG);
    // });

    thread::sleep(time::Duration::from_millis(1000));
    let mut i = 0;
    loop {
        i = i + 1;
        send_aeron_msg(
            StreamId::CreateOrder,
            format!("hello siddharth, msg : {}", i),
        );

        thread::sleep(time::Duration::from_millis(10));
        if i > 100 {
            break;
        }
    }
    thread::spawn(move || loop {
        println!("my msg:  {}", rec_aeron_msg(StreamId::CreateOrder).msg);
        // thread::sleep(time::Duration::from_millis(1000));
    });
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
