//! ## The BTC Price Feeder
//!This function is taking payload string received from binance websocket and updating the btc price into redisDB having keys "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.

#![allow(dead_code)]

use crate::config::BinanceMiniTickerPayload;
use crate::kafkalib::producer_kafka;
use crate::redislib::redis_db;
use std::thread;

/// BTC Price updater
///  calling this function inside receive_btc_price() to update price in redisDB
/// This fucntion is taking payload string received from binance websocket and updating the btc price into redisDB having key "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.
pub fn update_btc_price(payload: String) {
    let payload_clone = payload.clone();

    //checking if received msg is payload or ping/pong texts
    if payload.contains("24hrMiniTicker") {
        //btc price update on redis DB
        thread::spawn(move || {
            let binance_payload: BinanceMiniTickerPayload = serde_json::from_str(&payload).unwrap();
            // set `btc:price` = received price
            redis_db::set("btc:price", &binance_payload.c);
            // set `btc:price:full_payload` = full mini ticker payload
            redis_db::set("btc:price:full_payload", &payload);
            // println!("rate :{}", &binance_payload.e);
            // println!("rate :{:#?}", &binance_payload);
        });
        //btc price payload added to kafka topic : BinanceMiniTickerPayload
        thread::spawn(move || {
            // println!("Producer payload :{:#?}", &payload_clone);
            // producer::produce_main(message_data, topic);
            producer_kafka::produce_main(&payload_clone, "BinanceMiniTickerPayload");
        });
    }
}
