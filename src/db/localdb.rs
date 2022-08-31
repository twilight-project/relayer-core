#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use mpsc::{channel, Receiver, Sender};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::SystemTime;
use uuid::Uuid;
lazy_static! {
    pub static ref POSITION_SIZE_LOG: Arc<Mutex<PositionSizeLog>> =
        Arc::new(Mutex::new(PositionSizeLog::new()));
    pub static ref TRADER_LP_LONG: Arc<Mutex<SortedSet>> = Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LP_SHORT: Arc<Mutex<SortedSet>> = Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_OPEN_LONG: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_OPEN_SHORT: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_CLOSE_LONG: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_LIMIT_CLOSE_SHORT: Arc<Mutex<SortedSet>> =
        Arc::new(Mutex::new(SortedSet::new()));
    pub static ref TRADER_ORDER_DB: Arc<Mutex<OrderDB<TraderOrder>>> =
        Arc::new(Mutex::new( LocalDB::<TraderOrder>::new()));
    pub static ref LEND_ORDER_DB: Arc<Mutex<OrderDB<LendOrder>>> =
        Arc::new(Mutex::new(LocalDB::<LendOrder>::new()));
    pub static ref LEND_POOL_DB: Arc<Mutex<LendPool>> =
        Arc::new(Mutex::new(LendPool::default()));
    // pub static ref TRADER_ORDER_DB: Arc<Mutex<OrderDB<TraderOrder>>> =
    //     Arc::new(Mutex::new(LocalDB::<TraderOrder>::check_backup()));
    // pub static ref LEND_ORDER_DB: Arc<Mutex<OrderDB<LendOrder>>> =
    //     Arc::new(Mutex::new(LocalDB::<LendOrder>::check_backup()));
    // pub static ref LEND_POOL_DB: Arc<Mutex<LendPool>> =
    //     Arc::new(Mutex::new(LendPool::check_backup()));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PositionSizeLog {
    pub total_short_positionsize: f64,
    pub total_long_positionsize: f64,
    pub totalpositionsize: f64,
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
        Event::new(
            Event::PositionSizeLogDBUpdate(PositionSizeLogCommand::AddPositionSize(
                positiontype,
                positionsize,
            )),
            String::from("AddPositionSize"),
            CORE_EVENT_LOG.clone().to_string(),
        );
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
        Event::new(
            Event::PositionSizeLogDBUpdate(PositionSizeLogCommand::RemovePositionSize(
                positiontype,
                positionsize,
            )),
            String::from("RemovePositionSize"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    pub fn new() -> Self {
        // impl to read from redis or event logs
        PositionSizeLog {
            total_short_positionsize: 0.0,
            total_long_positionsize: 0.0,
            totalpositionsize: 0.0,
        }
    }
}

// impl Display for TaderOrderError {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

// impl std::error::Error for TaderOrderError {}

// impl From<&str> for TaderOrderError {
//     fn from(message: &str) -> Self {
//         TaderOrderError(message.to_string())
//     }
// }
