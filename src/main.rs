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
    // // kafkalib::kafka_topic::kafka_new_topic("NewTraderOrderQueue");
    ordertest::initprice();
    // thread::spawn(move || {
    //     consume_kafka(
    //         String::from("NewTraderOrderQueue"),
    //         String::from("newOrder"),
    //     );
    // });
    thread::sleep(time::Duration::from_millis(100));
    thread::spawn(move || {
        start_cronjobs();
    });
    thread::sleep(time::Duration::from_millis(100));

    thread::spawn(move || {
        // println!(
        //     "my msg:  {:#?}",
        //     CreateTraderOrder::deserialize(rec_aeron_msg(StreamId::CreateTraderOrder).msg)
        //         .fill_order()
        // );
        get_new_trader_order();
        // thread::sleep(time::Duration::from_millis(10));
    });
    thread::spawn(move || {
        // println!(
        //     "my msg:  {:#?}",
        //     rec_aeron_msg(StreamId::CreateLendOrder).msg
        // );
        get_new_lend_order()
        // thread::sleep(time::Duration::from_millis(10));
    });
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
