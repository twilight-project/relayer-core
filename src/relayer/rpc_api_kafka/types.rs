use crate::relayer::types::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TestLocaldb {
    pub orderid: String,
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

impl From<CreateTraderOrder>
    for (
        String,
        PositionType,
        OrderType,
        f64,
        f64,
        f64,
        OrderStatus,
        f64,
        f64,
    )
{
    fn from(
        e: CreateTraderOrder,
    ) -> (
        String,
        PositionType,
        OrderType,
        f64,
        f64,
        f64,
        OrderStatus,
        f64,
        f64,
    ) {
        let CreateTraderOrder {
            account_id,
            position_type,
            order_type,
            leverage,
            initial_margin,
            available_margin,
            order_status,
            entryprice,
            execution_price,
        } = e;
        (
            account_id,
            position_type,
            order_type,
            leverage,
            initial_margin,
            available_margin,
            order_status,
            entryprice,
            execution_price,
        )
    }
}
// impl From<Example> for (i32, String) {
//     fn from(e: Example) -> (i32, String) {
//         let Example { a, b } = e;
//         (a, b)
//     }
// }
