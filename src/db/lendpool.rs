#![allow(dead_code)]
#![allow(unused_imports)]
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;

// lazy_static! {
//     pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
// }

#[derive(Debug, Clone)]
pub struct LendPool {
    sequence: usize,
    nonce: usize,
    total_pool_share: f64,
    total_locked_value: f64,
    cmd_log: Vec<RpcCommand>,
    event_log: Vec<PoolEvent>,
    pending_orders: Vec<RpcCommand>,
    last_snapshot_id: usize,
}

#[derive(Debug)]
pub struct PoolEventLog {
    pub offset: i64,
    pub key: String,
    pub value: PoolEvent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PoolEvent {
    TraderOrder(RpcCommand, usize),
    LendOrder(RpcCommand, usize),
    RelayerUpdate(RelayerCommand, usize),
    Stop(String),
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PoolOrder {
    nonce: usize,
    sequence: usize,
    trader_order_data: Vec<TraderOrder>,
    lend_order_data: Vec<LendOrder>,
}
