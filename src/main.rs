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
use crate::aeronlib::aeronqueue::{aeron_send, start_aeron};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    let one_sec = time::Duration::from_millis(1000);
    // // kafkalib::kafka_topic::kafka_new_topic("NewTraderOrderQueue");
    // ordertest::initprice();
    // thread::spawn(move || {
    //     consume_kafka(
    //         String::from("NewTraderOrderQueue"),
    //         String::from("newOrder"),
    //     );
    // });
    // thread::sleep(one_sec);
    // // start_cronjobs();
    // startserver();
    // println!("pool took {:#?}", sw.elapsed());
    thread::sleep(one_sec);
    // let (sender, receiver) = mpsc::channel();
    // thread::spawn(move || {
    //     start_aeron();
    // });
    // let receiver = Arc::new(Mutex::new(receiver));
    let sender = aeronlib::aeronqueue::start_aeron_topic(StreamId::AERONMSG);
    let sender2 = aeronlib::aeronqueue::start_aeron_topic(StreamId::AERONMSGTWO);
    let mut i = 0;
    // thread::spawn(move ||
    let sender_clone = sender.lock().unwrap();
    let sender_clone2 = sender2.lock().unwrap();
    thread::sleep(time::Duration::from_millis(3000));
    loop {
        i = i + 1;
        sender_clone
            .send(format!("hello siddharth, msg : {}", i))
            .unwrap();
        sender_clone2
            .send(format!("hello pub2, msg : {}", i))
            .unwrap();

        thread::sleep(time::Duration::from_millis(1000));
        if i > 50 {
            break;
        }
    }
    // });
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
