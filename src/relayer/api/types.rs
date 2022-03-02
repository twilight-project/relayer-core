use crate::aeronlib::aeronqueue::send_aeron_msg;
use crate::aeronlib::types::StreamId;
use crate::config::THREADPOOL_ORDER_AERON_QUEUE;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateOrder {
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

impl CreateOrder {
    pub fn push_in_aeron_queue(self) {
        let pool = THREADPOOL_ORDER_AERON_QUEUE.lock().unwrap();
        pool.execute(move || {
            send_aeron_msg(StreamId::CreateOrder, self.serialize());
        });
        drop(pool);
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: &String) -> Self {
        let deserialized: CreateOrder = serde_json::from_str(json).unwrap();
        deserialized
    }
}
