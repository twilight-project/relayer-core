#![allow(dead_code)]
#![allow(unused_imports)]
// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
use crate::db::{LocalDB, OrderDB, SortedSet};
use crate::relayer::*;
use crate::relayer::{ThreadPool, TraderOrder};
use mpsc::{channel, Receiver, Sender};
use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::SystemTime;
use uuid::Uuid;
lazy_static! {
    pub static ref DB_IN_MEMORY: Mutex<HashMap<String, Arc<RwLock<OrderLog>>>> =
        Mutex::new(HashMap::new());
    pub static ref POSITION_SIZE_LOG: Arc<Mutex<PositionSizeLog>> =
        Arc::new(Mutex::new(PositionSizeLog::new()));
    pub static ref DB_THREADPOOL: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("DB_THREADPOOL")));
        //TraderOrderbyLiquidationPriceFORLong
    pub static ref TRADER_LP_LONG: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LP_SHORT: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_OPEN_LONG: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_OPEN_SHORT: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_CLOSE_LONG: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_CLOSE_SHORT: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_ORDER_DB: Arc<Mutex<OrderDB<TraderOrder>>> =
        Arc::new(Mutex::new(LocalDB::<TraderOrder>::check_backup()));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderLog {
    pub orderdata: TraderOrder,
    pub orderlog: Vec<Rcmd>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PositionSizeLog {
    pub total_short_positionsize: f64,
    pub total_long_positionsize: f64,
    pub totalpositionsize: f64,
    pub orderlog: Vec<Rcmd>,
}
impl PositionSizeLog {
    pub fn add_order(positiontype: PositionType, positionsize: f64) {
        match positiontype {
            PositionType::LONG => {
                let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
                position_size_log.total_long_positionsize += positionsize;
                position_size_log.totalpositionsize += positionsize;
                drop(position_size_log);
                // send log to kafka
            }
            PositionType::SHORT => {
                let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
                position_size_log.total_short_positionsize += positionsize;
                position_size_log.totalpositionsize += positionsize;
                drop(position_size_log);
                // send log to kafka
            }
        }
    }
    pub fn remove_order(positiontype: PositionType, positionsize: f64) {
        match positiontype {
            PositionType::LONG => {
                let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
                position_size_log.total_long_positionsize -= positionsize;
                position_size_log.totalpositionsize -= positionsize;
                drop(position_size_log);
                // send log to kafka
            }
            PositionType::SHORT => {
                let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
                position_size_log.total_short_positionsize -= positionsize;
                position_size_log.totalpositionsize -= positionsize;
                drop(position_size_log);
                // send log to kafka
            }
        }
    }
    pub fn new() -> Self {
        // impl to read from redis or event logs
        PositionSizeLog {
            total_short_positionsize: 0.0,
            total_long_positionsize: 0.0,
            totalpositionsize: 0.0,
            orderlog: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Rcmd {
    pub cmd: TraderOrderCommand,
    pub timestamp: SystemTime,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub payload: HashMap<String, Arc<RwLock<OrderLog>>>,
    pub timestamp: SystemTime,
    pub snapshot_version: String,
}

// impl Snapshot {
//     pub fn take_snapshot() {
//         let mut db = DB_IN_MEMORY.lock().unwrap();
//         let snapshot = Snapshot {
//             payload: db.to_value(),
//             timestamp: SystemTime::now(),
//         };
//     }
// }

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
        // let threadpool = DB_THREADPOOL.lock().unwrap();
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
        let metadata: HashMap<String, String> = HashMap::new();
        Rcmd {
            cmd: cmd,
            timestamp: SystemTime::now(),
            metadata: metadata,
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
    OpenLimit {
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
        nonce: usize,
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

use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub struct TaderOrderError(String);

impl Display for TaderOrderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TaderOrderError {}

impl From<&str> for TaderOrderError {
    fn from(message: &str) -> Self {
        TaderOrderError(message.to_string())
    }
}
