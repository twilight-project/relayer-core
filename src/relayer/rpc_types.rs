use crate::relayer;
use crate::relayer::ThreadPool;
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
    pub price: i64,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateTraderOrder {
    pub account_id: String,
    pub position_type: PositionType,
    pub order_type: OrderType,
    pub leverage: f64,
    pub initial_margin: f64,
    pub available_margin: f64,
    pub order_status: OrderStatus,
    pub entryprice: f64,
    pub execution_price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateLendOrder {
    pub account_id: String,
    pub balance: f64,
    pub order_type: OrderType,
    pub order_status: OrderStatus,
    pub deposit: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ExecuteTraderOrder {
    pub account_id: String,
    pub uuid: Uuid,
    pub order_type: OrderType,
    pub settle_margin: f64,
    pub order_status: OrderStatus,
    pub execution_price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ExecuteLendOrder {
    pub account_id: String,
    pub uuid: Uuid,
    pub order_type: OrderType,
    pub settle_withdraw: f64, // % amount to withdraw
    pub order_status: OrderStatus,
    pub poolshare_price: f64, //withdraw pool share price
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CancelTraderOrder {
    pub account_id: String,
    pub uuid: Uuid,
    pub order_type: OrderType,
    pub order_status: OrderStatus,
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

impl CreateTraderOrder {
    pub fn push(self) {
        let create_trader_order_thread_pool = CREATE_TRADER_ORDER_THREAD_POOL.lock().unwrap();
        create_trader_order_thread_pool
            .execute(move || relayer::get_new_trader_order(self.serialize()));
        // thread::spawn(move || relayer::get_new_trader_order(self.serialize()));

        drop(create_trader_order_thread_pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: CreateTraderOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }

    pub fn fill_order(self) -> TraderOrder {
        let incomming_order = self.clone();
        TraderOrder::new(
            &incomming_order.account_id,
            incomming_order.position_type,
            incomming_order.order_type,
            incomming_order.leverage,
            incomming_order.initial_margin,
            incomming_order.available_margin,
            incomming_order.order_status,
            incomming_order.entryprice,
            incomming_order.execution_price,
        )
    }
}

impl CreateLendOrder {
    pub fn push(self) {
        let create_or_execute_lend_order_thread_pool =
            CREATE_OR_EXECUTE_LEND_ORDER_THREAD_POOL.lock().unwrap();
        create_or_execute_lend_order_thread_pool
            .execute(move || relayer::get_new_lend_order(self.serialize()));
        drop(create_or_execute_lend_order_thread_pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: CreateLendOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }

    pub fn fill_order(self) -> LendOrder {
        let incomming_order = self.clone();
        LendOrder::new(
            &incomming_order.account_id,
            incomming_order.balance,
            incomming_order.order_type,
            incomming_order.order_status,
            incomming_order.deposit,
        )
    }
}

impl ExecuteTraderOrder {
    pub fn push(self) {
        let execute_trader_order_thread_pool = EXECUTE_TRADER_ORDER_THREAD_POOL.lock().unwrap();
        execute_trader_order_thread_pool
            .execute(move || relayer::execute_trader_order(self.serialize()));
        drop(execute_trader_order_thread_pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: ExecuteTraderOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }

    pub fn get_order(self) -> Result<TraderOrder, std::io::Error> {
        TraderOrder::get_order_by_order_id(self.account_id, self.uuid)
    }
}
impl ExecuteLendOrder {
    pub fn push(self) {
        let create_or_execute_lend_order_thread_pool =
            CREATE_OR_EXECUTE_LEND_ORDER_THREAD_POOL.lock().unwrap();
        create_or_execute_lend_order_thread_pool
            .execute(move || relayer::execute_lend_order(self.serialize()));
        drop(create_or_execute_lend_order_thread_pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: ExecuteLendOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }
    pub fn get_order(self) -> Result<LendOrder, std::io::Error> {
        LendOrder::get_order_by_order_id(self.account_id, self.uuid)
    }
}
impl CancelTraderOrder {
    pub fn push(self) {
        let cancel_trader_order_thread_pool = CANCEL_TRADER_ORDER_THREAD_POOL.lock().unwrap();
        cancel_trader_order_thread_pool
            .execute(move || relayer::cancel_trader_order(self.serialize()));
        drop(cancel_trader_order_thread_pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: CancelTraderOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }

    pub fn get_order(self) -> Result<TraderOrder, std::io::Error> {
        let incomming_order = self.clone();
        TraderOrder::get_order_by_order_id(incomming_order.account_id, incomming_order.uuid)
    }
}
impl GetPnL {
    pub fn push(self) {
        // let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        // pool.execute(move || {
        //     send_aeron_msg(StreamId::GetPnL, self.serialize());
        // });
        // drop(pool);
    }

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
    pub fn push(self) {
        // let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        // pool.execute(move || {
        //     send_aeron_msg(StreamId::GetPoolShare, self.serialize());
        // });
        // drop(pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: GetPoolShare = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
