#![allow(dead_code)]
#![allow(unused_imports)]
// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
use crate::relayer::*;
use crate::relayer::{ThreadPool, TraderOrder};
use mpsc::{channel, Receiver, Sender};
use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::SystemTime;
use uuid::Uuid;
lazy_static! {

    // local database hashmap
    pub static ref LOCALDB: Mutex<HashMap<&'static str,f64>> = Mutex::new(HashMap::new());

    // local orderbook
    pub static ref LOCALDBSTRING: Mutex<HashMap<&'static str,String>> = Mutex::new(HashMap::new());

    // https://github.com/palfrey/serial_test/blob/main/serial_test/src/code_lock.rs
    pub static ref LOCK: Arc<RwLock<HashMap<String, ReentrantMutex<()>>>> = Arc::new(RwLock::new(HashMap::new()));

    pub static ref DB_IN_MEMORY:Mutex<HashMap<String, Arc<RwLock<OrderLog>>>>=Mutex::new(HashMap::new());

    pub static ref DB_THREADPOOL:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(5,String::from("DB_THREADPOOL")));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderLog {
    pub orderdata: TraderOrder,
    pub orderlog: Vec<Rcmd>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Rcmd {
    pub cmd: OrderType,
    pub price: f64,
    pub timestamp: SystemTime,
}

impl OrderLog {
    pub fn new(traderorder: TraderOrder) -> Self {
        OrderLog {
            orderdata: traderorder,
            orderlog: Vec::new(),
        }
    }

    pub fn get_order(orderid: &str) -> Arc<RwLock<OrderLog>> {
        let mut db = DB_IN_MEMORY.lock().unwrap();
        Arc::clone(db.get_mut(orderid).unwrap())
    }

    pub fn insert_new_order_log(orderlog: OrderLog, orderid: String) -> Result<(), std::io::Error> {
        let orderlog_arc = Arc::new(RwLock::new(orderlog));
        let mut db = DB_IN_MEMORY.lock().unwrap();
        let db_hash = db.insert(orderid, orderlog_arc);
        Ok(())
    }
}

impl Rcmd {
    pub fn new() -> Self {
        Rcmd {
            cmd: OrderType::MARKET,
            price: 0.0,
            timestamp: SystemTime::now(),
        }
    }
}
