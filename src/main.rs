#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod db;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod query;
mod questdb;
mod redislib;
mod relayer;

use config::*;
use db::*;
use query::*;
use redislib::*;
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
use uuid::Uuid;
#[macro_use]
extern crate lazy_static;
use std::sync::{mpsc, Arc, Mutex};

fn main() {
    // to create kafka topics
    dotenv::dotenv().expect("Failed loading dotenv");
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // kafkalib::kafka_topic::kafka_new_topic(&*RPC_CLIENT_REQUEST);
    // kafkalib::kafka_topic::kafka_new_topic(&*TRADERORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDORDER_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*LENDPOOL_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*CORE_EVENT_LOG);
    // kafkalib::kafka_topic::kafka_new_topic(&*SNAPSHOT_LOG);
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    heartbeat();
    // snapshot();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
// pub use alloc::collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque};
// let time = ServerTime::new(SystemTime::now());
//     println!("Time: {:#?}", time)

// fn main() {
//     dotenv::dotenv().expect("Failed loading dotenv");
// // let d1:String=serde_json::from_str(&getdatatx()).unwrap();
// let hex=&hex::decode(getdatatx()).unwrap();
// let tx:transaction::Transaction=bincode::deserialize(hex).unwrap();

//     let tx_send: RpcBody<transaction::Transaction> =
//     RpcRequest::new(
//         tx,
//         transactionapi::rpcclient::method::Method::TxCommit,
//     );
// let res =
//     tx_send.send(ZKOS_TRANSACTION_RPC_ENDPOINT.to_string());
//     match res {
//         Ok(intermediate_response) => {
//             println!("res: {}",serde_json::to_string(&intermediate_response).unwrap() );
//             // println!("res2:{:#?}",intermediate_response.clone().result.unwrap());
//             // let mut file =
//             //     File::create("ZKOS_TRANSACTION_RPC_ENDPOINT.txt")
//             //         .unwrap();
//             // file.write_all(
//             //     &serde_json::to_vec(&x.clone()).unwrap(),
//             // )
//             // .unwrap();

// //             /*******   test */
// let rsp= ZkosTxResponse{
//      txHash: "7DE9F3368FDBA3E23CED4AB9F425475C848CFAD5E62B692AE9DAB70B374F087F".to_string(),
//  };
//  println!("res: {}",serde_json::to_value(&rsp).unwrap() );
//  let error_from_chain = r#"{"jsonrpc":"2.0","id":"a691ee44-52a3-4c46-b03f-0ebf50ca08d3","result":{"Ok":"Error: failed to verify utxo"}}"#;
//  let error_from_rpc = r#"{"jsonrpc":"2.0","id":"a6dde34d-1eb1-4d7d-a392-0ff2c00c3f0f","result":{"Err":"{"code":-32601,\"message\":\"Method not found\", \"data\":""}"}}"#;
//  let correct_result = r#"{"jsonrpc":"2.0","id":"a691ee44-52a3-4c46-b03f-0ebf50ca08d3","result":{"Ok":"{\"txHash\" : \"7DE9F3368FDBA3E23CED4AB9F425475C848CFAD5E62B692AE9DAB70B374F087F\"}"}}"#;

// //  let resp:transactionapi::rpcclient::txrequest::RpcResponse<serde_json::Value>=serde_json::from_str(&serde_json::to_string(&intermediate_response).unwrap()).unwrap();
//  let resp:transactionapi::rpcclient::txrequest::RpcResponse<serde_json::Value>=serde_json::from_str(&error_from_chain).unwrap();
//  let tx_hash:String = match resp.result{
//                                         Ok(response)=>{match response {
//                                             serde_json::Value::String(txHash)=>match serde_json::from_str::<ZkosTxResponse>(&txHash){
//                                                 Ok(value)=>value.txHash,
//                                                 Err(_)=>txHash
//                                             },
//                                             serde_json::Value::Object(zkostxresponse)=>zkostxresponse.get("txHash").unwrap().to_string(),
//                                             _=>"errror".to_string()

//                                         }}
//                                         Err(arg)=>{
//                                             arg.to_string()}
//                                             };
//                                             println!("checkid :{:#?}",tx_hash);
//                                               /*******   test */
//             // let tx_hash:String = match intermediate_response.result{
//             //                             Ok(response)=>{match serde_json::from_value(response) {
//             //                                 Ok(ZkosTxResponse{
//             //                                      txHash,
//             //                                  })=>txHash,
//             //                                 Err(error)=>error.to_string()
//             //                             }}
//             //                             Err(arg)=>{
//             //                                 arg.to_string()}
//             //                                 };

//         }
//         Err(arg) => {
//             println!("errr1:{:#?}", arg);
//         }
//     }

// }
// use serde_derive::{Deserialize, Serialize};
// #[derive(Serialize, Deserialize)]
// pub struct ZkosTxResponse{
//    pub txHash: String,
// }
// #[derive(Serialize, Deserialize)]
// pub struct ZkosTxError{
//    pub error: String,
// }
// use std::fs::File;
// use std::io::prelude::*;
// use transactionapi::rpcclient::txrequest::{Resp, RpcBody, RpcRequest};

// fn main() {
//     dotenv::dotenv().expect("Failed loading dotenv");
// let d1:String=serde_json::to_string(&getkey()).unwrap();
// println!("{}",d1)
// // let hex=&hex::decode(getkey()).unwrap();
// // let tx:relayer::CreateTraderOrderZkos=bincode::deserialize(hex).unwrap();

// }
