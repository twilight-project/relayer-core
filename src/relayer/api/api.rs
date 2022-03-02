use crate::relayer::*;
// use crate::config::THREADPOOL_ORDERKAFKAQUEUE;
// use crate::kafkalib::producer_kafka;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::ServerBuilder;
use stopwatch::Stopwatch;
pub fn startserver() {
    let mut io = IoHandler::default();
    io.add_method("create_trader_order", move |params: Params| async move {
        match params.parse::<CreateOrder>() {
            Ok(ordertx) => {
                let sw = Stopwatch::start_new();
                ordertx.push_in_aeron_queue();
                let time_escaped = sw.elapsed();
                println!("mutex took {:#?}", time_escaped);
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
// use stopwatch::Stopwatch;
// fn new_trader_order_request_queue(order_request: CreateOrder) {
//     println!("successfully queued");
//     let sw = Stopwatch::start_new();
//     // let ordertx = TraderOrder::new(
//     //     &ordertx.account_id,
//     //     ordertx.position_type,
//     //     ordertx.order_type,
//     //     ordertx.leverage,
//     //     ordertx.initial_margin,
//     //     ordertx.available_margin,
//     //     ordertx.order_status,
//     //     ordertx.entryprice,
//     //     ordertx.execution_price,
//     // );
//     // Ok(Value::String(ordertx.serialize()))
//     let pool = THREADPOOL_ORDERKAFKAQUEUE.lock().unwrap();
//     pool.execute(move || {
//         producer_kafka::produce_main(
//             &serde_json::to_string(&order_request).unwrap(),
//             "NewTraderOrderQueue",
//         );
//     });
//     drop(pool);
//     println!("mutex took {:#?}", sw.elapsed());
// }
