//! ## The BTC Price Feeder
//!This function is taking payload string received from binance websocket and updating the btc price into redisDB having keys "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.

#![allow(dead_code)]

use crate::config::{
    BinanceMiniTickerPayload,
    POSTGRESQL_POOL_CONNECTION,
    // THREADPOOL_PSQL_SEQ_QUEUE,
    THREADPOOL_REDIS_SEQ_QUEUE,
};
// use crate::kafkalib::producer_kafka;
use crate::redislib::redis_db;
use crate::relayer::set_localdb;
// use std::thread;

/// BTC Price updater
///  calling this function inside receive_btc_price() to update price in redisDB
/// This fucntion is taking payload string received from binance websocket and updating the btc price into redisDB having key "btc:price" for only price-value and key "btc:price:full_payload" to update full payload having price, timestamp etc.
pub fn update_btc_price(payload: String) {
    // let payload_clone = payload.clone();

    //checking if received msg is payload or ping/pong texts
    if payload.contains("24hrMiniTicker") {
        // let psql_pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
        let redis_pool = THREADPOOL_REDIS_SEQ_QUEUE.lock().unwrap();
        //btc price update on redis DB
        let binance_payload: BinanceMiniTickerPayload =
            serde_json::from_str(&payload.clone()).unwrap();
        let binance_payload_clone = binance_payload.clone();
        set_localdb(
            "Latest_Price",
            binance_payload_clone.clone().c.parse::<f64>().unwrap(),
        );
        // redis_pool.execute(move || {
        //     redis_db::set("btc:price", &binance_payload.clone().c);
        //     redis_db::set("btc:price:full_payload", &payload);
        //     // println!("rate :{}", &binance_payload.c);
        // });
        //btc price payload added to kafka topic : BinanceMiniTickerPayload

        //removed from relayer core, no need to save in psql by relayer core
        // psql_pool.execute(move || {
        //     // println!("Producer payload :{:#?}", &payload_clone);
        //     // producer::produce_main(message_data, topic);
        //     // producer_kafka::produce_main(&payload_clone, "BinanceMiniTickerPayload");
        //     psql_sink("BinanceMiniTickerPayload", &0, &0, binance_payload_clone);
        // });
    }
}

pub fn psql_sink(topic: &str, partition: &i32, offset: &i64, value: BinanceMiniTickerPayload) {
    let query = format!("INSERT INTO public.binancebtctickernew(e,TimeStamp_E,s,c,o,h,l,v,q,topic, partition_msg, offset_msg) VALUES ('{}',{},'{}',{},{},{},{},{},{},'{}',{},{});",&value.e,&value.e2,&value.s,&value.c,&value.o,&value.h,&value.l,&value.v,&value.q,&topic,partition,offset);
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
    client.execute(&query, &[]).unwrap();
}
