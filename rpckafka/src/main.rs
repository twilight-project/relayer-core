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
    // thread::spawn(move || {
    //     let rec = kafkalib::kafkacmd::receive_from_kafka_queue(
    //         String::from("CLIENT-REQUEST"),
    //         String::from("alfa"),
    //     )
    //     .unwrap();
    //     let receiver = rec.lock().unwrap();
    //     loop {
    //         // let message = rec.lock().unwrap().recv().unwrap();
    //         let message = receiver.recv().unwrap();
    //         println!("Data: {:?}", message.value);
    //         println!("Data: {:?}", message.key);
    //     }
    // });
    let handle = thread::Builder::new()
        .name(String::from("kafka_queue_rpc_server"))
        .spawn(move || {
            kafka_queue_rpc_server();
        })
        .unwrap();
    handle.join().unwrap();
}
