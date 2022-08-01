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

// #[derive(Debug, Clone)]
// pub struct LendPool<T> {
//     ordertable: HashMap<Uuid, Arc<RwLock<T>>>,
//     sequence: usize,
//     nonce: usize,
//     cmd: Vec<RpcCommand>,
//     event: Vec<Event<T>>,
//     aggrigate_log_sequence: usize,
//     last_snapshot_id: usize,
// }
