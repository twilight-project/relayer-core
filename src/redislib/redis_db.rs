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
use std::process::Command;

/// use to set key/value in redis
/// return type bool
pub fn set(key: &str, value: &str) -> bool {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .query::<String>(&mut *conn)
        .unwrap();
    // .query(&mut conn)
    // .expect("failed to execute SET for 'foo'");
    return true;
}

/// use to get value in redis
/// return type bool
pub fn get(key: &str) -> String {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return match redis::cmd("GET").arg(key).query::<String>(&mut *conn) {
        Ok(s) => s,
        Err(_) => String::from("key not found"),
    };
}

pub fn get_type_f64(key: &str) -> f64 {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return match redis::cmd("GET").arg(key).query::<f64>(&mut *conn) {
        Ok(s) => s,
        // Err(_) => String::from("key not found"),
        Err(_) => 0.0,
    };
}

/// `save_redis_backup` can copy redis backup created in redis container to host system
///
/// ###Example:
///
///```rust,no_run
/// use crate::twilight_relayer_rust::redislib::redis_db::save_redis_backup;
///save_redis_backup(String::from("src/redislib/."));
/// ```
///Or to make a call from main.rs
///```rust,no_run
/// use crate::twilight_relayer_rust::redislib::redis_db;
/// redis_db::save_redis_backup(String::from("src/redislib/backup/."));
/// ```
pub fn save_redis_backup(filepath: String) {
    let mut cmd_backup = Command::new("docker");
    cmd_backup.arg("cp");
    cmd_backup.arg("redis:/data/dump.rdb");
    // cmd_backup.arg("src/redislib/.");
    cmd_backup.arg(filepath);
    cmd_backup.output().expect("process failed to execute");
}

pub fn zadd(key: &str, value: &str, score: &str) -> bool {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("ZADD")
        .arg(key)
        .arg(score)
        .arg(value)
        .query::<i32>(&mut *conn)
        .unwrap();
    // .query(&mut conn)
    // .expect("failed to execute SET for 'foo'");
    return true;
}

pub fn del(key: &str) -> bool {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("DEL").arg(key).query::<i32>(&mut *conn).unwrap();
    return true;
}

pub fn zdel(key: &str, member: &str) -> bool {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("ZREM")
        .arg(key)
        .arg(member)
        .query::<i32>(&mut *conn)
        .unwrap();
    return true;
}
//INCRBYFLOAT
pub fn incrbyfloat(key: &str, value: &str) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(value)
        .query::<String>(&mut *conn)
        .unwrap();
}
//DECRBYFLOAT
pub fn decrbyfloat(key: &str, value: f64) {
    let negvalue = format!("{}", -1.0 * value);
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(negvalue)
        .query::<String>(&mut *conn)
        .unwrap();
}

//INCRBYFLOAT type f64
pub fn incrbyfloat_type_f64(key: &str, value: f64) -> f64 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let updated_value = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(value.to_string())
        .query::<f64>(&mut *conn)
        .unwrap();
    return updated_value;
}
//DECRBYFLOAT type f64
pub fn decrbyfloat_type_f64(key: &str, value: f64) -> f64 {
    let negvalue = format!("{}", (-1.0 * value).to_string());
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let updated_value = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(negvalue)
        .query::<f64>(&mut *conn)
        .unwrap();
    return updated_value;
}

/// list of settling orders for ordertype Limit and Position type Long
pub fn zrangelonglimitorderbyexecutionprice(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrderbyLONGLimit")
        .arg("0")
        .arg(format!("({}", current_price))
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}
/// list of settling orders for ordertype Limit and Position type Short
pub fn zrangeshortlimitorderbyexecutionprice(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrderbySHORTLimit")
        .arg(format!("({}", current_price))
        .arg("+inf")
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

/// list of liquidating order for position type long
pub fn zrangegetliquidateorderforlong(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrderbyLiquidationPriceFORLong")
        .arg(format!("({}", current_price))
        .arg("+inf")
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

/// list of liquidating order for position type short
pub fn zrangegetliquidateorderforshort(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrderbyLiquidationPriceFORShort")
        .arg("0")
        .arg(format!("({}", current_price))
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

/// get list of all open orderid
pub fn zrangeallopenorders() -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrder")
        .arg("0")
        .arg("-1")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

pub fn incr_lend_nonce_by_one() -> u128 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let i = redis::cmd("INCR")
        .arg("LendNonce")
        .query::<u128>(&mut *conn)
        .unwrap();
    return i;
}

pub fn get_nonce_u128() -> u128 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    return match redis::cmd("GET").arg("LendNonce").query::<u128>(&mut *conn) {
        Ok(s) => s,
        // Err(_) => String::from("key not found"),
        Err(_) => 0,
    };
}

//get_entry_sequence_u128

pub fn get_entry_sequence_u128() -> u128 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    return match redis::cmd("GET")
        .arg("EntrySequence")
        .query::<u128>(&mut *conn)
    {
        Ok(s) => s,
        // Err(_) => String::from("key not found"),
        Err(_) => 0,
    };
}

pub fn incr_entry_sequence_by_one() -> u128 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let i = redis::cmd("INCR")
        .arg("EntrySequence")
        .query::<u128>(&mut *conn)
        .unwrap();
    return i;
}
// command detail https://stackoverflow.com/questions/20017255/how-to-get-a-member-with-maximum-or-minimum-score-from-redis-sorted-set-given/22052718
//ZREVRANGEBYSCORE myset +inf -inf WITHSCORES LIMIT 0 1 //taking without score
/// get lender with maximum lendstate/deposit
/// returns orderid order lendtx
pub fn getbestlender() -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let i = redis::cmd("ZREVRANGEBYSCORE")
        .arg("LendOrderbyDepositLendState")
        .arg("+inf")
        .arg("-inf")
        .arg("LIMIT")
        .arg("0")
        .arg("1")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
    return i;
}

pub fn getbestlendertest(min_payment: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let i = redis::cmd("ZREVRANGEBYSCORE")
        .arg("TraderOrderbyLONGLimit")
        .arg("+inf")
        .arg(min_payment.to_string())
        .arg("withscores")
        .arg("LIMIT")
        .arg("0")
        .arg("30")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
    return i;
}

// ZINCRBY myzset 2 "one"
pub fn zincr_lend_pool_account(orderid: &str, payment: f64) -> f64 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let i = redis::cmd("ZINCRBY")
        .arg("LendOrderbyDepositLendState")
        .arg(payment.to_string())
        .arg(orderid)
        .query::<f64>(&mut *conn)
        .unwrap();
    return i;
}
// ZINCRBY myzset 2 "one"
pub fn zdecr_lend_pool_account(orderid: &str, payment: f64) -> f64 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let negative_payment = format!("{}", (-1.0 * payment).to_string());
    let i = redis::cmd("ZINCRBY")
        .arg("LendOrderbyDepositLendState")
        .arg(negative_payment)
        .arg(orderid)
        .query::<f64>(&mut *conn)
        .unwrap();
    return i;
}

/// list of pending order for filling for position type short
pub fn zrangegetpendinglimitorderforlong(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Long")
        .arg(format!("[{}", current_price))
        .arg("+inf")
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

/// list of pending order for filling for position type Long
pub fn zrangegetpendinglimitorderforshort(current_price: f64) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrder_LimitOrder_Pending_FOR_Short")
        .arg("0")
        .arg(format!("[{}", current_price))
        .arg("byscore")
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}

// for testing delete all tables
pub fn del_test() -> i32 {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("DEL")
        .arg("TraderOrderbyLONGLimit")
        .arg("TraderOrderbySHORTLimit")
        .arg("TraderOrderbyLiquidationPriceFORLong")
        .arg("TraderOrderbyLiquidationPriceFORShort")
        .arg("TraderOrder")
        // .arg("LendNonce")
        .arg("LendOrderbyDepositLendState")
        .arg("Fee")
        .arg("FundingRate")
        .arg("CurrentPrice")
        .arg("tlv")
        .arg("tps")
        .arg("TotalShortPositionSize")
        .arg("TotalLongPositionSize")
        .arg("TotalPoolPositionSize")
        // .arg("EntrySequence")
        .query::<i32>(&mut *conn)
        .unwrap();
    // .query(&mut conn)
    // .expect("failed to execute SET for 'foo'");
    // return true;
}

// for test
pub fn zrank_test(orderid: &str, table: &str) -> i32 {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    return match redis::cmd("ZRANK")
        .arg(table)
        .arg(orderid)
        .query::<i32>(&mut *conn)
    {
        Ok(s) => s,
        Err(_) => -1,
    };
}
