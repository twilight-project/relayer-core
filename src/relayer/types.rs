use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TXType {
    ORDERTX, //TraderOrder
    LENDTX,  //LendOrder
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderType {
    LIMIT,
    MARKET,
    DARK,
    LEND,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PositionType {
    LONG,
    SHORT,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderStatus {
    SETTLED,
    LENDED,
    LIQUIDATE,
    CANCELLED,
    PENDING, // change it to New
    FILLED,  //executed on price ticker
}
