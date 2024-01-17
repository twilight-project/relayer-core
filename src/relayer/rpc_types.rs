use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;
lazy_static! {
    pub static ref CREATE_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(15, String::from("CREATE_TRADER_ORDER_THREAD_POOL"))
    );
    pub static ref CREATE_OR_EXECUTE_LEND_ORDER_THREAD_POOL: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(1, String::from("CREATE_OR_EXECUTE_LEND_ORDER_THREAD_POOL"))
    );
    pub static ref CANCEL_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(5, String::from("CANCEL_TRADER_ORDER_THREAD_POOL"))
    );
    pub static ref EXECUTE_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(15, String::from("EXECUTE_TRADER_ORDER_THREAD_POOL"))
    );
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TestLocaldb {
    pub orderid: Uuid,
    pub price: f64,
    pub key: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetPnL {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetPoolShare {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CandleRequest {
    pub sample_by: String,
    pub limit: i32,
    pub pagination: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetOrderDetail {
    pub account_id: String,
    pub order_id: Uuid,
}

impl GetPnL {
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: GetPnL = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
impl GetPoolShare {
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: GetPoolShare = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
