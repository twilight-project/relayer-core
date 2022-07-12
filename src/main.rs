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
use redislib::{redis_db, redis_db_orderbook};
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

    let sw = Stopwatch::start_new();

    let orderid_list = redis_db::zrangeallopenorders();
    let length = orderid_list.len();
    if length > 0 {
        let t_c = sw.elapsed();
        println!("pool took1 {:#?}", t_c);
        get_size_in_mb(&orderid_list);
        let part_size = 500000;
        let loop_length: usize = (length + part_size) / part_size;
        println!("length:{}", length);
        println!("loop_length:{}", loop_length);

        for i in 0..loop_length {
            let mut endlimit = (i + 1) * part_size;
            if endlimit > length {
                endlimit = length;
            }
            println!("{:?}", &orderid_list[i * part_size..endlimit].len());
            println!("i:{}", i);
            let mut orderid_list_part: Vec<String> = Vec::new();
            orderid_list_part = orderid_list[i * part_size..endlimit].to_vec();
            let ordertx_array: Vec<TraderOrder> =
                redis_db::mget_trader_order(orderid_list_part).unwrap();
            let t_c2 = sw.elapsed();
            println!("pool took{} - {:#?}", i, t_c2);
            get_size_in_mb(&format!("{:#?}", ordertx_array));
        }
    }
}
