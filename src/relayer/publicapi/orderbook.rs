use crate::config::POSTGRESQL_POOL_CONNECTION;
extern crate rust_decimal;
extern crate rust_decimal_macros;
extern crate uuid;
use crate::redislib::redis_db_orderbook;
use redis_db_orderbook::RedisBulkOrderdata;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Side {
    BUY,
    SELL,
}

impl Into<u32> for Side {
    fn into(self) -> u32 {
        match self {
            Side::SELL => 0,
            Side::BUY => 1,
        }
    }
}
impl Into<u8> for Side {
    fn into(self) -> u8 {
        match self {
            Side::SELL => 0,
            Side::BUY => 1,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub bid: Vec<Bid>,
    pub ask: Vec<Ask>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bid {
    pub positionsize: f64,
    pub price: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ask {
    pub positionsize: f64,
    pub price: f64,
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

pub fn get_latest_orderbook() -> String {
    let order_list: RedisBulkOrderdata = redis_db_orderbook::getlimitorderszscore();
    // let mut array: Vec<String>;
    let order_list_clone = order_list.clone();
    let mut array = Vec::new();
    for data in order_list_clone.short_orderid_to_fill.vec {
        array.push(data.value);
    }
    for data in order_list_clone.long_orderid_to_fill.vec {
        array.push(data.value);
    }
    for data in order_list_clone.short_orderid_to_settle.vec {
        array.push(data.value);
    }
    for data in order_list_clone.long_orderid_to_settle.vec {
        array.push(data.value);
    }
    let mut bid = Vec::new();
    let mut ask = Vec::new();
    if array.len() > 0 {
        let orderdb = redis_db_orderbook::mget_order_hashmap(array);
        for data in order_list.short_orderid_to_fill.vec {
            ask.push(Ask {
                positionsize: orderdb
                    .get(&data.value.parse::<Uuid>().unwrap())
                    .unwrap()
                    .positionsize,
                price: data.score.parse::<f64>().unwrap(),
            });
        }
        for data in order_list.long_orderid_to_fill.vec {
            bid.push(Bid {
                positionsize: orderdb
                    .get(&data.value.parse::<Uuid>().unwrap())
                    .unwrap()
                    .positionsize,
                price: data.score.parse::<f64>().unwrap(),
            });
        }
        for data in order_list.short_orderid_to_settle.vec {
            bid.push(Bid {
                positionsize: orderdb
                    .get(&data.value.parse::<Uuid>().unwrap())
                    .unwrap()
                    .positionsize,
                price: data.score.parse::<f64>().unwrap(),
            });
        }
        for data in order_list.long_orderid_to_settle.vec {
            ask.push(Ask {
                positionsize: orderdb
                    .get(&data.value.parse::<Uuid>().unwrap())
                    .unwrap()
                    .positionsize,
                price: data.score.parse::<f64>().unwrap(),
            });
        }
    }
    let orderbook = OrderBook { bid: bid, ask: ask };
    // println!("orderbook {}", serde_json::to_string(&orderbook).unwrap());
    return serde_json::to_string(&orderbook).unwrap();
}
