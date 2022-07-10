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

use crate::config::{
    LOCALDB, ORDERTEST, QUESTDB_POOL_CONNECTION, REDIS_POOL_CONNECTION, THREADPOOL,
};
use config::local_serial_core;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::{redis_db, redis_db_orderbook};
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use questdb::questdb::send_candledata_in_questdb;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // println!("time:{}", relayer::check_server_time());
    // relayer::get_fudning_data_from_psql(10);

    init_psql();
    ordertest::initprice();
    ordertest::generatelendorder();
    thread::sleep(time::Duration::from_millis(100));
    start_cronjobs();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
