use crate::relayer::types::*;
use crate::relayer::Meta;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder, Meta),
    CreateLendOrder(CreateLendOrder, Meta),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta),
    ExecuteLendOrder(ExecuteLendOrder, Meta),
    CancelTraderOrder(CancelTraderOrder, Meta),
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
        nonce: usize,
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
