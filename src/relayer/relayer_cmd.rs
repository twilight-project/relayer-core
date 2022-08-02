use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
// use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelayerCommand {
    FundingCycle(PoolOrder, Meta),
    PriceTickerLiquidation(PoolOrder, Meta),
    PriceTickerOrderFill(PoolOrder, Meta),
    PriceTickerOrderSettle(PoolOrder, Meta),
    FundingCycleLiquidation(PoolOrder, Meta),
    RpcCommandPoolupdate(PoolOrder, Meta),
    InitiateNewPool(PoolOrder, Meta),
}
