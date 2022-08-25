use crate::config::*;
use crate::relayer::*;
// use crate::config::THREADPOOL_ORDERKAFKAQUEUE;
// use crate::kafkalib::producer_kafka;
use crate::db::*;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_core::*;
use jsonrpc_http_server::jsonrpc_core::{MetaIoHandler, Metadata, Params, Value};
use jsonrpc_http_server::{hyper, ServerBuilder};
use std::collections::HashMap;

#[derive(Default, Clone, Debug)]
struct Meta {
    metadata: HashMap<String, Option<String>>,
}
impl Metadata for Meta {}
pub fn startserver_repl() {
    // let mut io = IoHandler::default();
    let mut io = MetaIoHandler::default();
    io.add_method_with_meta(
        "CreateTraderOrder",
        move |params: Params, _meta: Meta| async move {
            match params.parse::<CreateTraderOrder>() {
                Ok(ordertx) => {
                    ordertx.push();
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
        "CreateLendOrder",
        move |params: Params, _meta: Meta| async move {
            match params.parse::<CreateLendOrder>() {
                Ok(ordertx) => {
                    ordertx.push();
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
        move |params: Params, _meta: Meta| async move {
            match params.parse::<ExecuteTraderOrder>() {
                Ok(ordertx) => {
                    ordertx.push();
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
        move |params: Params, _meta: Meta| async move {
            match params.parse::<ExecuteLendOrder>() {
                Ok(ordertx) => {
                    ordertx.push();
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
        move |params: Params, _meta: Meta| async move {
            match params.parse::<CancelTraderOrder>() {
                Ok(ordertx) => {
                    ordertx.push();
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
    io.add_method_with_meta(
        "GetOrderBook",
        move |params: Params, _meta: Meta| async move {
            let orderbook: OrderBook =
                serde_json::from_str(&get_localdb_string("OrderBook")).unwrap();
            Ok(serde_json::to_value(orderbook).unwrap())
        },
    );
    io.add_method_with_meta(
        "GetServerTime",
        move |params: Params, _meta: Meta| async move {
            Ok(serde_json::to_value(&check_server_time()).unwrap())
        },
    );
    io.add_method_with_meta(
        "GetRecentOrder",
        move |params: Params, _meta: Meta| async move {
            Ok(serde_json::to_value(get_recent_orders()).unwrap())
        },
    );
    io.add_method_with_meta(
        "GetCandleData",
        move |params: Params, _meta: Meta| async move {
            match params.parse::<CandleRequest>() {
                Ok(candle_request) => {
                    if (candle_request.sample_by == "1m")
                        || (candle_request.sample_by == "5m")
                        || (candle_request.sample_by == "15m")
                        || (candle_request.sample_by == "30m")
                        || (candle_request.sample_by == "1h")
                        || (candle_request.sample_by == "4h")
                        || (candle_request.sample_by == "8h")
                        || (candle_request.sample_by == "12h")
                        || (candle_request.sample_by == "24h")
                    {
                        if candle_request.limit < 101 {
                            match get_candle(
                                candle_request.sample_by.to_string(),
                                candle_request.limit,
                                candle_request.pagination,
                            ) {
                                Ok(value) => Ok(serde_json::to_value(&value).unwrap()),
                                Err(args) => {
                                    let err = JsonRpcError::invalid_params(format!(
                                        "Error: , {:?}",
                                        args
                                    ));
                                    Err(err)
                                }
                            }
                        } else {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters, max limit : 100"
                            ));
                            Err(err)
                        }
                    } else {
                        let err =
                            JsonRpcError::invalid_params(format!("Invalid parameters, sample_by"));
                        Err(err)
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
        "GetCandleDataAdvance",
        move |params: Params, _meta: Meta| async move {
            match params.parse::<CandleRequest>() {
                Ok(candle_request) => {
                    if (candle_request.sample_by == "1m")
                        || (candle_request.sample_by == "5m")
                        || (candle_request.sample_by == "15m")
                        || (candle_request.sample_by == "30m")
                        || (candle_request.sample_by == "1h")
                        || (candle_request.sample_by == "4h")
                        || (candle_request.sample_by == "8h")
                        || (candle_request.sample_by == "12h")
                        || (candle_request.sample_by == "24h")
                    {
                        if candle_request.limit < 101 {
                            match get_candle_advance(
                                candle_request.sample_by.to_string(),
                                candle_request.limit,
                                candle_request.pagination,
                            ) {
                                Ok(value) => Ok(serde_json::to_value(&value).unwrap()),
                                Err(args) => {
                                    let err = JsonRpcError::invalid_params(format!(
                                        "Error: , {:?}",
                                        args
                                    ));
                                    Err(err)
                                }
                            }
                        } else {
                            let err = JsonRpcError::invalid_params(format!(
                                "Invalid parameters max, limit : 100"
                            ));
                            Err(err)
                        }
                    } else {
                        let err = JsonRpcError::invalid_params(format!(
                            "Invalid parameters, invalid parameter 'sample_by'"
                        ));
                        Err(err)
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

    // io.add_method_with_meta("threadpoolkiller", move |params: Params, _meta: Meta| async move {
    //     let mut threadpoolkiller = THREADPOOL_MAX_ORDER_INSERT.lock().unwrap();
    //     threadpoolkiller.shutdown();
    //     drop(threadpoolkiller);
    //     Ok(serde_json::to_value(&check_server_time()).unwrap())
    // });

    // io.add_method_with_meta(
    //     "checklocaldb",
    //     move |params: Params, _meta: Meta| async move {
    //         match params.parse::<TestLocaldb>() {
    //             Ok(value) => {
    //                 // println!("{:#?}", OrderLog::get_order_readonly(&value.orderid));
    //                 let db = DB_IN_MEMORY.lock().unwrap();
    //                 println!("{:#?}", db);
    //                 drop(db);
    //                 Ok(serde_json::to_value(&check_server_time()).unwrap())
    //             }
    //             Err(args) => {
    //                 let err =
    //                     JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
    //                 Err(err)
    //             }
    //         }
    //     },
    // );

    println!("Starting jsonRPC server @ 127.0.0.1:3031");
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

            println!("I'm Here");
            Meta {
                metadata: {
                    let mut hashmap = HashMap::new();
                    hashmap.insert(String::from("CONTENT_TYPE"), auth);
                    hashmap.insert(String::from("Relayer"), relayer);
                    hashmap
                },
            }
        })
        .start_http(&"0.0.0.0:3031".parse().unwrap())
        .unwrap();
    server.wait();
}
