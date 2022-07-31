use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
// use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelayerCommand {
    FundingCycle(CreateTraderOrder, Meta),
    PriceTickerLiquidation(CreateLendOrder, Meta),
    PriceTickerOrderFill(ExecuteTraderOrder, Meta),
    PriceTickerOrderSettle(ExecuteLendOrder, Meta),
    FundingCycleLiquidation(CancelTraderOrder, Meta),
}
