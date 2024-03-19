//! ## The BTC Price Feeder
//!This function is taking payload string received from binance websocket and updating the btc price into redisDB having keys "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.

#![allow(dead_code)]

use crate::config::BinanceAggTradePayload;
// use crate::kafkalib::producer_kafka;
use crate::relayer::set_localdb;
// use std::thread;

/// BTC Price updater
///  calling this function inside receive_btc_price() to update price in redisDB
/// This fucntion is taking payload string received from binance websocket and updating the btc price into redisDB having key "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.
pub fn update_btc_price(payload: String, mut last_price: &f64) -> f64 {
    // let payload_clone = payload.clone();
    let current_price: f64;
    //checking if received msg is payload or ping/pong texts
    if payload.contains("aggTrade") {
        // let psql_pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();

        //btc price update on redis DB
        let binance_payload: BinanceAggTradePayload =
            serde_json::from_str(&payload.clone()).unwrap();
        current_price = binance_payload.clone().price.parse::<f64>().unwrap();
        if current_price != *last_price {
            println!("current_price: {:?}", current_price);
            set_localdb("Latest_Price", current_price);
        }
    } else {
        current_price = *last_price;
    }
    current_price
}
