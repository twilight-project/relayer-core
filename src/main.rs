mod config;
mod db;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;
use config::IS_RELAYER_ACTIVE;
use db::snapshot;
use relayer::*;
use std::{process, thread, time};
#[macro_use]
extern crate lazy_static;

fn main() {
    dotenv::dotenv().ok();
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    heartbeat();
    loop {
        thread::sleep(time::Duration::from_millis(10000));
        if *IS_RELAYER_ACTIVE {
        } else {
            thread::sleep(time::Duration::from_millis(5000));
            println!("Relayer relayer started taling snapshot");
            let _ = snapshot();
            thread::sleep(time::Duration::from_millis(10000));
            println!("Relayer Shutting down");
            process::exit(1);
        }
    }
}
