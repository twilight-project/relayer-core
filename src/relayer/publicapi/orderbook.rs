use crate::config::POSTGRESQL_POOL_CONNECTION;
use postgres::types::Timestamp;
use postgres::types::Type;

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FundingRow {
    pub rate: f64,
    pub price: f64,
    pub timestamp: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FundingHistory {
    pub funding: Vec<FundingRow>,
}

pub fn get_orderbook() {}

pub fn get_recent_trades() {} //get trades

pub fn get_funding_history() {} //price, rate and time
pub fn get_fudning_data_from_psql(limit: u32) {
    let query = format!(" Select fundingrate, price, timestamp from  public.fundingratehistory  Order By  timestamp desc Limit {} ;",limit);
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
    for row in client.query(&query, &[]).unwrap() {
        // let ttime: Timestamp::Value = row.get("timestamp");
        // println!(
        //     "Data : {:#?}",
        //     FundingRow {
        //         rate: row.get("fundingrate"),
        //         price: row.get("price"),
        //         timestamp: 0
        //     },
        // ttime
        // );
        println!("data:{:#?}", row);
    }
}
