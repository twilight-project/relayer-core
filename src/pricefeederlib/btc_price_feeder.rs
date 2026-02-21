//! ## The BTC Price Feeder
//!This function is taking payload string received from binance websocket and updating the btc price

#![allow(dead_code)]

use crate::config::BinanceAggTradePayload;
// use crate::kafkalib::producer_kafka;
use crate::relayer::set_localdb;
// use std::thread;

/// BTC Price updater
/// This fucntion is taking payload string received from binance websocket and updating the btc price
pub fn update_btc_price(payload: String, last_price: &f64) -> f64 {
    // let payload_clone = payload.clone();
    let current_price: f64;
    //checking if received msg is payload or ping/pong texts
    if payload.contains("aggTrade") {
        let binance_payload: BinanceAggTradePayload = match serde_json::from_str(&payload) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to parse Binance payload: {:?}", e);
                return *last_price;
            }
        };
        current_price = match binance_payload.price.parse::<f64>() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to parse price from Binance payload: {:?}", e);
                return *last_price;
            }
        };
        if current_price != *last_price {
            set_localdb("Latest_Price", current_price);
        }
        // Record timestamp of last successful price update (epoch millis)
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        set_localdb("Latest_Price_Timestamp", timestamp);
    } else {
        current_price = *last_price;
    }
    current_price
}
