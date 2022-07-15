extern crate twilight_relayer_rust;
use crate::twilight_relayer_rust::relayer::*;
extern crate reqwest;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, CONTENT_TYPE, USER_AGENT};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use stopwatch::Stopwatch;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RPCResponse {
    pub jsonrpc: String,
    pub result: String,
    pub id: i64,
}

fn construct_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("reqwest"));
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers
}
// cargo test -- --nocapture --test test_create_trader_order --test-threads 1
#[test]
fn test_create_trader_order() {
    let sw = Stopwatch::start_new();
    let threadpool_live = ThreadPool::new(10, String::from("newpool"));
    let client = reqwest::blocking::Client::new();
    for _i in 0..90000 {
        let clint_clone = client.clone();
        threadpool_live.execute(move || {
        let _res = clint_clone
            .post("http://127.0.0.1:3031")
            // .post("http://172.104.186.106/rpc")
            .headers(construct_headers())
            .body("{\"jsonrpc\": \"2.0\", \"method\": \"CreateTraderOrder\", \"id\":123, \"params\": {\"account_id\":\"siddharth\",\"position_type\":\"LONG\",\"order_type\":\"MARKET\",\"leverage\":15.0,\"initial_margin\":2,\"available_margin\":2,\"order_status\":\"PENDING\",\"entryprice\":39000.01,\"execution_price\":44440.02} }")
            .send()
            .unwrap();
    });
    }
    drop(threadpool_live);
    let time_ec = sw.elapsed();
    println!("time: {:#?} ", time_ec);
}

// cargo test -- --nocapture --test test_create_trader_order --test-threads 1
#[test]
fn test_create_trader_order_sencond() {
    let sw = Stopwatch::start_new();
    let threadpool_live = ThreadPool::new(10, String::from("newpool"));
    let client = reqwest::blocking::Client::new();
    for _i in 0..90000 {
        let clint_clone = client.clone();
        threadpool_live.execute(move || {
        let _res = clint_clone
            .post("http://127.0.0.1:3031")
            // .post("http://172.104.186.106/rpc")
            .headers(construct_headers())
            .body("{\"jsonrpc\": \"2.0\", \"method\": \"CreateTraderOrder\", \"id\":123, \"params\": {\"account_id\":\"siddharth\",\"position_type\":\"SHORT\",\"order_type\":\"MARKET\",\"leverage\":15.0,\"initial_margin\":2.1,\"available_margin\":2.1,\"order_status\":\"PENDING\",\"entryprice\":39000.01,\"execution_price\":44440.02} }")
            .send()
            .unwrap();
    });
    }
    drop(threadpool_live);
    let time_ec = sw.elapsed();
    println!("time: {:#?} ", time_ec);
}
use std::{thread, time};
use twilight_relayer_rust::config::*;
use twilight_relayer_rust::redislib::*;
use twilight_relayer_rust::*;
#[macro_use]
extern crate lazy_static;
// cargo test -- --nocapture --test test_zzneworderfb --test-threads 1
#[test]
fn test_zzneworderfb() {
    init_psql();
    delele_all_data_table().unwrap();
    println!("pslq successfull...");
    ordertest::initprice();
    ordertest::generatelendorder();
    println!("pslq successfull...");
    start_cronjobs();
    thread::sleep(time::Duration::from_millis(2000));
    let orderrequest: CreateTraderOrder = CreateTraderOrder {
        account_id: String::from("test order"),
        position_type: PositionType::LONG,
        order_type: OrderType::MARKET,
        leverage: 10.0,
        initial_margin: 1.0,
        available_margin: 1.0,
        order_status: OrderStatus::PENDING,
        entryprice: 10000.0,
        execution_price: 20000.0,
    };
    println!("I'm here");
    let threadpool_live = ThreadPool::new(25, String::from("newpool"));
    let sw = Stopwatch::start_new();
    for _i in 0..50000 {
        let order_request_clone = orderrequest.clone();
        threadpool_live.execute(move || get_new_trader_order(order_request_clone.serialize()));
        // thread::spawn(move || relayer::get_new_trader_order(self.serialize()));
    }
    drop(threadpool_live);
    let time_ec = sw.elapsed();
    println!("time: {:#?} ", time_ec);
    // thread::sleep(time::Duration::from_millis(60000));
    loop {
        let length = redis_batch::zrangeallopenorders_batch_count();
        if length > 49999 {
            break;
        }
        thread::sleep(time::Duration::from_millis(500));
    }
    let time_ec = sw.elapsed();
    println!("time: {:#?} ", time_ec);
    let mut threadpool = THREADPOOL.lock().unwrap();
    let mut threadpool_questdb = THREADPOOL_QUESTDB.lock().unwrap();
    let mut threadpool_max_order_insert_pool = THREADPOOL_MAX_ORDER_INSERT.lock().unwrap();
    threadpool.shutdown();
    threadpool_questdb.shutdown();
    threadpool_max_order_insert_pool.shutdown();
    let mut psql_insert_order_pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
    psql_insert_order_pool.shutdown();
    drop(threadpool_max_order_insert_pool);
    drop(psql_insert_order_pool);
    drop(threadpool);
    drop(threadpool_questdb);
}
