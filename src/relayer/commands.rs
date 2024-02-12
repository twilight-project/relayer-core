use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
pub type ZkosHexString = String;
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelayerCommand {
    FundingCycle(PoolBatchOrder, Meta, f64),
    FundingOrderEventUpdate(TraderOrder, Meta),
    PriceTickerLiquidation(Vec<Uuid>, Meta, f64),
    PriceTickerOrderFill(Vec<Uuid>, Meta, f64), //no update for lend pool
    PriceTickerOrderSettle(Vec<Uuid>, Meta, f64),
    FundingCycleLiquidation(Vec<Uuid>, Meta, f64),
    RpcCommandPoolupdate(),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder, Meta, ZkosHexString),
    CreateLendOrder(CreateLendOrder, Meta, ZkosHexString),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta, ZkosHexString),
    ExecuteLendOrder(ExecuteLendOrder, Meta, ZkosHexString),
    CancelTraderOrder(CancelTraderOrder, Meta, ZkosHexString),
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
    RelayerCommandTraderOrderLiquidateTX(
        TraderOrder,
        Option<ZkosHexString>,
        zkvm::Output,
        zkvm::Output,
    ),
}

impl RpcCommand {
    pub fn zkos_msg(&self) -> String {
        match self.clone() {
            RpcCommand::CreateTraderOrder(create_trader_order, meta, zkos_hex_string) => {
                zkos_hex_string.clone()
            }
            RpcCommand::CreateLendOrder(create_lend_order, meta, zkos_hex_string) => {
                zkos_hex_string.clone()
            }
            RpcCommand::ExecuteTraderOrder(execute_trader_order, meta, zkos_hex_string) => {
                zkos_hex_string.clone()
            }
            RpcCommand::ExecuteLendOrder(execute_lend_order, meta, zkos_hex_string) => {
                zkos_hex_string.clone()
            }
            RpcCommand::CancelTraderOrder(cancel_trader_order, meta, zkos_hex_string) => {
                zkos_hex_string.clone()
            }
            _ => "".to_string(),
        }
    }
}
