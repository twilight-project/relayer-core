use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_core::*;
use jsonrpc_http_server::{
    hyper,
    jsonrpc_core::{MetaIoHandler, Metadata, Params, Value},
    ServerBuilder,
};
use relayerwalletlib::verify_client_message::*;
use std::collections::HashMap;
use stopwatch::Stopwatch;
#[derive(Default, Clone, Debug)]
struct Meta {
    metadata: HashMap<String, Option<String>>,
}
impl Metadata for Meta {}
pub fn startserver() {
    // let mut io = IoHandler::default();
    let mut io = MetaIoHandler::default();

    io.add_method_with_meta(
        "GetServerTime",
        move |params: Params, _meta: Meta| async move {
            Ok(serde_json::to_value(&check_server_time()).unwrap())
        },
    );

    /*************************** order details lend and trade by zkos */
    io.add_method_with_meta(
        "QueryTraderOrderZkos",
        move |params: Params, _meta: Meta| async move {
            let request: std::result::Result<QueryTraderOrderZkos, jsonrpc_core::Error>;

            request = match params.parse::<ByteRec>() {
                Ok(hex_data) => match hex::decode(&hex_data.data) {
                    Ok(order_bytes) => match bincode::deserialize(&order_bytes) {
                        Ok(ordertx) => Ok(ordertx),
                        Err(args) => {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters, {:?}",
                                args
                            ));
                            Err(err)
                        }
                    },
                    Err(args) => {
                        let err =
                            JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                        Err(err)
                    }
                },
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            };
            // println!("data: {:#?}", request.clone().unwrap().query_trader_order);
            match request {
                Ok(mut query) => {
                    //verify signature
                    match verify_query_order(
                        query.msg.clone(),
                        &bincode::serialize(&query.query_trader_order).unwrap(),
                    ) {
                        Ok(_) => {
                            let query_para = query.msg.public_key.clone();
                            // let query_para = hex::encode(query_para1.as_bytes());
                            // println!("i am at 364:{:#?}", query_para);
                            let order = get_traderorder_details_by_account_id(
                                serde_json::from_str(&query_para).unwrap(),
                            );
                            match order {
                                Ok(order_data) => Ok(serde_json::to_value(&order_data).unwrap()),
                                Err(args) => {
                                    let err = JsonRpcError::invalid_params(format!(
                                        "Invalid parameters, {:?}",
                                        args
                                    ));
                                    Err(err)
                                }
                            }
                        }
                        Err(arg) => {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters, {:?}",
                                arg
                            ));
                            Err(err)
                        }
                    }
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
        "QueryLendOrderZkos",
        move |params: Params, _meta: Meta| async move {
            let request: std::result::Result<QueryLendOrderZkos, jsonrpc_core::Error>;
            request = match params.parse::<ByteRec>() {
                Ok(hex_data) => {
                    match hex::decode(hex_data.data) {
                        Ok(order_bytes) => match bincode::deserialize(&order_bytes) {
                            Ok(ordertx) => Ok(ordertx),
                            Err(args) => {
                                let err = JsonRpcError::invalid_params(format!(
                                    "Invalid parameters, {:?}",
                                    args
                                ));
                                Err(err)
                            }
                        },
                        // Ok(hex_data) => Ok(hex_data),
                        Err(args) => {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters, {:?}",
                                args
                            ));
                            Err(err)
                        }
                    }
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            };

            match request {
                Ok(mut query) => {
                    match verify_query_order(
                        query.msg.clone(),
                        &bincode::serialize(&query.query_lend_order).unwrap(),
                    ) {
                        Ok(_) => {
                            // let query_para = query.query_lend_order.account_id;
                            let query_para = query.msg.public_key.clone();
                            let order = get_lendorder_details_by_account_id(query_para);
                            match order {
                                Ok(order_data) => Ok(serde_json::to_value(&order_data).unwrap()),
                                Err(args) => {
                                    let err = JsonRpcError::invalid_params(format!(
                                        "Invalid parameters, {:?}",
                                        args
                                    ));
                                    Err(err)
                                }
                            }
                        }
                        Err(arg) => {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters, {:?}",
                                arg
                            ));
                            Err(err)
                        }
                    }
                }
                Err(args) => {
                    let err =
                        JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                    Err(err)
                }
            }
        },
    );
    /*******************************end  */

    println!("Starting jsonRPC server @ 127.0.0.1:3030");
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

            // println!("I'm Here");
            Meta {
                metadata: {
                    let mut hashmap = HashMap::new();
                    hashmap.insert(String::from("CONTENT_TYPE"), auth);
                    hashmap.insert(String::from("Relayer"), relayer);
                    hashmap
                },
            }
        })
        .start_http(&"0.0.0.0:3030".parse().unwrap())
        .unwrap();
    server.wait();
}
// use stopwatch::Stopwatch;
