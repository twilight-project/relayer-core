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
    for _i in 0..10000 {
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
    for _i in 0..10000 {
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
