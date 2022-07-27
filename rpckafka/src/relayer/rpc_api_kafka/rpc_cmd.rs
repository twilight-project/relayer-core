use crate::relayer::rpc_api_kafka::types::{
    CancelTraderOrder, CreateLendOrder, CreateTraderOrder, ExecuteLendOrder, ExecuteTraderOrder,
};
use crate::relayer::types::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder),
    CreateLendOrder(CreateLendOrder),
    ExecuteTraderOrder(ExecuteTraderOrder),
    ExecuteLendOrder(ExecuteLendOrder),
    CancelTraderOrder(CancelTraderOrder),
    NewOrder {
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
    },
    OpenLimit {
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
    },
    Liquidate {
        liquidation_price: f64,
        available_margin: f64,
        nonce: u128,
    },
    CancelOrder {
        uuid: Uuid,
        order_type: OrderType,
        order_status: OrderStatus,
    },
    CloseMarket {
        uuid: Uuid,
        order_type: OrderType,
        order_status: OrderStatus,
        execution_price: f64,
    },
}
