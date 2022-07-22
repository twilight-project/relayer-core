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
    pub cmd: TraderOrderCommand,
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

    pub fn get_order_readonly(orderid: &str) -> Result<OrderLog, std::io::Error> {
        let mut db = DB_IN_MEMORY.lock().unwrap();
        let ordertrader = match db.get_mut(orderid) {
            Some(value) => value,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        };

        let orx = ordertrader.read().unwrap();
        Ok(orx.clone())
    }

    pub fn insert_new_order_log(orderlog: OrderLog, orderid: String) -> Result<(), std::io::Error> {
        let orderlog_arc = Arc::new(RwLock::new(orderlog));
        let mut db = DB_IN_MEMORY.lock().unwrap();
        let _db_hash = db.insert(orderid, orderlog_arc);
        Ok(())
    }

    pub fn insert_new_traderorder(ordertx: TraderOrder, log: Rcmd) -> Result<(), std::io::Error> {
        let orderlog_arc = Arc::new(RwLock::new(OrderLog {
            orderdata: ordertx.clone(),
            orderlog: vec![log],
        }));

        let mut db = DB_IN_MEMORY.lock().unwrap();
        let _db_hash = db.insert(ordertx.uuid.to_string(), orderlog_arc);
        Ok(())
    }
}

impl Rcmd {
    pub fn new(cmd: TraderOrderCommand) -> Self {
        Rcmd {
            cmd: cmd,
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderCommand {
    NewOrder,
    CancelOrder,
    SettleOrder,
    FundingRate,
    Liquidate,
    Filled,
    PendingOrder,
    OpenLimit,
    CloseLimit,
    StopLimit,
    CloseMarket,
    LimitTPSL,
    PriceTicker,
    Snapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TraderOrderCommand {
    NewOrder {
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
    },
    Liquidate {
        liquidation_price: f64,
        available_margin: f64,
        nonce: u128,
    },
    CancelOrder {
        uuid: Uuid,
        order_type: OrderType,
        order_status: OrderStatus,
    },
    CloseMarket {
        uuid: Uuid,
        order_type: OrderType,
        order_status: OrderStatus,
        execution_price: f64,
    },
}

//  position_type: PositionType,
//  order_type: OrderType,
//  leverage: f64,
//  initial_margin: f64,
//  order_status: OrderStatus,
//  entryprice: f64,
//  execution_price: f64,
