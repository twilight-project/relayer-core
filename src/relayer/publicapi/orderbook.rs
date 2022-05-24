use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Side {
    BUY,
    SELL,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PendingTrade {
    pub side: Side,
    pub positionsize: f64,
    pub price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CloseTrade {
    pub side: Side,
    pub positionsize: f64,
    pub price: f64,
    pub timestamp: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderBook {
    pub bid: Vec<PendingTrade>,
    pub ask: Vec<PendingTrade>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RecentOrders {
    pub orders: Vec<CloseTrade>,
}

pub fn get_orderbook() {}

pub fn get_recent_trades() {} //get trades
                              //size , price, time

pub fn get_funding_history() {}
//price, rate and time
