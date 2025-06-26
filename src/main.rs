mod config;
mod db;
mod kafkalib;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;
use db::snapshot;
use relayer::*;
use std::{process, thread, time};
#[macro_use]
extern crate lazy_static;

fn main() {
    dotenv::dotenv().ok();
    heartbeat();
    loop {
        thread::sleep(time::Duration::from_millis(10000));
        if get_relayer_status() {
        } else {
            thread::sleep(time::Duration::from_millis(5000));
            println!("Relayer relayer started taking snapshot");
            let _ = snapshot();
            thread::sleep(time::Duration::from_millis(10000));
            println!("Relayer Shutting down");
            process::exit(0);
        }
    }
}
