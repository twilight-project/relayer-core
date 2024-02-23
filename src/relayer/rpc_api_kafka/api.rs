use crate::config::*;
use crate::kafkalib::kafkacmd;
// use crate::relayer::RpcCommand;
use crate::relayer::*;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_http_server::{
    hyper,
    jsonrpc_core::{MetaIoHandler, Metadata, Params, Value},
    ServerBuilder,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Meta {
    pub metadata: HashMap<String, Option<String>>,
}
impl Metadata for Meta {}
pub fn kafka_queue_rpc_server() {
    // let mut io = IoHandler::default();
    let mut io = MetaIoHandler::default();
    io.add_method_with_meta(
        "CreateTraderOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<CreateTraderOrder>() {
                Ok(ordertx) => {
                    let data = RpcCommand::CreateTraderOrder(
                        ordertx.clone(),
                        meta,
                        "zkos_hex_string".to_string(),
                    );
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "CreateTraderOrder",
                    );
                    Ok(Value::String(
                        "Order request submitted successfully.".into(),
                    ))
                    // Ok(Value::Null)
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );

    io.add_method_with_meta(
        "CreateLendOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<CreateLendOrder>() {
                Ok(ordertx) => {
                    let data = RpcCommand::CreateLendOrder(
                        ordertx.clone(),
                        meta,
                        "zkos_hex_string".to_string(),
                    );
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "CreateLendOrder",
                    );
                    Ok(Value::String(
                        "Order request submitted successfully.".into(),
                    ))
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );

    io.add_method_with_meta(
        "ExecuteTraderOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<ExecuteTraderOrder>() {
                Ok(ordertx) => {
                    let data = RpcCommand::ExecuteTraderOrder(
                        ordertx.clone(),
                        meta,
                        "zkos_hex_string".to_string(),
                    );
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "ExecuteTraderOrder",
                    );
                    Ok(Value::String(
                        "Execution request submitted successfully".into(),
                    ))
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );
    io.add_method_with_meta(
        "ExecuteLendOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<ExecuteLendOrder>() {
                Ok(ordertx) => {
                    let data = RpcCommand::ExecuteLendOrder(
                        ordertx.clone(),
                        meta,
                        "zkos_hex_string".to_string(),
                    );
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "ExecuteLendOrder",
                    );
                    Ok(Value::String(
                        "Execution request submitted successfully.".into(),
                    ))
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );
    io.add_method_with_meta(
        "CancelTraderOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<CancelTraderOrder>() {
                Ok(ordertx) => {
                    let data = RpcCommand::CancelTraderOrder(
                        ordertx.clone(),
                        meta,
                        "zkos_hex_string".to_string(),
                    );
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "CancelTraderOrder",
                    );
                    Ok(Value::String(
                        "Cancellation request submitted successfully.".into(),
                    ))
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );
    println!("Starting jsonRPC server @ {}", *RPC_SERVER_SOCKETADDR);
    let server = ServerBuilder::new(io)
        .threads(*RPC_SERVER_THREAD)
        .meta_extractor(|req: &hyper::Request<hyper::Body>| {
            let auth = req
                .headers()
                .get(hyper::header::CONTENT_TYPE)
                .map(|h| h.to_str().unwrap_or("").to_owned());
            let relayer = req
                .headers()
                .get("Relayer")
                .map(|h| h.to_str().unwrap_or("").to_owned());
            Meta {
                metadata: {
                    let mut hashmap = HashMap::new();
                    hashmap.insert(String::from("CONTENT_TYPE"), auth);
                    hashmap.insert(String::from("Relayer"), relayer);
                    hashmap.insert(
                        String::from("request_server_time"),
                        Some(ServerTime::now().epoch),
                    );
                    hashmap
                },
            }
        })
        .start_http(&RPC_SERVER_SOCKETADDR.parse().unwrap())
        .unwrap();
    server.wait();
}
