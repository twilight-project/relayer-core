use crate::relayer::*;
// use crate::config::THREADPOOL_ORDERKAFKAQUEUE;
// use crate::kafkalib::producer_kafka;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::ServerBuilder;
pub fn startserver() {
    let mut io = IoHandler::default();
    io.add_method("CreateTraderOrder", move |params: Params| async move {
        match params.parse::<CreateTraderOrder>() {
            Ok(ordertx) => {
                ordertx.push();
                Ok(Value::String(
                    "Order request submitted successfully.".into(),
                ))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });

    io.add_method("CreateLendOrder", move |params: Params| async move {
        match params.parse::<CreateLendOrder>() {
            Ok(ordertx) => {
                ordertx.push();
                Ok(Value::String(
                    "Order request submitted successfully.".into(),
                ))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });

    io.add_method("ExecuteTraderOrder", move |params: Params| async move {
        match params.parse::<ExecuteTraderOrder>() {
            Ok(ordertx) => {
                ordertx.push();
                Ok(Value::String(
                    "Execution request submitted successfully".into(),
                ))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });
    io.add_method("ExecuteLendOrder", move |params: Params| async move {
        match params.parse::<ExecuteLendOrder>() {
            Ok(ordertx) => {
                ordertx.push();
                Ok(Value::String(
                    "Execution request submitted successfully.".into(),
                ))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });
    io.add_method("CancelTraderOrder", move |params: Params| async move {
        match params.parse::<CancelTraderOrder>() {
            Ok(ordertx) => {
                ordertx.push();
                Ok(Value::String(
                    "Cancellation request submitted successfully.".into(),
                ))
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });
    io.add_method("GetOrderBook", move |params: Params| async move {
        Ok(Value::String(get_localdb_string("OrderBook")))
    });
    io.add_method("GetServerTime", move |params: Params| async move {
        Ok(Value::String(
            serde_json::to_string(&check_server_time()).unwrap(),
        ))
    });
    io.add_method("GetRecentOrder", move |params: Params| async move {
        Ok(Value::String(get_recent_orders()))
    });

    println!("Starting jsonRPC server @ 127.0.0.1:3030");
    let server = ServerBuilder::new(io)
        .threads(3)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .unwrap();
    server.wait();
}
