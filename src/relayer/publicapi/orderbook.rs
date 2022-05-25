use crate::config::POSTGRESQL_POOL_CONNECTION;
extern crate rust_decimal;
extern crate rust_decimal_macros;
use postgres::types::Timestamp;
use postgres::types::Type;
use rust_decimal::prelude::*;
// use rust_decimal_macros::dec;
use rust_decimal::Decimal;

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
    pub rate: Decimal,
    pub price: Decimal,
    pub timestamp: std::time::SystemTime,
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
        // let ttime: std::time::SystemTime = row.get("timestamp");
        println!(
            "Data : {:#?}",
            FundingRow {
                rate: row.get("fundingrate"),
                price: row.get("price"),
                timestamp: row.get("timestamp")
            },
        );
        println!(
            "Data 2 : {:#?}",
            serde_json::to_string(&FundingRow {
                rate: row.get("fundingrate"),
                price: row.get("price"),
                timestamp: row.get("timestamp")
            })
            .unwrap()
        );
        let timestamp: std::time::SystemTime = row.get("timestamp");
        let price: Decimal = row.get("fundingrate");
        let fprice = price.to_f64().unwrap();
        println!("data:{:#?}", iso8601(&timestamp));
        println!(
            "data:{:#?}",
            timestamp
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
        );
    }
}

use chrono::prelude::{DateTime, Utc};
//https://stackoverflow.com/questions/64146345/how-do-i-convert-a-systemtime-to-iso-8601-in-rust
fn iso8601(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    format!("{}", dt.format("%+"))
    // formats like "2001-07-08T00:34:60.026490+09:30"
}
