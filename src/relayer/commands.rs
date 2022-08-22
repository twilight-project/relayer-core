use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelayerCommand {
    FundingCycle(PoolBatchOrder, Meta),
    PriceTickerLiquidation(Vec<Uuid>, Meta, f64),
    PriceTickerOrderFill(Vec<Uuid>, Meta, f64), //no update for lend pool
    PriceTickerOrderSettle(Vec<Uuid>, Meta, f64),
    FundingCycleLiquidation(PoolBatchOrder, Meta),
    RpcCommandPoolupdate(PoolBatchOrder, Meta),
    AddTraderOrderToBatch(TraderOrder, RpcCommand, Meta, f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder, Meta),
    CreateLendOrder(CreateLendOrder, Meta),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta),
    ExecuteLendOrder(ExecuteLendOrder, Meta),
    CancelTraderOrder(CancelTraderOrder, Meta),
    // Liquidation(TraderOrder, Meta),
}
