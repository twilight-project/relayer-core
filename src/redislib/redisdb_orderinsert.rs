#![allow(dead_code)]
use crate::config::REDIS_POOL_CONNECTION;
use crate::relayer::*;
use r2d2_redis::redis;

pub fn longorderinsert_pipeline(ordertx: TraderOrder, key_array: Vec<String>) -> (u128, u128) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (lend_nonce, entrysequence): (u128, u128) = redis::pipe()
        .cmd("GET")
        .arg("LendNonce")
        .cmd("INCR")
        .arg("EntrySequence_TraderOrder")
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
    // println!("key_array: {:#?}", key_array);

    return (lend_nonce, entrysequence);
}

pub fn longorderinsert_pipeline_second(ordertx: TraderOrder) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let (_status, _entrysequence): (bool, i32) = redis::pipe()
        .cmd("SET")
        .arg(&ordertx.uuid.to_string())
        .arg(&ordertx.serialize())
        .cmd("ZADD")
        .arg("TraderOrder")
        .arg(&ordertx.entry_sequence.to_string())
        .arg(&ordertx.uuid.to_string())
        .query(&mut *conn)
        .unwrap();
}

// redis_db::set(&ordertx.uuid.to_string(), &ordertx.serialize());
// redis_db::zadd(
//     &"TraderOrder",
//     &ordertx.uuid.to_string(),           //value
//     &ordertx.entry_sequence.to_string(), //score
// );
