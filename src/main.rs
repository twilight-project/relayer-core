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

    // init_psql();
    // ordertest::initprice();
    // ordertest::generatelendorder();
    // thread::sleep(time::Duration::from_millis(100));
    // start_cronjobs();
    // loop {
    //     thread::sleep(time::Duration::from_millis(100000000));
    // }
    // println!("Count:{}", redis_batch::zrangeallopenorders_batch_count());
    // println!("Count:{}", redis_db::zrangeallopenorders().len());
    let sw = Stopwatch::start_new();
    // check_pipe();

    let (loop_count, lenght, data_receiver) = redis_batch::getdata_redis_batch(250000);
    for i in 0..loop_count {
        let order_array = data_receiver.lock().unwrap().recv().unwrap();
        println!(
            "data length : {}, data size: {:#?}MB",
            order_array.len(),
            data_size(&format!("{:#?}", order_array)) / (8 * 1024 * 1024)
        );
    }

    let time = sw.elapsed();
    println!("time took {:#?}", time);
}
use crate::config::REDIS_POOL_CONNECTION;
// use crate::relayer::*;
use datasize::data_size;
use datasize::DataSize;
// use r2d2_redis::redis;
use std::collections::HashSet;
pub fn check_pipe() {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    // let (v1, v2): (String, String);
    let mut pipeline = redis::pipe();
    pipeline
        .cmd("mset")
        .arg(format!("wikey-{}", -1))
        .arg(format!("ivalue-{}", -1));
    pipeline
        .arg(format!("wikey-{}", 0))
        .arg(format!("ivalue-{}", 0));
    for i in 0..500000 {
        pipeline
            .arg(format!("wikey-{}", i))
            .arg(format!("ivalue-{}ivalue-ivalue-ivalue-ivalue-ivalue-i-ivalue-ivalue-ivalivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-iv-ivalue-ivalue-ivalue-", i));
    }
    pipeline
        .cmd("mset")
        .arg(format!("wikey-{}", -3))
        .arg(format!("ivalue-{}", -3));
    println!("Im here");
    let (v1, v2): (String, String) = pipeline.query(&mut *conn).unwrap();
    println!("{}{}", v1, v2);
}
