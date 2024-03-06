mod config;
mod db;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;
use relayer::*;
use std::{thread, time};
#[macro_use]
extern crate lazy_static;

fn main() {
    dotenv::dotenv().ok();
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    heartbeat();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
