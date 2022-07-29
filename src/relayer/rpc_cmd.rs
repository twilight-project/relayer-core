use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
// use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RpcCommand {
    CreateTraderOrder(CreateTraderOrder, Meta),
    CreateLendOrder(CreateLendOrder, Meta),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta),
    ExecuteLendOrder(ExecuteLendOrder, Meta),
    CancelTraderOrder(CancelTraderOrder, Meta),
}
