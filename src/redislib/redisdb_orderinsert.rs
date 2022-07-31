#![allow(dead_code)]
use crate::config::REDIS_POOL_CONNECTION;
use crate::relayer::*;
use r2d2_redis::redis;

pub fn orderinsert_pipeline() -> (usize, usize) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (lend_nonce, entrysequence): (usize, usize) = redis::pipe()
        .cmd("GET")
        .arg("LendNonce")
        .cmd("INCR")
        .arg("EntrySequence_TraderOrder")
        .query(&mut *conn)
        .unwrap();
    // println!("key_array: {:#?}", key_array);

    return (lend_nonce, entrysequence);
}

pub fn orderinsert_pipeline_second(ordertx: TraderOrder, key_array: Vec<String>) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (_status, _entrysequence): (bool, i32) = redis::pipe()
        .cmd("SET")
        .arg(&ordertx.uuid.to_string())
        .arg(&ordertx.serialize())
        .cmd("ZADD")
        .arg("TraderOrder")
        .arg(&ordertx.entry_sequence.to_string())
        .arg(&ordertx.uuid.to_string())
        .cmd("INCRBYFLOAT")
        .arg(key_array[0].clone())
        .arg(&ordertx.positionsize.to_string())
        .ignore()
        .cmd("ZADD")
        .arg(key_array[1].clone())
        .arg(&ordertx.liquidation_price.to_string())
        .arg(&ordertx.uuid.to_string())
        .ignore()
        .cmd("INCRBYFLOAT")
        .arg(key_array[2].clone())
        .arg(&ordertx.positionsize.to_string())
        .ignore()
        .query(&mut *conn)
        .unwrap();
}

pub fn orderinsert_pipeline_pending(ordertx: TraderOrder, key_array: String) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (_status, _entrysequence): (bool, i32) = redis::pipe()
        .cmd("SET")
        .arg(&ordertx.uuid.to_string())
        .arg(&ordertx.serialize())
        .cmd("ZADD")
        .arg(key_array)
        .arg(&ordertx.entryprice.to_string())
        .arg(&ordertx.uuid.to_string())
        .query(&mut *conn)
        .unwrap();
}

pub fn order_remove_from_redis_pipeline(ordertx: TraderOrder, key_array: Vec<String>) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (_status, _status2): (i32, f64) = redis::pipe()
        .cmd("DEL")
        .arg(&ordertx.uuid.to_string())
        .cmd("ZREM")
        .arg("TraderOrder")
        .arg(&ordertx.uuid.to_string())
        .ignore()
        .cmd("ZREM")
        .arg(key_array[0].clone())
        .arg(&ordertx.uuid.to_string())
        .ignore()
        .cmd("INCRBYFLOAT")
        .arg(key_array[1].clone())
        .arg(format!("{}", -1.0 * ordertx.positionsize))
        .cmd("ZREM")
        .arg(key_array[2].clone())
        .arg(&ordertx.uuid.to_string())
        .ignore()
        .cmd("INCRBYFLOAT")
        .arg("TotalPoolPositionSize")
        .arg(format!("{}", -1.0 * ordertx.positionsize))
        .ignore()
        .query(&mut *conn)
        .unwrap();
}
// redis data update on funding cycle
pub fn funding_order_liquidation_price_update(ordertx: TraderOrder, key_array: String) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (_status, _entrysequence): (bool, i32) = redis::pipe()
        .cmd("SET")
        .arg(&ordertx.uuid.to_string())
        .arg(&ordertx.serialize())
        .cmd("ZADD")
        .arg(key_array)
        .arg(&ordertx.liquidation_price.to_string())
        .arg(&ordertx.uuid.to_string())
        .query(&mut *conn)
        .unwrap();
}

pub fn order_insert_hmset(ordertx: TraderOrder) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("HMSET")
        .arg(ordertx.uuid.clone().to_string())
        .arg(ordertx.to_hmset_arg_array())
        .query::<String>(&mut *conn)
        .unwrap();

    // return true;

    // let mut hcmd = redis::cmd("HMSET");
    // for arg in ordertx.to_hmset_arg_array() {
    //     hcmd.arg(arg);
    // }
    // hcmd.query::<String>(&mut *conn).unwrap();
}

pub fn order_get_hgetall(key: String) -> Result<TraderOrder, std::io::Error> {
    // pub fn order_get_hgetall(key: String) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    let order: Vec<String> = redis::cmd("HGETALL")
        .arg(key)
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
    TraderOrder::from_hgetll_trader_order(order)
    // println!("order get from redis{:#?}", order);
}
