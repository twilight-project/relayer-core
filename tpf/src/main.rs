mod config;
mod kafkalib;
mod pricefeederlib;
mod redislib;
mod utils;
use std::thread;
mod postgresqllib;
extern crate stopwatch;

#[macro_use]
extern crate lazy_static;

fn main() {
	// kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
	let handle = thread::spawn(move || {
		kafkalib::consumer_kafka::consume_main();
	});
	pricefeederlib::price_feeder::receive_btc_price();
	handle.join().unwrap();

	// redislib::redis_db::save_redis_backup(String::from("src/redislib/backup/."));
}
