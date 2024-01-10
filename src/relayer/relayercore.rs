use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::*;
use address::Network;
use std::sync::{Arc, Mutex, RwLock};
// use transaction::verify_relayer::create_trade_order;
use relayerwalletlib::order::*;
use relayerwalletlib::zkoswalletlib::programcontroller::ContractManager;
use relayerwalletlib::zkoswalletlib::relayer::*;
use transactionapi::rpcclient::txrequest::{Resp, RpcBody, RpcRequest};
use utxo_in_memory::db::LocalDBtrait;
// use stopwatch::Stopwatch;
lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(15, String::from("THREADPOOL_NORMAL_ORDER")));
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(15, String::from("THREADPOOL_URGENT_ORDER")));
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_ZKOS: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_ZKOS")));
    pub static ref CONTRACTMANAGER: Arc<Mutex<ContractManager>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let mut contract_manager = ContractManager::import_program(&WALLET_PROGRAM_PATH.clone());
        Arc::new(Mutex::new(contract_manager))
    };
}
pub fn client_cmd_receiver() {
    if *KAFKA_STATUS == "Enabled" {
        match receive_from_kafka_queue(
            RPC_CLIENT_REQUEST.clone().to_string(),
            String::from("client_cmd_receiver3"),
        ) {
            Ok(rpc_cmd_receiver) => {
                let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);
                loop {
                    let rpc_client_cmd_request = rpc_cmd_receiver1.lock().unwrap().recv().unwrap();
                    // println!("print command : {:#?}", rpc_client_cmd_request);
                    rpc_event_handler(rpc_client_cmd_request);
                }
            }
            Err(arg) => {
                println!("error in client_cmd_receiver: {:#?}", arg);
            }
        }
    }
}

pub fn rpc_event_handler(command: RpcCommand) {
    let command_clone = command.clone();
    match command {
        RpcCommand::CreateTraderOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_NORMAL_ORDER.lock().unwrap();
            buffer.execute(move || {
                let (orderdata, status) = TraderOrder::new_order(rpc_request.clone());
                let order_state = orderdata.orderinsert_localdb(status);
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let completed_order = trader_order_db.add(order_state, command_clone);
                drop(trader_order_db);
            });
            drop(buffer);
        }
        RpcCommand::ExecuteTraderOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_URGENT_ORDER.lock().unwrap();
            buffer.execute(move || {
                let execution_price = rpc_request.execution_price.clone();
                let current_price = get_localdb("CurrentPrice");
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let order_detail_wraped = trader_order_db.get_mut(rpc_request.uuid);
                drop(trader_order_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        match order.order_status {
                            OrderStatus::FILLED => {
                                let (payment, order_status) = order.check_for_settlement(
                                    execution_price,
                                    current_price,
                                    rpc_request.order_type,
                                );
                                match order_status {
                                    OrderStatus::SETTLED => {
                                        let order_clone = order.clone();
                                        drop(order);
                                        let mut lendpool = LEND_POOL_DB.lock().unwrap();
                                        lendpool.add_transaction(
                                            LendPoolCommand::AddTraderOrderSettlement(
                                                command_clone,
                                                order_clone.clone(),
                                                payment,
                                            ),
                                        );
                                        drop(lendpool);
                                    }
                                    _ => {
                                        drop(order);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                println!(
                                    "Order {} not found or invalid order status !!",
                                    rpc_request.uuid
                                );
                            }
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                    }
                }
            });
            drop(buffer);
        }
        RpcCommand::CreateLendOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut lend_pool = LEND_POOL_DB.lock().unwrap();

                let (tlv0, tps0) = lend_pool.get_lendpool();
                let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, tlv0, tps0);
                lendorder.order_status = OrderStatus::FILLED;
                lend_pool.add_transaction(LendPoolCommand::LendOrderCreateOrder(
                    command_clone,
                    lendorder.clone(),
                    lendorder.deposit,
                ));
                drop(lend_pool);
            });
            drop(buffer);
        }
        RpcCommand::ExecuteLendOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut lend_pool = LEND_POOL_DB.lock().unwrap();
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                let order_detail_wraped = lendorder_db.get_mut(rpc_request.uuid);
                drop(lendorder_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        match order.order_status {
                            OrderStatus::FILLED => {
                                let (tlv2, tps2) = lend_pool.get_lendpool();
                                match order.calculatepayment_localdb(tlv2, tps2) {
                                    Ok(_) => {
                                        let order_clone = order.clone();
                                        drop(order);
                                        lend_pool.add_transaction(
                                            LendPoolCommand::LendOrderSettleOrder(
                                                command_clone,
                                                order_clone.clone(),
                                                order_clone.nwithdraw,
                                            ),
                                        );
                                        drop(lend_pool);
                                    }
                                    Err(arg) => {
                                        drop(order);
                                        drop(lend_pool);
                                        println!("Error found:{:#?}", arg);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                drop(lend_pool);
                                println!(
                                    "Order {} not found or invalid order status !!",
                                    rpc_request.uuid
                                );
                            }
                        }
                    }
                    Err(arg) => {
                        drop(lend_pool);
                        println!("Error found:{:#?}", arg);
                    }
                }
            });
            drop(buffer);
        }
        RpcCommand::CancelTraderOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_URGENT_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let order_detail_wraped = trader_order_db.get_mut(rpc_request.uuid);
                drop(trader_order_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        println!("FILLED order:{:#?}", order);
                        match order.order_status {
                            OrderStatus::PENDING => {
                                let (cancel_status, order_status) = order.cancelorder_localdb();
                                match order_status {
                                    OrderStatus::CANCELLED => {
                                        let order_clone = order.clone();
                                        drop(order);
                                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                        let cancelled_order =
                                            trader_order_db.remove(order_clone, command_clone);
                                        drop(trader_order_db);
                                    }
                                    _ => {
                                        drop(order);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                println!(
                                    "Order {} not found or invalid order status !!",
                                    rpc_request.uuid
                                );
                            }
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                    }
                }
            });
            drop(buffer);
        } // RpcCommand::Liquidation(trader_order, metadata) => {}
        RpcCommand::RelayerCommandTraderOrderSettleOnLimit(..) => {}
    }
}

pub fn relayer_event_handler(command: RelayerCommand) {
    let command_clone = command.clone();
    match command {
        RelayerCommand::FundingCycle(pool_batch_order, metadata, fundingrate) => {
            Event::new(
                Event::FundingRateUpdate(
                    fundingrate,
                    metadata
                        .metadata
                        .get(&String::from("CurrentPrice"))
                        .unwrap()
                        .clone()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    iso8601(&std::time::SystemTime::now()),
                ),
                format!(
                    "insert_fundingrate-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                        .to_string()
                ),
                TRADERORDER_EVENT_LOG.clone().to_string(),
            );
            let mut lendpool = LEND_POOL_DB.lock().unwrap();
            lendpool.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(command_clone));
            drop(lendpool);
        }
        RelayerCommand::PriceTickerLiquidation(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<Result<Arc<RwLock<TraderOrder>>, std::io::Error>> =
                Vec::new();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let orderdetail = trader_order_db.get_mut(order_id);
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            let buffer = THREADPOOL_BULK_PENDING_ORDER.lock().unwrap();
            for order_detail_wraped in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        match order.order_status {
                            OrderStatus::FILLED => {
                                order.order_status = OrderStatus::LIQUIDATE;
                                //update batch process
                                let payment = order.liquidate(current_price_clone);
                                let order_clone = order.clone();
                                drop(order);
                                println!("locking mutex LEND_POOL_DB");
                                let mut lendpool = LEND_POOL_DB.lock().unwrap();
                                lendpool.add_transaction(
                                    LendPoolCommand::AddTraderOrderLiquidation(
                                        RelayerCommand::PriceTickerLiquidation(
                                            vec![order_clone.uuid.clone()],
                                            metadata_clone,
                                            current_price_clone,
                                        ),
                                        order_clone.clone(),
                                        payment,
                                    ),
                                );
                                println!("dropping mutex LEND_POOL_DB");

                                drop(lendpool);
                            }
                            _ => {
                                drop(order);
                                println!("Invalid order status !!\n");
                            }
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                    }
                });
            }
            drop(buffer);
        }
        RelayerCommand::PriceTickerOrderFill(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<Result<Arc<RwLock<TraderOrder>>, std::io::Error>> =
                Vec::new();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let orderdetail = trader_order_db.get_mut(order_id);
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            let buffer = THREADPOOL_BULK_PENDING_ORDER.lock().unwrap();
            for order_detail_wraped in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        match order.order_status {
                            OrderStatus::PENDING => {
                                let (update_order_detail, order_status) =
                                    order.pending_order(current_price_clone);

                                let filled_order =
                                    update_order_detail.orderinsert_localdb(order_status);
                                order.order_status = OrderStatus::FILLED;
                                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                drop(order);
                                let _ = trader_order_db.update(
                                    filled_order.clone(),
                                    RelayerCommand::PriceTickerOrderFill(
                                        vec![filled_order.uuid.clone()],
                                        metadata_clone,
                                        current_price_clone,
                                    ),
                                );
                                drop(trader_order_db);
                            }
                            _ => {
                                drop(order);
                                println!("Invalid order status !!\n");
                            }
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                    }
                });
            }
            drop(buffer);
        }
        RelayerCommand::PriceTickerOrderSettle(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<Result<Arc<RwLock<TraderOrder>>, std::io::Error>> =
                Vec::new();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let orderdetail = trader_order_db.get_mut(order_id);
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            let buffer = THREADPOOL_BULK_PENDING_ORDER.lock().unwrap();
            for order_detail_wraped in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        match order.order_status {
                            OrderStatus::FILLED => {
                                let (payment, order_status) = order.check_for_settlement(
                                    current_price_clone,
                                    current_price_clone,
                                    OrderType::MARKET,
                                );
                                match order_status {
                                    OrderStatus::SETTLED => {
                                        let order_clone = order.clone();
                                        drop(order);
                                        let mut lendpool = LEND_POOL_DB.lock().unwrap();
                                        lendpool.add_transaction(
                                            LendPoolCommand::AddTraderLimitOrderSettlement(
                                                RelayerCommand::PriceTickerOrderSettle(
                                                    vec![order_clone.uuid.clone()],
                                                    metadata_clone,
                                                    current_price_clone,
                                                ),
                                                order_clone.clone(),
                                                payment,
                                            ),
                                        );
                                        drop(lendpool);
                                    }
                                    _ => {
                                        drop(order);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                println!("Invalid order status !!\n");
                            }
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                    }
                });
            }
            drop(buffer);
        }
        RelayerCommand::FundingCycleLiquidation(order_id_array, metadata, currentprice) => {}
        RelayerCommand::RpcCommandPoolupdate() => {
            let mut lendpool = LEND_POOL_DB.lock().unwrap();
            lendpool.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(command_clone));
            drop(lendpool);
        }
        RelayerCommand::FundingOrderEventUpdate(mut trader_order, metadata) => {
            trader_order.timestamp = systemtime_to_utc();
            Event::new(
                Event::TraderOrderFundingUpdate(trader_order.clone(), command_clone),
                format!("update_order_funding-{}", trader_order.uuid.clone()),
                TRADERORDER_EVENT_LOG.clone().to_string(),
            );
        }
    }
}

pub fn zkos_order_handler(command: ZkosTxCommand) {
    let command_clone = command.clone();
    if ENABLE_ZKOS_CHAIN_TRANSACTION.clone(){
        match command {
            ZkosTxCommand::CreateTraderOrderTX(trader_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS.lock().unwrap();
                buffer.execute(move || match trader_order.order_status {
                    OrderStatus::FILLED => {
                        match rpc_command {
                            RpcCommand::CreateTraderOrder(order_request, meta,_zkos_hex_string,) => {
                               
                                let zkos_create_order_result=ZkosCreateOrder::decode_from_hex_string(_zkos_hex_string);
                                // create transaction
                                match zkos_create_order_result {
                                    Ok(zkos_create_order) => {
                                        let mut file = File::create("./zkos_create_order.txt").unwrap();
                                        file.write_all(
                                            &serde_json::to_vec(&zkos_create_order.clone()).unwrap(),
                                        )
                                        .unwrap();
                                        let mut file_bin =
                                            File::create("zkos_create_order_file_bin.txt").unwrap();
                                        file_bin
                                            .write_all(
                                                &serde_json::to_vec(
                                                    &bincode::serialize(&zkos_create_order.clone())
                                                        .unwrap(),
                                                )
                                                .unwrap(),
                                            )
                                            .unwrap();
    
                                        let transaction = create_trade_order(
                                            zkos_create_order.input,
                                            zkos_create_order.output,
                                            zkos_create_order.signature,
                                            zkos_create_order.proof,
                                            &ContractManager::import_program(
                                                &WALLET_PROGRAM_PATH.clone(),
                                            ),
                                            Network::Mainnet,
                                            5u64,
                                        );
                                    
                                        let mut file = File::create("./transaction.txt").unwrap();
                                        file.write_all(
                                            &serde_json::to_vec(&transaction.clone()).unwrap(),
                                        )
                                        .unwrap();
    
                                        let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                            Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                            Err(arg)=>{Err(arg.to_string())}
                                        };
                                        let mut tx_hash_storage =
                                        TXHASH_STORAGE.lock().unwrap();
    
                                        let mut file =
                                        File::create("ZKOS_TRANSACTION_RPC_ENDPOINT.txt")
                                            .unwrap();
                                    file.write_all(
                                        &serde_json::to_vec(&tx_hash_result.clone())
                                            .unwrap(),
                                    )
                                    .unwrap();
    
                                    match tx_hash_result{
                                        Ok(tx_hash)=>{let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&tx_hash).unwrap(),
                                            0,
                                        );}
                                        Err(arg)=>{let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&arg).unwrap(),
                                            0,
                                        );}
                                    }
                                    drop(tx_hash_storage);
                                              
    
    
                                      
                                    }
                                    Err(arg) => {
                                        println!(
                                            "Error:ZkosTxCommand::CreateTraderOrderTX : arg:{:#?}",
                                            arg
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
    
                      
                    }
                    _ => {}
                });
                drop(buffer);
            }
            ZkosTxCommand::CreateLendOrderTX(lend_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    _ => {}
            });
            drop(buffer);
            }
            
            ZkosTxCommand::ExecuteLendOrderTX(lend_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    _ => {}
            });
            drop(buffer);
            }
            ZkosTxCommand::ExecuteTraderOrderTX(trader_order, meta) => {}
            ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX(trader_order, meta) => {}
            ZkosTxCommand::CancelTraderOrderTX(trader_order, meta) => {}
            ZkosTxCommand::CreateTraderOrderLIMITTX(trader_order, rpc_command) => {
                // let buffer = THREADPOOL_ZKOS.lock().unwrap();
                // buffer.execute(move || match trader_order.order_status {
                //     OrderStatus::FILLED => {
                //         match rpc_command {
                //             RelayerCommand::PriceTickerOrderFill(order_request, meta) => {
                //                 let zkos_data = meta.metadata.get("zkos_data").unwrap().clone();
    
                //                 let der_zkos_data: Result<Vec<u8>, std::io::Error> =
                //                     match serde_json::from_str(&zkos_data.unwrap()) {
                //                         Ok(data) => Ok(data),
                //                         Err(arg) => {
                //                             Err(std::io::Error::new(std::io::ErrorKind::Other, arg))
                //                         }
                //                     };
    
                //                 let zkos_create_order_result: Result<ZkosCreateOrder, std::io::Error> =
                //                     match der_zkos_data {
                //                         Ok(zkos_data1) => match bincode::deserialize(&zkos_data1) {
                //                             Ok(data) => Ok(data),
                //                             Err(arg) => {
                //                                 Err(std::io::Error::new(std::io::ErrorKind::Other, arg))
                //                             }
                //                         },
                //                         Err(arg) => {
                //                             Err(std::io::Error::new(std::io::ErrorKind::Other, arg))
                //                         }
                //                     };
                //                 // create transaction
                //                 match zkos_create_order_result {
                //                     Ok(zkos_create_order) => {
                //                         let mut file = File::create("zkos_create_order.txt").unwrap();
                //                         file.write_all(
                //                             &serde_json::to_vec(&zkos_create_order.clone()).unwrap(),
                //                         )
                //                         .unwrap();
                //                         let mut file_bin =
                //                             File::create("zkos_create_order_file_bin.txt").unwrap();
                //                         file_bin
                //                             .write_all(
                //                                 &serde_json::to_vec(
                //                                     &bincode::serialize(&zkos_create_order.clone())
                //                                         .unwrap(),
                //                                 )
                //                                 .unwrap(),
                //                             )
                //                             .unwrap();
    
                //                         let transaction = create_trade_order(
                //                             zkos_create_order.input,
                //                             zkos_create_order.output,
                //                             zkos_create_order.signature,
                //                             zkos_create_order.proof,
                //                             &ContractManager::import_program(
                //                                 &WALLET_PROGRAM_PATH.clone(),
                //                             ),
                //                             Network::Mainnet,
                //                             5u64,
                //                         );
                                    
                //                         let mut file = File::create("transaction.txt").unwrap();
                //                         file.write_all(
                //                             &serde_json::to_vec(&transaction.clone()).unwrap(),
                //                         )
                //                         .unwrap();
    
                //                         let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                //                             Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                //                             Err(arg)=>{Err(arg.to_string())}
                //                         };
                //                         let mut tx_hash_storage =
                //                         TXHASH_STORAGE.lock().unwrap();
    
                //                         let mut file =
                //                         File::create("ZKOS_TRANSACTION_RPC_ENDPOINT.txt")
                //                             .unwrap();
                //                     file.write_all(
                //                         &serde_json::to_vec(&tx_hash_result.clone())
                //                             .unwrap(),
                //                     )
                //                     .unwrap();
    
                //                     match tx_hash_result{
                //                         Ok(tx_hash)=>{let _ = tx_hash_storage.add(
                //                             bincode::serialize(&trader_order.uuid).unwrap(),
                //                             serde_json::to_string(&tx_hash).unwrap(),
                //                             0,
                //                         );}
                //                         Err(arg)=>{let _ = tx_hash_storage.add(
                //                             bincode::serialize(&trader_order.uuid).unwrap(),
                //                             serde_json::to_string(&arg).unwrap(),
                //                             0,
                //                         );}
                //                     }
                //                     drop(tx_hash_storage);
                                              
    
    
                                      
                //                     }
                //                     Err(arg) => {
                //                         println!(
                //                             "Error:ZkosTxCommand::CreateTraderOrderTX : arg:{:#?}",
                //                             arg
                //                         );
                //                     }
                //                 }
                //             }
                //             _ => {}
                //         }
    
                      
                //     }
                //     _ => {}
                // });
                // drop(buffer);
            }
        }
    
    
    }

}

use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
#[derive(Serialize, Deserialize)]
pub struct ZkosTxResponse {
    txHash: String,
}
impl ZkosTxResponse {
    pub fn get_txhash(
        resp: transactionapi::rpcclient::txrequest::RpcResponse<serde_json::Value>,
    ) -> String {
        let tx_hash: String = match resp.result {
            Ok(response) => match response {
                serde_json::Value::String(txHash) => {
                    match serde_json::from_str::<ZkosTxResponse>(&txHash) {
                        Ok(value) => value.txHash,
                        Err(_) => txHash,
                    }
                }
                _ => "errror".to_string(),
            },
            Err(arg) => arg.to_string(),
        };
        tx_hash
    }
}
