// use crate::config::*;
use crate::relayer::*;
// use crate::config::THREADPOOL_ORDERKAFKAQUEUE;
// use crate::kafkalib::producer_kafka;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_core::*;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, MetaIoHandler, Metadata, Params, Value};
use jsonrpc_http_server::{hyper, ServerBuilder};
pub fn startserver_repl() {
    // let mut io = IoHandler::default();
    let mut io = MetaIoHandler::default();

    io.add_method_with_meta(
        "GetServerTime",
        move |params: Params, meta: Meta| async move {
            println!("Im here :{:#?}", meta);
            Ok(serde_json::to_value(&check_server_time()).unwrap())
        },
    );

    // io.add_method("checklocaldb", move |params: Params| async move {
    //     match params.parse::<TestLocaldb>() {
    //         Ok(value) => {
    //             // println!("{:#?}", OrderLog::get_order_readonly(&value.orderid));
    //             let db = DB_IN_MEMORY.lock().unwrap();
    //             println!("{:#?}", db);
    //             drop(db);
    //             Ok(serde_json::to_value(&check_server_time()).unwrap())
    //         }
    //         Err(args) => {
    //             let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
    //             Err(err)
    //         }
    //     }
    // });

    println!("Starting jsonRPC server @ 127.0.0.1:3031");
    let server = ServerBuilder::new(io)
        .threads(25)
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
                matadata: {
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
use crate::db::*;
use std::collections::HashMap;

#[derive(Default, Clone, Debug)]
struct Meta {
    matadata: HashMap<String, Option<String>>,
}
impl Metadata for Meta {}
