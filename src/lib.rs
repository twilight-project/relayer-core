pub mod config;
pub mod db;
pub mod kafkalib;
pub mod logging;
pub mod logging_examples;
pub mod postgresqllib;
pub mod pricefeederlib;
pub mod redislib;
pub mod relayer;

#[macro_use]
extern crate lazy_static;

#[cfg(feature = "profiling")]
use std::time::Instant;

#[cfg(feature = "profiling")]
macro_rules! time_it {
    ($name:expr, $code:block) => {
        let start = Instant::now();
        let result = $code;
        let duration = start.elapsed();
        tracing::info!("⏱️ {} took: {:?}", $name, duration);
        result
    };
}
