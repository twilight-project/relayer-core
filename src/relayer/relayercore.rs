use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::*;
use std::sync::{Arc, Mutex, RwLock};
// use stopwatch::Stopwatch;
lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_NORMAL_ORDER")));
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_URGENT_ORDER")));
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_FIFO_ORDER")));
}
pub fn client_cmd_receiver() {
    if *KAFKA_STATUS == "Enabled" {
        match receive_from_kafka_queue(
            String::from("CLIENT-REQUEST"),
            String::from("client_cmd_receiver3"),
        ) {
            Ok(rpc_cmd_receiver) => {
                let mut i = 0;
                let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);
                loop {
                    i += 1;
                    let rpc_client_cmd_request = rpc_cmd_receiver1.lock().unwrap().recv().unwrap();
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
        RpcCommand::CreateTraderOrder(rpc_request, metadata) => {
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
        RpcCommand::ExecuteTraderOrder(rpc_request, metadata) => {
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
        RpcCommand::CreateLendOrder(rpc_request, metadata) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut lend_pool = LEND_POOL_DB.lock().unwrap();
                let (tlv0, tps0) = lend_pool.get_lendpool();
                let lendorder: LendOrder = LendOrder::new_order(rpc_request, tlv0, tps0);
                lend_pool.add_transaction(LendPoolCommand::LendOrderCreateOrder(
                    command_clone,
                    lendorder.clone(),
                    lendorder.deposit,
                ));
                drop(lend_pool);
            });
            drop(buffer);
        }
        RpcCommand::ExecuteLendOrder(rpc_request, metadata) => {
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
        RpcCommand::CancelTraderOrder(rpc_request, metadata) => {
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
    }
}

pub fn relayer_event_handler(command: RelayerCommand) {
    let command_clone = command.clone();
    match command {
        RelayerCommand::FundingCycle(pool_batch_order, metadata) => {}
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
                                let mut lendpool = LEND_POOL_DB.lock().unwrap();
                                lendpool.add_transaction(
                                    LendPoolCommand::AddTraderOrderLiquidation(
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
        RelayerCommand::FundingCycleLiquidation(pool_batch_order, metadata) => {}
        RelayerCommand::RpcCommandPoolupdate(pool_batch_order, metadata) => {}
        RelayerCommand::AddTraderOrderToBatch(trader_order, rpc_request, metadata, price) => {}
    }
}
