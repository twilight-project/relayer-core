// mod config;
// mod db;
// mod kafkalib;
// mod ordertest;
// mod postgresqllib;
// mod pricefeederlib;
// mod questdb;
// mod redislib;
// mod relayer;
// use config::CORE_EVENT_LOG;
// use db::snapshot;
// use kafka::client::FetchOffset;
// use relayer::*;
// use std::{process, thread, time};
// #[macro_use]
// extern crate lazy_static;

mod config;
mod db;
mod error;
mod kafkalib;
// mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;
use config::CORE_EVENT_LOG;
use db::{snapshot, Event};
use error::{RelayerError, Result};
use kafka::client::FetchOffset;
use relayer::*;
use std::{
    process, thread,
    time::{self, SystemTime},
};
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};
#[macro_use]
extern crate lazy_static;

fn setup_logging() -> Result<()> {
    let log_file = std::env::var("LOG_FILE").unwrap_or_else(|_| "relayer.log".to_string());

    let file_appender = tracing_appender::rolling::RollingFileAppender::new(
        tracing_appender::rolling::RollingFileAppender::builder()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("relayer")
            .filename_suffix("log")
            .build(),
        log_file,
    );

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    Ok(())
}

fn main() -> Result<()> {
    // Initialize logging
    setup_logging()?;

    // Load environment variables
    dotenv::dotenv().ok();
    info!("Starting relayer...");

    // Start heartbeat
    if let Err(e) = heartbeat() {
        error!("Failed to start heartbeat: {}", e);
        return Err(e);
    }

    // Main loop
    loop {
        thread::sleep(time::Duration::from_millis(10000));

        if get_relayer_status() {
            continue;
        }

        warn!("Relayer status check failed, initiating shutdown");
        thread::sleep(time::Duration::from_millis(5000));

        info!("Taking final snapshot before shutdown");
        if let Err(e) = snapshot() {
            error!("Failed to take final snapshot: {}", e);
        }

        thread::sleep(time::Duration::from_millis(10000));
        info!("Shutting down relayer");
        process::exit(0);
    }
}
