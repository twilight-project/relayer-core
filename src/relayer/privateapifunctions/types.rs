use crate::relayer;
use crate::relayer::ThreadPool;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;
lazy_static! {
    // pub static ref CREATE_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> =
    //     Mutex::new(ThreadPool::new(3));
    // pub static ref CREATE_OR_EXECUTE_LEND_ORDER_THREAD_POOL: Mutex<ThreadPool> =
    //     Mutex::new(ThreadPool::new(1));
    // pub static ref CANCEL_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> =
    //     Mutex::new(ThreadPool::new(1));
    // pub static ref EXECUTE_TRADER_ORDER_THREAD_POOL: Mutex<ThreadPool> =
    //     Mutex::new(ThreadPool::new(3));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetOrderDetail {
    pub account_id: String,
    pub order_id: Uuid,
}
