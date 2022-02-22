use crate::relayer::*;
// use jsonrpc_core::types::error::ErrorCode;
use crate::kafkalib::producer_kafka;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::ServerBuilder;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateOrder {
    pub account_id: String,
    pub position_type: PositionType,
    pub order_type: OrderType,
    pub leverage: f64,
    pub initial_margin: f64,
    pub available_margin: f64,
    pub order_status: OrderStatus,
    pub entryprice: f64,
    pub execution_price: f64,
}

pub fn startserver() {
    let mut io = IoHandler::default();
    io.add_method("create_trader_order", move |params: Params| async move {
        match params.parse::<CreateOrder>() {
            Ok(ordertx) => {
                new_trader_order_request_queue(ordertx);
                Ok(Value::String("Order submitted successfully".into()))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });

    let server = ServerBuilder::new(io)
        .threads(3)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .unwrap();

    server.wait();
}

fn new_trader_order_request_queue(order_request: CreateOrder) {
    println!("successfully queued");
    producer_kafka::produce_main(
        &serde_json::to_string(&order_request).unwrap(),
        "NewTraderOrderQueue",
    );
    // let ordertx = TraderOrder::new(
    //     &ordertx.account_id,
    //     ordertx.position_type,
    //     ordertx.order_type,
    //     ordertx.leverage,
    //     ordertx.initial_margin,
    //     ordertx.available_margin,
    //     ordertx.order_status,
    //     ordertx.entryprice,
    //     ordertx.execution_price,
    // );
    // Ok(Value::String(ordertx.serialize()))
}
