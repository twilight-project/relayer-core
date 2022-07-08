use super::orderbook::Side;
use crate::questdb::questdb;
use crate::questdb::questdb::{send_candledata_in_questdb, QUESTDB_INFLUX};
use crate::redislib::redis_db;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::{thread, time};
lazy_static! {
 // recent orders
 pub static ref RECENTORDER: Mutex<VecDeque<CloseTrade>> = Mutex::new(VecDeque::with_capacity(50));
 pub static ref CURRENTPENDINGCHART: Mutex<VecDeque<CloseTrade>> = Mutex::new(VecDeque::with_capacity(2));

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

use crate::config::THREADPOOL;
use crate::relayer::QueueResolver;
pub fn update_recent_orders(value: CloseTrade) {
    let mut currect_pending_chart = CURRENTPENDINGCHART.lock().unwrap();

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
    drop(threadpool);
    match send_candledata_in_questdb(value_clone.clone()) {
        Ok(_) => {
            println!("I'm Here");
            currect_pending_chart.push_front(value_clone.clone());
            if currect_pending_chart.len() > 2 {
                currect_pending_chart.pop_back();
            }
        }
        Err(arg) => {
            println!("QuestDB not reachable!!, {:#?}", arg);
            println!("trying to establish new connection...");
            let mut stream = QUESTDB_INFLUX.lock().unwrap();
            match questdb::connect() {
                Ok(value) => {
                    *stream = value;
                    drop(stream);
                    println!("new connection established");
                    //check for pending msg first
                    // let _ = currect_pending_chart.pop_back().unwrap();
                    for i in 1..currect_pending_chart.len() {
                        send_candledata_in_questdb(currect_pending_chart.pop_back().unwrap())
                            .unwrap();
                    }
                    QueueResolver::executor(String::from("questdb_queue"));
                    send_candledata_in_questdb(value_clone).unwrap();
                }
                Err(arg) => {
                    println!("QuestDB not reachable!!, {:#?}", arg);
                    //send pending msg into queue
                    QueueResolver::pending(
                        move || {
                            send_candledata_in_questdb(value_clone).unwrap();
                        },
                        String::from("questdb_queue"),
                    );
                }
            }
            // *stream = questdb::connect().expect("No connection found for QuestDB");
        }
    }
    // threadpool.execute(move || {
    // });
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
