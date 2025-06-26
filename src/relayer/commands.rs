use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
use zkvm::Output;
pub type ZkosHexString = String;
pub type RequestID = String;

use jsonrpc_http_server::jsonrpc_core::Metadata;
use std::collections::HashMap;

#[derive(Default, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Meta {
    pub metadata: HashMap<String, Option<String>>,
}
impl Metadata for Meta {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FeeUpdate {
    pub order_filled_on_market: f64,
    pub order_filled_on_limit: f64,
    pub order_settled_on_market: f64,
    pub order_settled_on_limit: f64,
}
impl FeeUpdate {
    pub fn to_relayer_command(&self) -> RelayerCommand {
        RelayerCommand::UpdateFees(
            self.order_filled_on_market,
            self.order_filled_on_limit,
            self.order_settled_on_market,
            self.order_settled_on_limit,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FeeType {
    FilledOnMarket,
    FilledOnLimit,
    SettledOnMarket,
    SettledOnLimit,
}
impl Into<String> for FeeType {
    fn into(self) -> String {
        match self {
            FeeType::FilledOnMarket => "FilledOnMarket".to_string(),
            FeeType::FilledOnLimit => "FilledOnLimit".to_string(),
            FeeType::SettledOnMarket => "SettledOnMarket".to_string(),
            FeeType::SettledOnLimit => "SettledOnLimit".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelayerCommand {
    FundingCycle(PoolBatchOrder, Meta, f64),
    FundingOrderEventUpdate(TraderOrder, Meta),
    PriceTickerLiquidation(Vec<Uuid>, Meta, f64),
    PriceTickerOrderFill(Vec<Uuid>, Meta, f64), //no update for lend pool
    PriceTickerOrderSettle(Vec<Uuid>, Meta, f64),
    FundingCycleLiquidation(Vec<Uuid>, Meta, f64),
    RpcCommandPoolupdate(),
    UpdateFees(f64, f64, f64, f64), //order_filled_on_market, order_filled_on_limit, order_settled_on_limit, order_settled_on_market
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder, Meta, ZkosHexString, RequestID),
    CreateLendOrder(CreateLendOrder, Meta, ZkosHexString, RequestID),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta, ZkosHexString, RequestID),
    ExecuteLendOrder(ExecuteLendOrder, Meta, ZkosHexString, RequestID),
    CancelTraderOrder(CancelTraderOrder, Meta, ZkosHexString, RequestID),
    RelayerCommandTraderOrderSettleOnLimit(TraderOrder, Meta, f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SortedSetCommand {
    AddLiquidationPrice(Uuid, f64, PositionType),
    AddOpenLimitPrice(Uuid, f64, PositionType),
    AddCloseLimitPrice(Uuid, f64, PositionType),
    RemoveLiquidationPrice(Uuid, PositionType),
    RemoveOpenLimitPrice(Uuid, PositionType),
    RemoveCloseLimitPrice(Uuid, PositionType),
    UpdateLiquidationPrice(Uuid, f64, PositionType),
    UpdateOpenLimitPrice(Uuid, f64, PositionType),
    UpdateCloseLimitPrice(Uuid, f64, PositionType),
    BulkSearchRemoveLiquidationPrice(f64, PositionType),
    BulkSearchRemoveOpenLimitPrice(f64, PositionType),
    BulkSearchRemoveCloseLimitPrice(f64, PositionType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PositionSizeLogCommand {
    AddPositionSize(PositionType, f64),
    RemovePositionSize(PositionType, f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ZkosTxCommand {
    CreateTraderOrderTX(TraderOrder, RpcCommand),
    CreateTraderOrderLIMITTX(TraderOrder, Option<ZkosHexString>),
    CreateLendOrderTX(LendOrder, RpcCommand, zkvm::Output, zkvm::Output),
    ExecuteTraderOrderTX(TraderOrder, RpcCommand, zkvm::Output, zkvm::Output),
    ExecuteLendOrderTX(LendOrder, RpcCommand, zkvm::Output, zkvm::Output),
    CancelTraderOrderTX(TraderOrder, RpcCommand),
    RelayerCommandTraderOrderSettleOnLimitTX(
        TraderOrder,
        Option<ZkosHexString>,
        zkvm::Output,
        zkvm::Output,
    ),
    RelayerCommandTraderOrderLiquidateTX(TraderOrder, Option<Output>, zkvm::Output, zkvm::Output),
}

impl RpcCommand {
    pub fn zkos_msg(&self) -> String {
        match self.clone() {
            RpcCommand::CreateTraderOrder(
                create_trader_order,
                meta,
                zkos_hex_string,
                _request_id,
            ) => zkos_hex_string.clone(),
            RpcCommand::CreateLendOrder(create_lend_order, meta, zkos_hex_string, _request_id) => {
                zkos_hex_string.clone()
            }
            RpcCommand::ExecuteTraderOrder(
                execute_trader_order,
                meta,
                zkos_hex_string,
                _request_id,
            ) => zkos_hex_string.clone(),
            RpcCommand::ExecuteLendOrder(
                execute_lend_order,
                meta,
                zkos_hex_string,
                _request_id,
            ) => zkos_hex_string.clone(),
            RpcCommand::CancelTraderOrder(
                cancel_trader_order,
                meta,
                zkos_hex_string,
                _request_id,
            ) => zkos_hex_string.clone(),
            _ => "".to_string(),
        }
    }
}
