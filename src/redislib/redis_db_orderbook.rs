//! ## redis_db
//!redis_db provide two main redis function `get` and `set` key-value pairs in redis database.
//!
//! ### Examples
//!   
//! Basic usage:
//!
//! ```rust,no_run
//! use crate::twilight_relayer_rust::redislib::redis_db;
//!
//! fn main() {
//! 	redis_db::set(&"name", &"John");
//! 	println!("{}", redis_db::get(&"name")); //output: John
//! }
//! ```

#![allow(dead_code)]
// extern crate redis;
// extern crate stopwatch;
use crate::config::REDIS_POOL_CONNECTION;
use r2d2_redis::redis;
// use std::process::Command;

pub fn mget_orderstring(key_array: Vec<String>) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    let array: Vec<String>;
    //  = Vec::new();
    array = redis::cmd("MGET").arg(key_array).query(&mut *conn).unwrap();
    // for key in key_array {
    //     trans_query = trans_query.arg(key);
    // }
    // trans_query = trans_query.query(&mut *conn).unwrap();

    array
}
pub fn getlimitorders() -> Vec<Vec<String>> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (
        short_orderid_to_fill,
        long_orderid_to_fill,
        short_orderid_to_settle,
        long_orderid_to_settle,
    ): (Vec<String>, Vec<String>, Vec<String>, Vec<String>) = redis::pipe()
        .cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Short")
        .arg("0")
        .arg("-1")
        .cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Long")
        .arg("0")
        .arg("-1")
        .cmd("ZRANGE")
        .arg("TraderOrder_Settelment_by_SHORT_Limit")
        .arg("0")
        .arg("-1")
        .cmd("ZRANGE")
        .arg("TraderOrder_Settelment_by_LONG_Limit")
        .arg("0")
        .arg("-1")
        .query(&mut *conn)
        .unwrap();
    // println!("d1: {:#?}, d2: {:#?}, d3: {:#?}, d4: {:#?}", k1, k2, k3, k4);

    let (short_order_to_fill, long_order_to_fill, short_order_to_settle, long_order_to_settle): (
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
    ) = redis::pipe()
        .cmd("MGET")
        .arg(short_orderid_to_fill)
        .cmd("MGET")
        .arg(long_orderid_to_fill)
        .cmd("MGET")
        .arg(short_orderid_to_settle)
        .cmd("MGET")
        .arg(long_orderid_to_settle)
        .query(&mut *conn)
        .unwrap();

    return vec![
        short_order_to_fill,
        long_order_to_fill,
        short_order_to_settle,
        long_order_to_settle,
    ];
}

use serde_derive::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Data {
    pub score: String,
    pub value: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ZrangeWithScore {
    pub vec: Vec<Data>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RedisBulkOrderdata {
    pub short_orderid_to_fill: ZrangeWithScore,
    pub long_orderid_to_fill: ZrangeWithScore,
    pub short_orderid_to_settle: ZrangeWithScore,
    pub long_orderid_to_settle: ZrangeWithScore,
}

use self::redis::{from_redis_value, FromRedisValue, RedisResult, Value};
impl FromRedisValue for ZrangeWithScore {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let json_str: Vec<String> = from_redis_value(v)?;
        let mut result_array: Vec<Data> = vec![
            Data {
                score: "00".to_string(),
                value: "0.00".to_string(),
            };
            json_str.len() / 2
        ];
        let mut j = 0;
        for (i, el) in json_str.iter().enumerate() {
            if (i) % 2 == 0 {
                result_array[j].value = el.to_string();
            } else {
                result_array[j].score = el.to_string();
                j = j + 1;
            }
        }
        Ok(ZrangeWithScore { vec: result_array })
    }
}
pub fn getlimitorderszscore() -> RedisBulkOrderdata {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (
        short_orderid_to_fill,
        long_orderid_to_fill,
        short_orderid_to_settle,
        long_orderid_to_settle,
    ): (
        ZrangeWithScore,
        ZrangeWithScore,
        ZrangeWithScore,
        ZrangeWithScore,
    ) = redis::pipe()
        .cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Short")
        .arg("0")
        .arg("-1")
        .arg("WITHSCORES")
        .cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Long")
        .arg("0")
        .arg("-1")
        .arg("WITHSCORES")
        .cmd("ZRANGE")
        .arg("TraderOrder_Settelment_by_SHORT_Limit")
        .arg("0")
        .arg("-1")
        .arg("WITHSCORES")
        .cmd("ZRANGE")
        .arg("TraderOrder_Settelment_by_LONG_Limit")
        .arg("0")
        .arg("-1")
        .arg("WITHSCORES")
        .query(&mut *conn)
        .unwrap();

    return RedisBulkOrderdata {
        short_orderid_to_fill,
        long_orderid_to_fill,
        short_orderid_to_settle,
        long_orderid_to_settle,
    };
}

use crate::relayer::TraderOrder;
extern crate uuid;
use std::collections::HashMap;
use uuid::Uuid;
#[derive(Clone, PartialEq)]
pub struct MGetOrdersForRedis {
    pub orderdb: HashMap<Uuid, TraderOrder>,
}

impl FromRedisValue for MGetOrdersForRedis {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let json_str: Vec<String> = from_redis_value(v)?;

        let mut order_db: HashMap<Uuid, TraderOrder> = HashMap::new();
        for order_data in json_str {
            let order_data_deser = TraderOrder::deserialize(&order_data);
            order_db.insert(order_data_deser.uuid, order_data_deser);
        }
        Ok(MGetOrdersForRedis { orderdb: order_db })
    }
}

pub fn mget_order_hashmap(key_array: Vec<String>) -> HashMap<Uuid, TraderOrder> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    // let array: Vec<String>;
    // let mut order_db: HashMap<Uuid, TraderOrder> = HashMap::new();
    //  = Vec::new();
    let order_db = redis::cmd("MGET")
        .arg(key_array)
        .query::<MGetOrdersForRedis>(&mut *conn)
        .unwrap();
    // for key in key_array {
    //     trans_query = trans_query.arg(key);
    // }
    // trans_query = trans_query.query(&mut *conn).unwrap();

    order_db.orderdb
}
