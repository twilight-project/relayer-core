use crate::config::*;
use crate::kafkalib::kafkacmd;
// use crate::relayer::RpcCommand;
use super::api::Meta;
use crate::relayer::*;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_http_server::{
    hyper,
    jsonrpc_core::{MetaIoHandler, Metadata, Params, Value},
    ServerBuilder,
};
use quisquislib::keys::PublicKey;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
// #[derive(Default, Clone, Debug, Deserialize, Serialize, PartialEq)]
// pub struct Meta {
//     pub metadata: HashMap<String, Option<String>>,
// }
// impl Metadata for Meta {}
pub fn kafka_queue_rpc_server_with_zkos() {
    // let mut io = IoHandler::default();
    let mut io = MetaIoHandler::default();
    io.add_method_with_meta(
        "CreateTraderOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<CreateTraderOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut order_request = ordertx.create_trader_order.clone();

                    let account_id =
                        //ordertx.input.input.input.as_owner_address().unwrap();
                        ordertx.input.input.as_owner_address().unwrap();
                    order_request.account_id = hex::encode(account_id.as_bytes());
                    let response_clone = order_request.account_id.clone();
                    //
                    let mut meta_clone = meta.clone();
                    meta_clone.metadata.insert(
                        String::from("zkos_data"),
                        Some(
                            serde_json::to_string(&bincode::serialize(&ordertx.input).unwrap())
                                .unwrap(),
                        ),
                    );
                    let data = RpcCommand::CreateTraderOrder(order_request, meta_clone);
                    //call verifier to check balance, etc...
                    //if verified the call kafkacmd::send_to_kafka_queue
                    //also convert public key into hash fn and put it in account_id field
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        "CreateTraderOrder",
                    );
                    Ok(Value::String(
                        format!(
                            "Order request submitted successfully. your id is: {:#?}",
                            response_clone
                        )
                        .into(),
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
            match params.parse::<CreateLendOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut order_request = ordertx.create_lend_order.clone();
                    let account_id =
                    //ordertx.input.input.input.as_owner_address().unwrap();
                    ordertx.input.input.as_owner_address().unwrap();
                    order_request.account_id = hex::encode(account_id.as_bytes());
                    //

                    let data = RpcCommand::CreateLendOrder(order_request, meta);
                    kafkacmd::send_to_kafka_queue(
                        data,
                        String::from("CLIENT-REQUEST"),
                        &format!(
                            "CreateLendOrder-{}",
                            std::time::SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string()
                        ),
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
            match params.parse::<ExecuteTraderOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut settle_request = ordertx.execute_trader_order.clone();
                    let account_id = ordertx.msg.input.as_owner_address().unwrap();
                    settle_request.account_id = hex::encode(account_id.as_bytes());
                    //

                    let data = RpcCommand::ExecuteTraderOrder(settle_request, meta);
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
            match params.parse::<ExecuteLendOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut settle_request = ordertx.execute_lend_order.clone();
                    let account_id = ordertx.msg.input.as_owner_address().unwrap();
                    settle_request.account_id = hex::encode(account_id.as_bytes());
                    //

                    let data = RpcCommand::ExecuteLendOrder(settle_request, meta);
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
        "QueryTraderOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<ExecuteLendOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut settle_request = ordertx.execute_lend_order.clone();
                    let account_id = ordertx.msg.input.as_owner_address().unwrap();
                    settle_request.account_id = hex::encode(account_id.as_bytes());
                    //

                    let data = RpcCommand::ExecuteLendOrder(settle_request, meta);
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
        "QueryLendOrder",
        move |params: Params, meta: Meta| async move {
            match params.parse::<ExecuteLendOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut settle_request = ordertx.execute_lend_order.clone();
                    let account_id = ordertx.msg.input.as_owner_address().unwrap();
                    settle_request.account_id = hex::encode(account_id.as_bytes());
                    //

                    let data = RpcCommand::ExecuteLendOrder(settle_request, meta);
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
            match params.parse::<CancelTraderOrderZkos>() {
                Ok(ordertx) => {
                    //to get public key from data
                    let mut cancel_request = ordertx.cancel_trader_order.clone();
                    let account_id = ordertx.msg.public_key;
                    // cancel_request.account_id = hex::encode(account_id.as_bytes());
                    cancel_request.account_id = account_id;
                    //

                    let data = RpcCommand::CancelTraderOrder(cancel_request, meta);
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
                        Some(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),
                        ),
                    );
                    hashmap
                },
            }
        })
        .start_http(&RPC_SERVER_SOCKETADDR.parse().unwrap())
        .unwrap();
    server.wait();
}
