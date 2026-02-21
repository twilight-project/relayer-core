mod config;
mod db;
mod kafkalib;
mod logging;
mod pricefeederlib;
mod relayer;
use db::snapshot;
use relayer::*;
use std::{process, thread, time};
#[macro_use]
extern crate lazy_static;

fn main() {
    dotenv::dotenv().ok();

    // Initialize logging system
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        process::exit(1);
    }

    tracing::info!("Starting Twilight Relayer");
    heartbeat();
    wait_for_relayer_shutdown();
    tracing::info!("Relayer shutdown cmd received");
    thread::sleep(time::Duration::from_millis(10000));
    tracing::info!("Relayer started taking snapshot");
    let _ = snapshot();
    thread::sleep(time::Duration::from_millis(10000));
    tracing::info!("Relayer shutting down");
    process::exit(0);
}
