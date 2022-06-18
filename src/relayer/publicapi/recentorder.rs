use super::orderbook::Side;
use crate::redislib::redis_db;
// use crate::relayer::TraderOrder;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::{thread, time};
// use uuid::Uuid;
lazy_static! {
 // recent orders
 pub static ref RECENTORDER: Mutex<VecDeque<CloseTrade>> = Mutex::new(VecDeque::with_capacity(50));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CloseTrade {
    pub side: Side,
    pub positionsize: f64,
    pub price: f64,
    pub timestamp: std::time::SystemTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RecentOrders {
    pub orders: Vec<CloseTrade>,
}

pub fn get_recent_orders() -> String {
    let local_storage = RECENTORDER.lock().unwrap();
    let data = local_storage.clone();
    drop(local_storage);
    serde_json::to_string(&data).unwrap()
}

use super::checkservertime::iso8601;
use crate::config::{QUESTDB_POOL_CONNECTION, THREADPOOL};
pub fn update_recent_orders(value: CloseTrade) {
    let threadpool = THREADPOOL.lock().unwrap();
    let value_clone = value.clone();
    threadpool.execute(move || {
        let mut local_storage = RECENTORDER.lock().unwrap();
        if local_storage.len() > 500 {
            local_storage.pop_back();
        }
        local_storage.push_front(value);
        drop(local_storage);
    });
    threadpool.execute(move || {
        let query = format!(
            "INSERT INTO recentorders VALUES ({}, {},{},$1)",
            (value_clone.side as u32),
            value_clone.price,
            value_clone.positionsize
        );
        let mut client = QUESTDB_POOL_CONNECTION.get().unwrap();
        client.execute(&query, &[&value_clone.timestamp]).unwrap();
        drop(client);
        // let mut local_storage = CANDLEDATA.lock().unwrap();
        // local_storage.push_front(value_clone);
        // drop(local_storage);
    });
}

pub fn updatebulk_recent_orders(value: Vec<CloseTrade>) {
    let mut local_storage = RECENTORDER.lock().unwrap();
    for data in value {
        if local_storage.len() > 500 {
            local_storage.pop_back();
        }
        local_storage.push_back(data);
    }
    drop(local_storage);
}

pub fn update_recent_order_from_db() {
    let recent_order_history = redis_db::get("RecentOrderHistory");
    if recent_order_history == String::from("key not found") {
    } else if recent_order_history == String::from("[]") {
    } else {
        let data: Vec<CloseTrade> = serde_json::from_str(&recent_order_history).unwrap();
        updatebulk_recent_orders(data);
    }
    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_millis(60000));
        thread::spawn(move || redis_db::set("RecentOrderHistory", &get_recent_orders()));
    });
}
