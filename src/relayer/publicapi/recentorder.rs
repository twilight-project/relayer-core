use super::orderbook::Side;
use crate::questdb::questdb::send_candledata_in_questdb;
use crate::redislib::redis_db;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::{thread, time};
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

pub fn get_recent_orders() -> RecentOrders {
    let local_storage = RECENTORDER.lock().unwrap();
    let data = local_storage.clone();
    drop(local_storage);
    return RecentOrders {
        orders: Vec::from(data),
    };
}

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
        send_candledata_in_questdb(value_clone);
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
        // println!("{}", recent_order_history);
        let data: RecentOrders = serde_json::from_str(&recent_order_history).unwrap();
        updatebulk_recent_orders(data.orders);
    }
    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_millis(60000));
        thread::spawn(move || {
            redis_db::set(
                "RecentOrderHistory",
                &serde_json::to_string(&get_recent_orders()).unwrap(),
            )
        });
    });
}
