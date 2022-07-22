use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TXType {
    ORDERTX, //TraderOrder
    LENDTX,  //LendOrder
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderType {
    LIMIT,
    MARKET,
    DARK,
    LEND,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PositionType {
    LONG,
    SHORT,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderStatus {
    SETTLED,
    LENDED,
    LIQUIDATE,
    CANCELLED,
    PENDING, // change it to New
    FILLED,  //executed on price ticker
}

// use std::fmt::{Debug, Display, Formatter, Result};
// impl Display for PositionType {
//     fn fmt(&self, f: &mut Formatter) -> Result {
//         write!(f, "{}", *self)
//     }
// }
// impl Display for OrderType {
//     fn fmt(&self, f: &mut Formatter) -> Result {
//         write!(f, "{}", *self)
//     }
// }
// impl Display for OrderStatus {
//     fn fmt(&self, f: &mut Formatter) -> Result {
//         write!(f, "{}", *self)
//     }
// }
// impl Display for TXType {
//     fn fmt(&self, f: &mut Formatter) -> Result {
//         write!(f, "{}", *self)
//     }
// }
