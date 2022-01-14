#![allow(dead_code)]
#![allow(unused_variables)]
extern crate uuid;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub enum TXType {
    ORDERTX, //TraderOrder
    LENDTX,  //LendOrder
}
#[derive(Serialize, Deserialize, Debug)]
pub enum OrderType {
    LIMIT,
    MARKET,
    DARK,
    LEND,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum PositionType {
    LONG,
    SHORT,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum OrderStatus {
    SETTLED,
    LEND,
    LIQUIDATE,
    CANCELLED,
    PENDING,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TraderOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub position_type: PositionType,
    pub position_side: i32,
    pub order_status: OrderStatus, //SETTLED or LIQUIDATE or PENDING or FILLED
    pub order_type: OrderType,
    pub entryprice: f64,
    pub execution_price: f64,
    pub positionsize: f64,
    pub leverage: f64,
    pub initial_margin: f64,
    pub available_margin: f64,
    pub timestamp: u128,
    pub bankruptcy_price: f64,
    pub bankruptcy_value: f64,
    pub maintenance_margin: f64,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct LendOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub order_type: OrderType,     // LEND
    pub order_status: OrderStatus, //lend or settle
    pub nonce: i32,
    pub lend_amount: f64,
    pub timestamp: u128,
}

pub fn entryvalue(initial_margin: f64, leverage: f64) -> f64 {
    initial_margin * leverage
}
pub fn positionsize(entryvalue: f64, entryprice: f64) -> f64 {
    entryvalue * entryprice
}
pub fn bankruptcyprice(position_type: &PositionType, entryprice: f64, leverage: f64) -> f64 {
    match position_type {
        &PositionType::LONG => entryprice * leverage / (leverage + 1.0),
        &PositionType::SHORT => {
            if leverage > 1.0 {
                entryprice * leverage / (leverage - 1.0)
            } else {
                0.0
            }
        }
    }
}
pub fn bankruptcyvalue(positionsize: f64, bankruptcyprice: f64) -> f64 {
    if bankruptcyprice > 0.0 {
        positionsize / bankruptcyprice
    } else {
        0.0
    }
}
pub fn maintenancemargin(entryvalue: f64, bankruptcyvalue: f64, fee: f64, funding: f64) -> f64 {
    (0.4 * entryvalue + fee * bankruptcyvalue + funding * bankruptcyvalue) / 100.0
}

pub fn positionside(position_type: &PositionType) -> i32 {
    match position_type {
        &PositionType::LONG => -1,
        &PositionType::SHORT => 1,
    }
}

// need to create new order
// need to create recalculate order
// need to create settle order initial_margin
// impl for order -> new, recaculate, liquidate etc
// create function bankruptcy price, bankruptcy rate, liquidation price

impl TraderOrder {
    pub fn new(
        account_id: &str,
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        available_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
        execution_price: f64,
    ) -> Self {
        let position_side = positionside(&position_type);
        let entryvalue = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entryvalue, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fee = 0.002; //.2%
        let funding = 0.025; //2.5%
        let maintenance_margin = maintenancemargin(entryvalue, bankruptcy_value, fee, funding);
        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => TraderOrder {
                uuid: Uuid::new_v4(),
                account_id: String::from(account_id),
                position_type,
                position_side,
                order_status,
                order_type,
                entryprice,
                execution_price,
                positionsize,
                leverage,
                initial_margin,
                available_margin,
                timestamp: n.as_millis(),
                bankruptcy_price,
                bankruptcy_value,
                maintenance_margin,
            },
            Err(e) => panic!("Could not generate new order: {}", e),
        }
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }
    pub fn deserialize(json: &String) -> Self {
        let deserialized: TraderOrder = serde_json::from_str(json).unwrap();
        deserialized
    }
}
