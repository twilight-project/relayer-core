use crate::config::*;
use crate::db::*;
use crate::relayer::relayer_command_handler::relayer_event_handler;
use crate::relayer::*;
use jsonrpc_core::types::error::Error as JsonRpcError;
use jsonrpc_core::*;
use jsonrpc_http_server::{
    hyper,
    jsonrpc_core::{ MetaIoHandler, Metadata, Params },
    ServerBuilder,
};
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
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

    io.add_method_with_meta("GetServerTime", move |params: Params, _meta: Meta| async move {
        Ok(serde_json::to_value(&check_server_time()).unwrap())
    });

    io.add_method_with_meta("CheckVector", move |params: Params, meta: Meta| async move {
        match params.parse::<TestLocaldb>() {
            Ok(value) => {
                // let sw = Stopwatch::start_new();
                match value.key {
                    1 => {
                        let trader_lp_long = LEND_ORDER_DB.lock().unwrap();
                        println!("\n LEND_ORDER_DB : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    2 => {
                        let trader_lp_long = LEND_POOL_DB.lock().unwrap();
                        println!("\n LEND_POOL_DB : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    3 => {
                        let trader_lp_long = TRADER_ORDER_DB.lock().unwrap();
                        println!("\n Trader_ORDER_DB : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    4 => {
                        let trader_lp_long = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
                        println!("\n TRADER_LIMIT_OPEN_SHORT : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    5 => {
                        let trader_lp_long = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
                        println!("\n TRADER_LIMIT_OPEN_LONG : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    6 => {
                        let trader_lp_long = TRADER_ORDER_DB.lock().unwrap();
                        println!("\n Trader_ORDER_DB : {:#?}", trader_lp_long.sequence.clone());
                        drop(trader_lp_long);
                    }
                    7 => {
                        let trader_lp_long = TRADER_LP_LONG.lock().unwrap();
                        println!("\n TRADER_LP_LONG : {:#?}", trader_lp_long.len);
                        drop(trader_lp_long);
                    }
                    8 => {
                        let trader_lp_long = TRADER_LP_SHORT.lock().unwrap();
                        println!("\n TRADER_LP_SHORT : {:#?}", trader_lp_long.len);
                        drop(trader_lp_long);
                    }
                    9 => {
                        let trader_lp_long = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
                        println!("\n TRADER_LIMIT_OPEN_LONG : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    10 => {
                        let trader_lp_long = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
                        println!("\n TRADER_LIMIT_OPEN_LONG : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    11 => {
                        let trader_lp_long = POSITION_SIZE_LOG.lock().unwrap();
                        println!("\n POSITION_SIZE_LOG : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    12 => {
                        // let trader_lp_long = POSITION_SIZE_LOG.lock().unwrap();
                        println!("\n CurrentPrice : {:#?}", get_localdb("CurrentPrice"));
                        // drop(trader_lp_long);
                    }
                    13 => {
                        // let trader_lp_long = POSITION_SIZE_LOG.lock().unwrap();
                        // println!("\n CurrentPrice : {:#?}", get_localdb("CurrentPrice"));

                        // drop(trader_lp_long);
                        std::thread::Builder
                            ::new()
                            .name(String::from("updatefundingrate_localdb(1.0)"))
                            .spawn(move || {
                                updatefundingrate_localdb(1.0);
                            })
                            .unwrap();
                    }
                    14 => {
                        std::thread::Builder
                            ::new()
                            .name(String::from("json-RPC startserver"))
                            .spawn(move || {
                                let current_price = value.price;
                                let get_open_order_short_list1 = TRADER_LP_SHORT.lock().unwrap();
                                let get_open_order_long_list1 = TRADER_LP_LONG.lock().unwrap();
                                let sw = Stopwatch::start_new();
                                let mut get_open_order_short_list =
                                    get_open_order_short_list1.clone();
                                let mut get_open_order_long_list =
                                    get_open_order_long_list1.clone();
                                let time1 = sw.elapsed();
                                let orderid_list_short = get_open_order_short_list.search_lt(
                                    (current_price * 10000.0) as i64
                                );
                                let orderid_list_long = get_open_order_long_list.search_gt(
                                    (current_price * 10000.0) as i64
                                );
                                drop(get_open_order_short_list1);
                                drop(get_open_order_long_list1);
                                let time2 = sw.elapsed();
                                println!("cloning time is {:#?}", time1);
                                println!(
                                    "searching for ordercount:{} is {:#?}",
                                    orderid_list_short.len() + orderid_list_long.len(),
                                    time2
                                );
                            })
                            .unwrap();
                    }
                    15 => {
                        std::thread::Builder
                            ::new()
                            .name(String::from("snapshot"))
                            .spawn(move || {
                                let _ = snapshot();
                            })
                            .unwrap();
                    }
                    16 => {
                        let mut trader_lp_long = LEND_POOL_DB.lock().unwrap();
                        println!("\n LEND_POOL_DB : {:#?}", trader_lp_long);
                        *trader_lp_long = LendPool::new();
                        drop(trader_lp_long);
                    }
                    17 => {
                        let mut lend_pool_db = LEND_POOL_DB.lock().unwrap();
                        *lend_pool_db = LendPool::new();
                        println!("\n LEND_POOL_DB : {:#?}", lend_pool_db);
                        drop(lend_pool_db);
                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                        *trader_order_db = LocalDB::<TraderOrder>::new();
                        println!("\n Trader_ORDER_DB : {:#?}", trader_order_db);
                        drop(trader_order_db);
                        let mut lend_order_db = LEND_ORDER_DB.lock().unwrap();
                        *lend_order_db = LocalDB::<LendOrder>::new();
                        println!("\n LEND_ORDER_DB : {:#?}", lend_order_db);
                        drop(lend_order_db);
                    }
                    18 => {
                        let lend_pool_db = LEND_POOL_DB.lock().unwrap();
                        // *lend_pool_db = LendPool::new();
                        println!(
                            "\n LEND_POOL_DB : {:#?}",
                            hex::encode(
                                bincode::serialize(&lend_pool_db.last_output_state.clone()).unwrap()
                            )
                        );
                        drop(lend_pool_db);
                    }
                    19 => {
                        set_relayer_status(value.relayer_status);
                    }
                    20 => {
                        relayer_event_handler(
                            RelayerCommand::PriceTickerLiquidation(
                                vec![value.orderid],
                                commands::Meta {
                                    metadata: meta.metadata,
                                },
                                value.price
                            )
                        );
                    }
                    21 => {
                        let output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
                        println!("\n OUTPUT_STORAGE : {:#?}", output_hex_storage);
                        drop(output_hex_storage);
                    }
                    22 => {
                        let output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
                        let uuid_to_byte = match bincode::serialize(&value.orderid) {
                            Ok(uuid_v_u8) => uuid_v_u8,
                            Err(_) => Vec::new(),
                        };
                        let output_option = match
                            output_hex_storage.get_utxo_by_id(uuid_to_byte, 0)
                        {
                            Ok(output) => output,
                            Err(_) => None,
                        };
                        println!("\n output_option : {:#?}", output_option);
                        drop(output_hex_storage);
                    }
                    _ => {
                        let trader_lp_long = LEND_ORDER_DB.lock().unwrap();
                        println!("\n LEND_POOL_DB : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                }
                Ok(serde_json::to_value(&check_server_time()).unwrap())
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(format!("Invalid parameters, {:?}", args));
                Err(err)
            }
        }
    });

    /*****************Fee Update */
    io.add_method_with_meta("UpdateFees", move |params: Params, meta: Meta| async move {
        match params.parse::<FeeUpdate>() {
            Ok(value) => {
                let relayer_command = value.to_relayer_command();
                relayer_event_handler(relayer_command);
                crate::log_heartbeat!(info, "Fee update in progress...");
                Ok(serde_json::to_value("Fee update in progress, check logs for status").unwrap())
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(
                    format!(
                        "Invalid parameters, {:?}. Expected format: {{ \"order_filled_on_market\": f64, \"order_filled_on_limit\": f64, \"order_settled_on_market\": f64, \"order_settled_on_limit\": f64 }}",
                        args
                    )
                );
                Err(err)
            }
        }
    });

    crate::log_heartbeat!(info, "Starting jsonRPC server @ {}", *RELAYER_SERVER_SOCKETADDR);
    let server = match
        ServerBuilder::new(io)
            .threads(*RELAYER_SERVER_THREAD)
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
            .start_http(&(*RELAYER_SERVER_SOCKETADDR).parse().unwrap())
    {
        Ok(server) => server,
        Err(e) => {
            crate::log_heartbeat!(error, "Failed to start jsonRPC server: {:?}", e);
            return;
        }
    };
    server.wait();
}
// use stopwatch::Stopwatch;
