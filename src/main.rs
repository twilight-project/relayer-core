// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
// mod aeronlib;
// mod aeronlibmpsc;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;

use crate::config::{LOCALDB, ORDERTEST, REDIS_POOL_CONNECTION, THREADPOOL};
use config::local_serial_core;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::{redis_db, redis_db_orderbook};
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // println!("time:{}", relayer::check_server_time());
    // relayer::get_fudning_data_from_psql(10);

    // init_psql();
    // ordertest::initprice();
    // ordertest::generatelendorder();
    // thread::sleep(time::Duration::from_millis(100));
    // start_cronjobs();
    // // thread::sleep(time::Duration::from_millis(3000));
    // // let sw = Stopwatch::start_new();
    // // println!("{}", relayer::get_localdb_string("OrderBook"));
    // // let time_ec = sw.elapsed();
    // // println!("time: {:#?} ", time_ec);
    // loop {
    //     thread::sleep(time::Duration::from_millis(100000000));
    // }

    // let sw = Stopwatch::start_new();
    // relayer::get_latest_orderbook();
    // let time_ec = sw.elapsed();
    // println!("time: {:#?} ", time_ec);
}
use chrono::Utc;
use serde_json::json;
