#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod kafkalib;
mod relayer;
use config::*;
use relayer::*;
use std::{thread, time};
#[macro_use]
extern crate lazy_static;
fn main() {
    dotenv::dotenv().expect("Failed loading dotenv");
    let handle = thread::Builder::new()
        .name(String::from("kafka_queue_rpc_server"))
        .spawn(move || {
            kafka_queue_rpc_server_with_zkos();
            // kafka_queue_rpc_server();
        })
        .unwrap();
    handle.join().unwrap();
}
