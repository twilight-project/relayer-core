use crate::aeronlib::aeronqueue::send_aeron_msg;
use crate::aeronlib::types::StreamId;
use crate::config::THREADPOOL_ORDER_AERON_QUEUE;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub execution_price: f64, //withdraw pool share price
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CancelTraderOrder {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetPnL {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GetPoolShare {}

impl CreateTraderOrder {
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::CreateTraderOrder, self.serialize());
        });
        drop(pool);
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
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::CreateLendOrder, self.serialize());
        });
        drop(pool);
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
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::ExecuteTraderOrder, self.serialize());
        });
        drop(pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: ExecuteTraderOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
impl ExecuteLendOrder {
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::ExecuteLendOrder, self.serialize());
        });
        drop(pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: ExecuteLendOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
impl CancelTraderOrder {
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::CancelTraderOrder, self.serialize());
        });
        drop(pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: String) -> Self {
        let deserialized: CancelTraderOrder = serde_json::from_str(&json).unwrap();
        deserialized
    }
}
impl GetPnL {
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::GetPnL, self.serialize());
        });
        drop(pool);
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
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::GetPoolShare, self.serialize());
        });
        drop(pool);
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
