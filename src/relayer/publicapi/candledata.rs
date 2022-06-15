use super::orderbook::Side;
use super::recentorder::CloseTrade;
use crate::redislib::redis_db;
use crate::relayer::TraderOrder;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::{thread, time};
use uuid::Uuid;

lazy_static! {
 // recent orders
 pub static ref CANDLEDATA: Mutex<VecDeque<CloseTrade>> = Mutex::new( VecDeque::new());
 pub static ref CANDLE_HASHMAP: Mutex<HashMap<String, Vec<CloseTrade>>> = Mutex::new(HashMap::new());
}
use std::collections::HashMap;

pub fn update_candle_data() {
    let mut candle_hashmap = CANDLE_HASHMAP.lock().unwrap();
    let mut recent_orders = CANDLEDATA.lock().unwrap();
    for i in 1..recent_orders.len() {
        let trade = recent_orders.pop_back().unwrap();
        println!("{:#?}", trade);
        let time_key = time_till_min(&trade.timestamp);
        let time_key_clone = time_key.clone();
        candle_hashmap
            .get_mut(&time_key.clone())
            .unwrap()
            .push(trade);
        // data_array.push(trade);
        // candle_hashmap.insert(time_key_clone, data_array.to_vec());
    }
    drop(recent_orders);
    drop(candle_hashmap);
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candle {
    pub min: f64,
    pub max: f64,
    pub open: f64,
    pub close: f64,
    pub sell_volume: f64,
    pub buy_volume: f64,
    pub timestamp: std::time::SystemTime,
}

use chrono::prelude::{DateTime, Utc};

fn time_till_min(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    let date_time = format!("{}", dt.format("%d-%m-%Y-%H:%M"));
    date_time
}
