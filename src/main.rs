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
mod questdb;
mod redislib;
mod relayer;

use config::local_serial_core;
use config::*;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::*;
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use questdb::questdb::send_candledata_in_questdb;
use std::sync::{mpsc, Arc, Mutex};

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // println!("time:{}", relayer::check_server_time());
    // relayer::get_fudning_data_from_psql(10);

    init_psql();
    ordertest::initprice();
    ordertest::generatelendorder();
    thread::sleep(time::Duration::from_millis(100));
    start_cronjobs();
    thread::sleep(time::Duration::from_millis(10000));
    updatefundingrate(1.0);

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
    // println!("Count:{}", redis_batch::zrangeallopenorders_batch_count());
    // println!("Count:{}", redis_db::zrangeallopenorders().len());
    // let sw = Stopwatch::start_new();
    // // check_pipe();
}
use crate::config::REDIS_POOL_CONNECTION;
// use crate::relayer::*;
use datasize::data_size;
use datasize::DataSize;
// use r2d2_redis::redis;
use std::collections::HashSet;
