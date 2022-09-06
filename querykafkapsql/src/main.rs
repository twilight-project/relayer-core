#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod kafkalib;
mod query;
mod relayer;
use config::*;
use query::*;
use relayer::*;
use std::{thread, time};
#[macro_use]
extern crate lazy_static;
fn main() {
    dotenv::dotenv().expect("Failed loading dotenv");
    thread::Builder::new()
        .name(String::from("upload_rpc_command_to_psql"))
        .spawn(move || {
            crate::query::upload_rpc_command_to_psql();
        })
        .unwrap();
    thread::Builder::new()
        .name(String::from("upload_event_log_to_psql"))
        .spawn(move || {
            crate::query::upload_event_log_to_psql();
        })
        .unwrap();

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
