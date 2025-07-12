use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::*;
use twilight_relayer_sdk::address::Network;
use twilight_relayer_sdk::lend::*;
use twilight_relayer_sdk::order::*;
use twilight_relayer_sdk::twilight_client_sdk::programcontroller::ContractManager;
use twilight_relayer_sdk::twilight_client_sdk::util::create_output_state_for_trade_lend_order;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::sleep;
use std::time::Duration;
use twilight_relayer_sdk::transaction::Transaction;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;
use twilight_relayer_sdk::zkvm::IOType;
use twilight_relayer_sdk::zkvm::Output;
// use stopwatch::Stopwatch;
lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_NORMAL_ORDER")));
    pub static ref THREADPOOL_NORMAL_ORDER_INSERT: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        8,
        String::from("THREADPOOL_NORMAL_ORDER_INSERT")
    ));
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(2, String::from("THREADPOOL_URGENT_ORDER")));
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_BULK_PENDING_ORDER")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER_INSERT: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_BULK_PENDING_ORDER_INSERT")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER_REMOVE: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_BULK_PENDING_ORDER_REMOVE")));
    pub static ref THREADPOOL_ZKOS_TRADER_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_ZKOS_TRADER_ORDER")));
    pub static ref THREADPOOL_ZKOS_FIFO: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_ZKOS_FIFO")));
    pub static ref CONTRACTMANAGER: Arc<Mutex<ContractManager>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let contract_manager = ContractManager::import_program(&WALLET_PROGRAM_PATH.clone());
        Arc::new(Mutex::new(contract_manager))
    };
}
pub fn client_cmd_receiver() {
        match receive_from_kafka_queue(
            RPC_CLIENT_REQUEST.clone(),
            RELAYER_CORE_GROUP_CLIENT_REQUEST.clone(),
        ) {
            Ok((rpc_cmd_receiver, tx_consumed)) => {
                let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);

                loop {
                    let rpc_client_cmd_request = rpc_cmd_receiver1.lock().unwrap();
                    match rpc_client_cmd_request.recv() {
                        Ok((rpc_command, offset_complition)) => {
                            rpc_event_handler(rpc_command, tx_consumed.clone(), offset_complition);
                        }
                        Err(arg) => {
                            println!("Relayer turn off command receiver from client-request");
                            break;
                        }
                    }
                }
            }
            Err(arg) => {
                println!("error in client_cmd_receiver: {:#?}", arg);
            }
        }
}

pub fn rpc_event_handler(
    command: RpcCommand,
    tx_consumed: crossbeam_channel::Sender<OffsetCompletion>,
    offset_complition: OffsetCompletion,
) {
    let command_clone = command.clone();
    let command_clone_for_zkos = command.clone();
    match command {
        RpcCommand::CreateTraderOrder(rpc_request, metadata, zkos_hex_string, request_id) => {
            let buffer = THREADPOOL_NORMAL_ORDER.lock().unwrap();
            buffer.execute(move || {
                let (orderdata, status) = TraderOrder::new_order(rpc_request.clone());
                let orderdata_clone = orderdata.clone();
                let orderdata_clone_for_zkos = orderdata.clone();

                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let is_order_duplicate = trader_order_db
                    .set_order_check(orderdata.account_id.clone())
                    .clone();
                drop(trader_order_db);

                if rpc_request.initial_margin > 0.0 || rpc_request.leverage <= 50.0 {
                    if is_order_duplicate {
                        if orderdata_clone.order_status == OrderStatus::FILLED {
                            let buffer_insert = THREADPOOL_NORMAL_ORDER_INSERT.lock().unwrap();
                            buffer_insert.execute(move || {
                                let (sender, zkos_receiver) = mpsc::channel();
                                zkos_order_handler(
                                    ZkosTxCommand::CreateTraderOrderTX(
                                        orderdata_clone_for_zkos,
                                        command_clone_for_zkos,
                                    ),
                                    sender,
                                );
                                match zkos_receiver.recv() {
                                    Ok(chain_message) => match chain_message {
                                        Ok(tx_hash) => {
                                            let order_state = orderdata.orderinsert_localdb(status);
                                            let mut trader_order_db =
                                                TRADER_ORDER_DB.lock().unwrap();
                                            let completed_order =
                                                trader_order_db.add(orderdata_clone, command_clone);
                                            drop(trader_order_db);
                                        }
                                        Err(verification_error) => {
                                            let mut trader_order_db =
                                                TRADER_ORDER_DB.lock().unwrap();
                                            let _ = trader_order_db.remove_order_check(
                                                orderdata_clone.account_id.clone(),
                                            );
                                            drop(trader_order_db);
                                        }
                                    },
                                    Err(arg) => {
                                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                        let _ = trader_order_db
                                            .remove_order_check(orderdata_clone.account_id.clone());
                                        drop(trader_order_db);
                                    }
                                }
                                match tx_consumed.send(offset_complition) {
                                    Ok(_) => {}
                                    Err(_) => {}
                                }
                            });
                            drop(buffer_insert);
                        } else if orderdata_clone.order_status == OrderStatus::PENDING {
                            //submit limit order after checking from chain or your db about order existanse of utxo
                            let order_state = orderdata.orderinsert_localdb(false);
                            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                            let completed_order =
                                trader_order_db.add(orderdata_clone, command_clone);
                            drop(trader_order_db);
                            Event::new(
                                Event::TxHash(
                                    completed_order.uuid,
                                    completed_order.account_id,
                                    request_id.clone(),
                                    completed_order.order_type,
                                    OrderStatus::PENDING,
                                    ServerTime::now().epoch,
                                    None,
                                    request_id.clone(),
                                ),
                                format!("tx_limit_submit-{:?}",request_id),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            match tx_consumed.send(offset_complition) {
                                Ok(_) => {}
                                Err(_) => {}
                            }
                        }
                    } else {
                        // send event for txhash with error saying order already exist in the relayer
                        Event::new(
                            Event::TxHash(
                                orderdata.uuid,
                                orderdata.account_id,
                                "Duplicate order found in the database with same public key"
                                    .to_string(),
                                orderdata.order_type,
                                OrderStatus::DuplicateOrder,
                                ServerTime::now().epoch,
                                None,
                                request_id,
                            ),
                            String::from("tx_duplicate_error"),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                } else {
                    Event::new(
                        Event::TxHash(
                            orderdata.uuid,
                            orderdata.account_id,
                            "Invalid Initial margin>0 or Leverage<=50".to_string(),
                            orderdata.order_type,
                            OrderStatus::CANCELLED,
                            ServerTime::now().epoch,
                            None,
                            request_id,
                        ),
                        String::from("tx_wrong_parameter_error"),
                        CORE_EVENT_LOG.clone().to_string(),
                    );
                    match tx_consumed.send(offset_complition) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
            });
            drop(buffer);
        }
        RpcCommand::ExecuteTraderOrder(rpc_request, metadata, zkos_hex_string, request_id) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let execution_price = rpc_request.execution_price.clone();
                let current_price = get_localdb("CurrentPrice");
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let order_detail_wraped = trader_order_db.get_mut(rpc_request.uuid);
                drop(trader_order_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        let mut order_updated_clone = order.clone();
                        match order_updated_clone.order_status {
                            OrderStatus::FILLED => {
                                let (payment, order_status) = order_updated_clone
                                    .check_for_settlement(
                                        execution_price,
                                        current_price,
                                        rpc_request.order_type,
                                    );
                                match order_status {
                                    OrderStatus::SETTLED => {
                                        let order_clone = order_updated_clone.clone();

                                        let mut lendpool = LEND_POOL_DB.lock().unwrap();

                                        let next_output_state =
                                            create_output_state_for_trade_lend_order(
                                                (lendpool.nonce + 1) as u32,
                                                lendpool
                                                    .last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_script_address()
                                                    .unwrap()
                                                    .clone(),
                                                lendpool
                                                    .last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_owner_address()
                                                    .clone()
                                                    .unwrap()
                                                    .clone(),
                                                (lendpool.total_locked_value.round()
                                                    - payment.round())
                                                    as u64,
                                                lendpool.total_pool_share.round() as u64,
                                                0,
                                            );
                                        let (sender, zkos_receiver) = mpsc::channel();
                                        zkos_order_handler(
                                            ZkosTxCommand::ExecuteTraderOrderTX(
                                                order_clone.clone(),
                                                command_clone.clone(),
                                                lendpool.last_output_state.clone(),
                                                next_output_state.clone(),
                                            ),
                                            sender,
                                        );

                                        match zkos_receiver.recv() {
                                            Ok(chain_message) => match chain_message {
                                                Ok(tx_hash) => {
                                                    *order = order_clone.clone();
                                                    order_updated_clone.order_remove_from_localdb();
                                                    drop(order);
                                                    lendpool.add_transaction(
                                                        LendPoolCommand::AddTraderOrderSettlement(
                                                            command_clone,
                                                            order_clone.clone(),
                                                            payment,
                                                            next_output_state,
                                                        ),
                                                    );
                                                }
                                                Err(verification_error) => {}
                                            },
                                            Err(arg) => {}
                                        }

                                        drop(lendpool);
                                    }
                                    OrderStatus::FILLED => {
                                        drop(order);
                                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                        trader_order_db.aggrigate_log_sequence += 1;
                                        trader_order_db.set_zkos_string_on_limit_update(
                                            order_updated_clone.uuid.clone(),
                                            zkos_hex_string,
                                        );
                                        Event::new(
                                            Event::TraderOrderLimitUpdate(order_updated_clone.clone(), command_clone.clone(), trader_order_db.aggrigate_log_sequence),
                                            format!("settle_limit_order-{}", order_updated_clone.uuid),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
                                        drop(trader_order_db);
                                        Event::new(
                                            Event::TxHash(
                                                order_updated_clone.uuid,
                                                order_updated_clone.account_id,
                                                request_id.clone(),
                                                order_updated_clone.order_type,
                                                OrderStatus::PENDING,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id.clone(),
                                            ),
                                            format!("tx_settle_limit_submit-{:?}",request_id),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
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
                                // send event for txhash with error saying order already exist in the relayer
                                Event::new(
                                    Event::TxHash(
                                        rpc_request.uuid,
                                        rpc_request.account_id,
                                        "Order not found or invalid order status !!".to_string(),
                                        rpc_request.order_type,
                                        OrderStatus::OrderNotFound,
                                        ServerTime::now().epoch,
                                        None,
                                        request_id,
                                    ),
                                    String::from("trader_tx_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                        // send event for txhash with error saying order already exist in the relayer
                        Event::new(
                            Event::TxHash(
                                rpc_request.uuid,
                                rpc_request.account_id,
                                format!("Error found:{:#?}", arg),
                                rpc_request.order_type,
                                OrderStatus::OrderNotFound,
                                ServerTime::now().epoch,
                                None,
                                request_id,
                            ),
                            String::from("trader_tx_not_found_error"),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                }
            });
            drop(buffer);
        }
        RpcCommand::CreateLendOrder(rpc_request, metadata, zkos_hex_string, request_id) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut lend_pool = LEND_POOL_DB.lock().unwrap();
                let is_order_exist: bool;

                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                is_order_exist = lendorder_db.set_order_check(rpc_request.account_id.clone());
                drop(lendorder_db);

                if is_order_exist {
                    let (tlv0, tps0) = lend_pool.get_lendpool();
                    let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, tlv0, tps0);
                    lendorder.order_status = OrderStatus::FILLED;
                    // let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, 20048615383.0, 2000000.0);

                    let next_output_state = create_output_state_for_trade_lend_order(
                        (lend_pool.nonce + 1) as u32,
                        lend_pool
                            .last_output_state
                            .clone()
                            .as_output_data()
                            .get_script_address()
                            .unwrap()
                            .clone(),
                        lend_pool
                            .last_output_state
                            .clone()
                            .as_output_data()
                            .get_owner_address()
                            .clone()
                            .unwrap()
                            .clone(),
                        lendorder.tlv1 as u64,
                        lendorder.tps1 as u64,
                        0,
                    );

                    let (sender, zkos_receiver) = mpsc::channel();
                    zkos_order_handler(
                        ZkosTxCommand::CreateLendOrderTX(
                            lendorder.clone(),
                            command_clone.clone(),
                            lend_pool.last_output_state.clone(),
                            next_output_state.clone(),
                        ),
                        sender,
                    );

                    match zkos_receiver.recv() {
                        Ok(chain_message) => match chain_message {
                            Ok(tx_hash) => {
                                lend_pool.add_transaction(LendPoolCommand::LendOrderCreateOrder(
                                    command_clone,
                                    lendorder.clone(),
                                    lendorder.deposit,
                                    next_output_state,
                                ));
                            }
                            Err(verification_error) => {
                                println!(
                                    "Error in line relayercore.rs 299 : {:?}",
                                    verification_error
                                );
                            }
                        },
                        Err(arg) => {
                            println!("Error in line relayercore.rs 304 : {:?}", arg);
                        }
                    }
                    match tx_consumed.send(offset_complition) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                } else {
                    // send event for txhash with error saying order already exist in the relayer
                    Event::new(
                        Event::TxHash(
                            Uuid::new_v4(),
                            rpc_request.account_id.clone(),
                            "Duplicate order found in the database with same public key"
                                .to_string(),
                            OrderType::LEND,
                            OrderStatus::DuplicateOrder,
                            ServerTime::now().epoch,
                            None,
                            request_id,
                        ),
                        String::from("tx_duplicate_error"),
                        CORE_EVENT_LOG.clone().to_string(),
                    );
                    match tx_consumed.send(offset_complition) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }

                drop(lend_pool);
            });
            drop(buffer);
        }
        RpcCommand::ExecuteLendOrder(rpc_request, metadata, zkos_hex_string, request_id) => {
            let buffer = THREADPOOL_FIFO_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut lend_pool = LEND_POOL_DB.lock().unwrap();
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                let order_detail_wraped = lendorder_db.get_mut(rpc_request.uuid);
                drop(lendorder_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        let mut order_updated_clone = order.clone();
                        match order_updated_clone.order_status {
                            OrderStatus::FILLED => {
                                let (tlv2, tps2) = lend_pool.get_lendpool();
                                match order_updated_clone.calculatepayment_localdb(tlv2, tps2) {
                                    Ok(_) => {
                                        let mut order_clone = order_updated_clone.clone();
                                        order_clone.order_status = OrderStatus::SETTLED;
                                        order_clone.exit_nonce = lend_pool.nonce + 1;

                                        let next_output_state =
                                            create_output_state_for_trade_lend_order(
                                                (lend_pool.nonce + 1) as u32,
                                                lend_pool
                                                    .last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_script_address()
                                                    .unwrap()
                                                    .clone(),
                                                lend_pool
                                                    .last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_owner_address()
                                                    .clone()
                                                    .unwrap()
                                                    .clone(),
                                                order_clone.tlv3.round() as u64,
                                                order_clone.tps3.round() as u64,
                                                0,
                                            );

                                        let (sender, zkos_receiver) = mpsc::channel();
                                        zkos_order_handler(
                                            ZkosTxCommand::ExecuteLendOrderTX(
                                                order_clone.clone(),
                                                command_clone.clone(),
                                                lend_pool.last_output_state.clone(),
                                                next_output_state.clone(),
                                            ),
                                            sender,
                                        );

                                        match zkos_receiver.recv() {
                                            Ok(chain_message) => match chain_message {
                                                Ok(tx_hash) => {
                                                    *order = order_clone.clone();
                                                    drop(order);
                                                    lend_pool.add_transaction(
                                                        LendPoolCommand::LendOrderSettleOrder(
                                                            command_clone,
                                                            order_clone.clone(),
                                                            order_clone.nwithdraw,
                                                            next_output_state,
                                                        ),
                                                    );
                                                }
                                                Err(verification_error) => {
                                                    println!(
                                                        "Error in line relayercore.rs 299 : {:?}",
                                                        verification_error
                                                    );
                                                }
                                            },
                                            Err(arg) => {
                                                println!(
                                                    "Error in line relayercore.rs 304 : {:?}",
                                                    arg
                                                );
                                            }
                                        }
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
                                // send event for txhash with error saying order already exist in the relayer
                                Event::new(
                                    Event::TxHash(
                                        rpc_request.uuid,
                                        rpc_request.account_id,
                                        "Order not found or invalid order status !!".to_string(),
                                        OrderType::LEND,
                                        OrderStatus::OrderNotFound,
                                        ServerTime::now().epoch,
                                        None,
                                        request_id,
                                    ),
                                    String::from("lend_tx_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    Err(arg) => {
                        drop(lend_pool);
                        println!("Error found:{:#?}", arg);
                        // send event for txhash with error saying order already exist in the relayer
                        Event::new(
                            Event::TxHash(
                                rpc_request.uuid,
                                rpc_request.account_id,
                                format!("Error found:{:#?}", arg),
                                OrderType::LEND,
                                OrderStatus::Error,
                                ServerTime::now().epoch,
                                None,
                                request_id,
                            ),
                            String::from("lend_tx_not_found_error"),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                }
            });
            drop(buffer);
        }
        RpcCommand::CancelTraderOrder(rpc_request, metadata, zkos_hex_string, request_id) => {
            let buffer = THREADPOOL_URGENT_ORDER.lock().unwrap();
            buffer.execute(move || {
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let order_detail_wraped = trader_order_db.get_mut(rpc_request.uuid);
                drop(trader_order_db);
                match order_detail_wraped {
                    Ok(order_detail) => {
                        let mut order = order_detail.write().unwrap();
                        // println!("FILLED order:{:#?}", order);
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
                                        Event::new(
                                            Event::TxHash(
                                                rpc_request.uuid,
                                                rpc_request.account_id,
                                                "Order successfully cancelled !!".to_string(),
                                                rpc_request.order_type,
                                                OrderStatus::CANCELLED,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id,
                                            ),
                                            format!("tx_hash_result-{:?}","Order successfully cancelled !!".to_string()),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
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
                                Event::new(
                                    Event::TxHash(
                                        rpc_request.uuid,
                                        rpc_request.account_id,
                                        "Order not found or invalid order status !!".to_string(),
                                        rpc_request.order_type,
                                        OrderStatus::OrderNotFound,
                                        ServerTime::now().epoch,
                                        None,
                                        request_id,
                                    ),
                                    String::from("trader_order_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    Err(arg) => {
                        println!("Error found:{:#?}", arg);
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
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
                format!("insert_fundingrate-{}", ServerTime::now().epoch),
                CORE_EVENT_LOG.clone().to_string(),
            );
            let mut lendpool = LEND_POOL_DB.lock().unwrap();
            lendpool.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(command_clone));
            drop(lendpool);
        }
        RelayerCommand::PriceTickerLiquidation(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<(
                Result<Arc<RwLock<TraderOrder>>, std::io::Error>,
                Option<Output>,
            )> = Vec::new();
            let output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let uuid_to_byte = match bincode::serialize(&order_id) {
                    Ok(uuid_v_u8) => uuid_v_u8,
                    Err(_) => Vec::new(),
                };
                let output_option = match output_hex_storage.get_utxo_by_id(uuid_to_byte, 0) {
                    Ok(output) => output,
                    Err(_) => None,
                };
                let orderdetail = (trader_order_db.get_mut(order_id), output_option);
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            drop(output_hex_storage);
            let buffer = THREADPOOL_BULK_PENDING_ORDER_REMOVE.lock().unwrap();
            for (order_detail_wraped, output_option) in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || {
                    match order_detail_wraped {
                        Ok(order_detail) => {
                            let mut order_mut_ref = order_detail.write().unwrap();
                            let mut order: TraderOrder = order_mut_ref.clone();
                            order_mut_ref.order_status=OrderStatus::LIQUIDATE;
                            drop(order_mut_ref);
                            match order.order_status {
                                OrderStatus::FILLED | OrderStatus::LIQUIDATE=> {
                                    order.order_status = OrderStatus::LIQUIDATE;
                                    //update batch process
                                    let payment = order.liquidate(current_price_clone);
                                        // println!("order:{:?}",order);
                                    let mut lendpool = LEND_POOL_DB.lock().unwrap();

                                    let next_output_state =
                                        create_output_state_for_trade_lend_order(
                                            (lendpool.nonce + 1) as u32,
                                            lendpool
                                                .last_output_state
                                                .clone()
                                                .as_output_data()
                                                .get_script_address()
                                                .unwrap()
                                                .clone(),
                                            lendpool
                                                .last_output_state
                                                .clone()
                                                .as_output_data()
                                                .get_owner_address()
                                                .clone()
                                                .unwrap()
                                                .clone(),
                                            (lendpool.total_locked_value.round() - payment.round())
                                                as u64,
                                            lendpool.total_pool_share.round() as u64,
                                            0,
                                        );

                                        let (sender, zkos_receiver) = mpsc::channel(); 
                                        zkos_order_handler(
                                        ZkosTxCommand::RelayerCommandTraderOrderLiquidateTX(
                                            order.clone(),
                                            output_option,
                                            lendpool.last_output_state.clone(),
                                            next_output_state.clone(),
                                        ), sender);


                                    match zkos_receiver.recv() {
                                        Ok(chain_message) => {
                                            match chain_message {
                                                Ok(tx_hash) => {
                                                    PositionSizeLog::remove_order(
                                                        order.position_type.clone(),
                                                        order.positionsize.clone(),
                                                    );
                                                    let mut order_mut_ref = order_detail.write().unwrap();
                                                    *order_mut_ref = order.clone();
                                                    drop(order_mut_ref);
                                                    // println!("locking mutex LEND_POOL_DB");

                                                    lendpool.add_transaction(
                                                        LendPoolCommand::AddTraderOrderLiquidation(
                                                            RelayerCommand::PriceTickerLiquidation(
                                                                vec![order.uuid.clone()],
                                                                metadata_clone,
                                                                current_price_clone,
                                                            ),
                                                            order.clone(),
                                                            payment,
                                                            next_output_state,
                                                        ),
                                                    );
                                                    // println!("dropping mutex LEND_POOL_DB");
                                                }
                                                Err(verification_error) => {
                                                    println!(
                                                        "Error in line relayercore.rs 774 : {:?}, uuid: {:?}, nonce: {:?}, \n next_output:{:?}",
                                                        verification_error,order.uuid, lendpool.nonce,next_output_state
                                                    );
                                                  
                                                }
                                            }
                                        }
                                        Err(arg) => {
                                            println!(
                                                "Error in line relayercore.rs 827 : {:?}",
                                                arg
                                            );
                                        }
                                    }

                                    drop(lendpool);
                                }
                                _ => {
                                    // drop(order_mut_ref);
                                    println!("Invalid order status !!\n");
                                }
                            }
                        }
                        Err(arg) => {
                            println!("Error found:{:#?}", arg);
                        }
                    }
                });
            }
            drop(buffer);
        }
        RelayerCommand::PriceTickerOrderFill(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<(
                Result<Arc<RwLock<TraderOrder>>, std::io::Error>,
                Option<String>,
            )> = Vec::new();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let orderdetail = (
                    trader_order_db.get_mut(order_id),
                    trader_order_db.get_zkos_string(order_id),
                );
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            let buffer = THREADPOOL_BULK_PENDING_ORDER.lock().unwrap();
            for (order_detail_wraped, zkos_msg_hex) in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || {
                    match order_detail_wraped {
                        Ok(order_detail) => {
                            let order_mut_clone = Arc::clone(&order_detail);
                            let order_read_only_ref = order_detail.read().unwrap();
                            let order = order_read_only_ref.clone();
                            match order.order_status {
                                OrderStatus::PENDING => {
                                    let buffer_insert =
                                        THREADPOOL_BULK_PENDING_ORDER_INSERT.lock().unwrap();
                                    buffer_insert.execute(move || {
                                        let mut order_mut_ref = order_mut_clone.write().unwrap();
                                        let order = order_mut_ref.clone();
                                        let (mut update_order_detail, order_status) =
                                            order.pending_order(current_price_clone);

                                            let (sender, zkos_receiver) = mpsc::channel();
                                             zkos_order_handler(
                                            ZkosTxCommand::CreateTraderOrderLIMITTX(
                                                update_order_detail.clone(),
                                                zkos_msg_hex
                                            ), sender
                                        );
                                        match zkos_receiver.recv() {
                                            Ok(chain_message) => {
                                                match chain_message {
                                                    Ok(tx_hash) => {
                                                        let mut filled_order =
                                                            update_order_detail.orderinsert_localdb(
                                                                order_status
                                                            );
                                                        filled_order.order_status =
                                                            OrderStatus::FILLED;
                                                        let mut trader_order_db =
                                                            TRADER_ORDER_DB.lock().unwrap();

                                                        *order_mut_ref = filled_order.clone();
                                                        drop(order_mut_ref);
                                                        let _ = trader_order_db.update(
                                                            filled_order.clone(),
                                                            RelayerCommand::PriceTickerOrderFill(
                                                                vec![filled_order.uuid.clone()],
                                                                metadata_clone,
                                                                current_price_clone
                                                            )
                                                        );
                                                        drop(trader_order_db);
                                                    }
                                                    Err(verification_error) => {
                                                        let mut trader_order_db =
                                                            TRADER_ORDER_DB.lock().unwrap();
                                                        drop(order_mut_ref);

                                                        update_order_detail.order_status =
                                                            OrderStatus::CANCELLED;

                                                        let dummy_rpccommand =
                                                            RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                                                                update_order_detail.clone(),
                                                                metadata_clone,
                                                                current_price_clone
                                                            );
                                                        //remove invalid order from the localdb
                                                        let _ = trader_order_db.remove(
                                                            update_order_detail.clone(),
                                                            dummy_rpccommand.clone()
                                                        );
                                                        drop(trader_order_db);

                                                        // Event::new(
                                                        //     Event::TxHash(
                                                        //         update_order_detail.uuid,
                                                        //         update_order_detail.account_id,
                                                        //         verification_error,
                                                        //         OrderType::LIMIT,
                                                        //         OrderStatus::CANCELLED,
                                                        //         ServerTime::now().epoch,
                                                        //         None
                                                        //     ),
                                                        //     String::from("tx_hash_error"),
                                                        //     CORE_EVENT_LOG.clone().to_string()
                                                        // );
                                                    }
                                                }
                                            }
                                            Err(arg) => {
                                                let mut trader_order_db =
                                                    TRADER_ORDER_DB.lock().unwrap();
                                                drop(order_mut_ref);
                                                update_order_detail.order_status =
                                                    OrderStatus::CANCELLED;

                                                let dummy_rpccommand =
                                                    RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                                                        update_order_detail.clone(),
                                                        metadata_clone,
                                                        current_price_clone
                                                    );
                                                //remove invalid order from the localdb
                                                let _ = trader_order_db.remove(
                                                    update_order_detail.clone(),
                                                    dummy_rpccommand.clone()
                                                );
                                                drop(trader_order_db);

                                                // Event::new(
                                                //     Event::TxHash(
                                                //         update_order_detail.uuid,
                                                //         update_order_detail.account_id,
                                                //         format!("Error found:{:#?}", arg),
                                                //         OrderType::LIMIT,
                                                //         OrderStatus::CANCELLED,
                                                //         ServerTime::now().epoch,
                                                //         None
                                                //     ),
                                                //     String::from("Receiver_error_limit"),
                                                //     CORE_EVENT_LOG.clone().to_string()
                                                // );
                                            }
                                        }
                                    });
                                }
                                _ => {
                                    drop(order);
                                    println!("Invalid order status !!\n");
                                }
                            }
                            drop(order_read_only_ref);
                        }
                        Err(arg) => {
                            println!("Error found:{:#?}", arg);
                        }
                    }
                });
            }
            drop(buffer);
        }
        RelayerCommand::PriceTickerOrderSettle(order_id_array, metadata, currentprice) => {
            let mut orderdetails_array: Vec<(
                Result<Arc<RwLock<TraderOrder>>, std::io::Error>,
                Option<String>,
            )> = Vec::new();
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            for order_id in order_id_array {
                let orderdetail = (
                    trader_order_db.get_mut(order_id),
                    trader_order_db.get_zkos_string(order_id),
                );
                orderdetails_array.push(orderdetail);
            }
            drop(trader_order_db);
            let buffer = THREADPOOL_BULK_PENDING_ORDER_REMOVE.lock().unwrap();
            for (order_detail_wraped, zkos_msg_hex) in orderdetails_array {
                let current_price_clone = currentprice.clone();
                let metadata_clone = metadata.clone();
                buffer.execute(move || {
                    match order_detail_wraped {
                        Ok(order_detail) => {
                            let mut order_mut_ref = order_detail.write().unwrap();
                            let mut order: TraderOrder = order_mut_ref.clone();
                            match order.order_status {
                                OrderStatus::FILLED => {
                                    let (payment, order_status) = order.check_for_settlement(
                                        current_price_clone,
                                        current_price_clone,
                                        OrderType::MARKET
                                    );

                                    match order_status {
                                        OrderStatus::SETTLED => {
                                            let order_clone = order.clone();

                                            let mut lendpool = LEND_POOL_DB.lock().unwrap();

                                            let next_output_state =
                                                create_output_state_for_trade_lend_order(
                                                    (lendpool.nonce + 1) as u32,
                                                    lendpool.last_output_state
                                                        .clone()
                                                        .as_output_data()
                                                        .get_script_address()
                                                        .unwrap()
                                                        .clone(),
                                                    lendpool.last_output_state
                                                        .clone()
                                                        .as_output_data()
                                                        .get_owner_address()
                                                        .clone()
                                                        .unwrap()
                                                        .clone(),
                                                    (lendpool.total_locked_value.round() -
                                                        payment.round()) as u64,
                                                    lendpool.total_pool_share.round() as u64,
                                                    0
                                                );

                                                let (sender, zkos_receiver) = mpsc::channel();
                                                 zkos_order_handler(
                                                ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX(
                                                    order_clone.clone(),
                                                    zkos_msg_hex,
                                                    lendpool.last_output_state.clone(),
                                                    next_output_state.clone()
                                                ),sender
                                            );


                                            match zkos_receiver.recv() {
                                                Ok(chain_message) => {
                                                    match chain_message {
                                                        Ok(tx_hash) => {
                                                            *order_mut_ref = order_clone.clone();
                                                            order_clone.order_remove_from_localdb();
                                                            drop(order_mut_ref);
                                                            lendpool.add_transaction(
                                                                LendPoolCommand::AddTraderLimitOrderSettlement(
                                                                    RelayerCommand::PriceTickerOrderSettle(
                                                                        vec![
                                                                            order_clone.uuid.clone()
                                                                        ],
                                                                        metadata_clone,
                                                                        current_price_clone
                                                                    ),
                                                                    order_clone.clone(),
                                                                    payment,
                                                                    next_output_state
                                                                )
                                                            );
                                                        }
                                                        Err(verification_error) => {
                                                            println!(
                                                                "Error in line relayercore.rs 1071 : {:?}",
                                                                verification_error
                                                            );
                                                            // settle limit removed from the db, need to place new limit/market settle request
                                                        }
                                                    }
                                                }
                                                Err(arg) => {
                                                    println!(
                                                        "Error in line relayercore.rs 1080 : {:?}",
                                                        arg
                                                    );
                                                }
                                            }

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
                CORE_EVENT_LOG.clone().to_string(),
            );
        }
        RelayerCommand::UpdateFees(order_filled_on_market, order_filled_on_limit, order_settled_on_market, order_settled_on_limit) => {
            let event_time = systemtime_to_utc();
            let mut local_storage = LOCALDB.lock().unwrap();
            let old_order_filled_on_market = local_storage.insert(FeeType::FilledOnMarket.into(), order_filled_on_market);
            let old_order_filled_on_limit = local_storage.insert(FeeType::FilledOnLimit.into(), order_filled_on_limit);
            let old_order_settled_on_market = local_storage.insert(FeeType::SettledOnMarket.into(), order_settled_on_market);
            let old_order_settled_on_limit = local_storage.insert(FeeType::SettledOnLimit.into(), order_settled_on_limit);
            drop(local_storage);
            if old_order_filled_on_market.is_some() {
                println!("OrderFilledOnMarket fee updated from {} to {} at {}", old_order_filled_on_market.unwrap_or(0.0), order_filled_on_market, event_time);
            }
            if old_order_filled_on_limit.is_some() {
                println!("OrderFilledOnLimit fee updated from {} to {} at {}", old_order_filled_on_limit.unwrap_or(0.0), order_filled_on_limit, event_time);
            }
            if old_order_settled_on_limit.is_some() {
                println!("OrderSettledOnLimit fee updated from {} to {} at {}", old_order_settled_on_limit.unwrap_or(0.0), order_settled_on_limit, event_time);
            }
            if old_order_settled_on_market.is_some() {
                println!("OrderSettledOnMarket fee updated from {} to {} at {}", old_order_settled_on_market.unwrap_or(0.0), order_settled_on_market, event_time);
            }
            println!("Fee update completed");
            Event::new(
                Event::FeeUpdate(RelayerCommand::UpdateFees(order_filled_on_market, order_filled_on_limit, order_settled_on_market, order_settled_on_limit), event_time.clone()),
                format!("FeeUpdate-{}", event_time),
                CORE_EVENT_LOG.clone().to_string(),
            );

        }
    }
}

pub fn zkos_order_handler(
    command: ZkosTxCommand,
    sender: mpsc::Sender<Result<String, std::string::String>>,
) {
    let command_clone = command.clone();
    // let (sender, receiver) = mpsc::channel();
    // let receiver_mutex = Arc::new(Mutex::new(receiver));
    if ENABLE_ZKOS_CHAIN_TRANSACTION.clone() {
        match command {
            ZkosTxCommand::CreateTraderOrderTX(trader_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || {
                    match trader_order.order_status {
                        OrderStatus::FILLED => {
                            match rpc_command {
                                RpcCommand::CreateTraderOrder(
                                    order_request,
                                    meta,
                                    zkos_hex_string,
                                    request_id,
                                ) => {
                                    let tx_result: Result<Transaction, String> = match
                                        hex::decode(zkos_hex_string)
                                    {
                                        Ok(bytes) =>
                                            match bincode::deserialize(&bytes) {
                                                Ok(tx) => Ok(tx),
                                                Err(arg) => Err(arg.to_string()),
                                            }
                                        Err(arg) => Err(arg.to_string()),
                                    };

                                    
                                    // create transaction
                                    match tx_result {
                                        Ok(tx) => {
                                            let updated_tx_result = update_memo_tx_client_order(
                                                &tx,
                                                trader_order.entryprice.round() as u64,
                                                trader_order.positionsize.round() as u64,
                                                trader_order.fee_filled.round() as u64
                                            );
                                            match updated_tx_result {
                                                Ok(tx_and_outputmemo) => {
                                                    let output = tx_and_outputmemo.get_output();
                                                    let output_memo_hex = match
                                                        bincode::serialize(&output.clone())
                                                    {
                                                        Ok(output_memo_bin) => {
                                                            Some(hex::encode(&output_memo_bin))
                                                        }
                                                        Err(_) => None,
                                                    };
                                                    
                                                    let transaction = tx_and_outputmemo.get_tx();
                                               
                                                    let tx_hash_result =
                                                        twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(
                                                            transaction
                                                        );
                                                   
                                                    let sender_clone = sender.clone();
                                                    match sender_clone.send(tx_hash_result.clone()) {
                                                        Ok(_) => {}
                                                        Err(_) => {
                                                            println!("error in sender");
                                                        }
                                                    }

                                                    match tx_hash_result {
                                                        Ok(tx_hash) => {
                                                            let mut output_hex_storage =
                                                                OUTPUT_STORAGE.lock().unwrap();
                                                            let _ = output_hex_storage.add(
                                                                trader_order.uuid_to_byte(),
                                                                Some(output),
                                                                0
                                                            );
                                                            drop(output_hex_storage);
                                                            Event::new(
                                                                Event::TxHash(
                                                                    trader_order.uuid,
                                                                    trader_order.account_id,
                                                                    tx_hash.clone(),
                                                                    trader_order.order_type,
                                                                    trader_order.order_status,
                                                                    ServerTime::now().epoch,
                                                                    output_memo_hex,
                                                                    request_id
                                                                ),
                                                                format!("tx_hash_result-{:?}",tx_hash),
                                                                CORE_EVENT_LOG.clone().to_string()
                                                            );
                                                        }
                                                        Err(arg) => {
                                                            Event::new(
                                                                Event::TxHash(
                                                                    trader_order.uuid,
                                                                    trader_order.account_id,
                                                                    arg,
                                                                    trader_order.order_type,
                                                                    OrderStatus::RejectedFromChain,
                                                                    ServerTime::now().epoch,
                                                                    None,
                                                                    request_id
                                                                ),
                                                                String::from("tx_hash_error"),
                                                                CORE_EVENT_LOG.clone().to_string()
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(arg) => {
                                                    Event::new(
                                                        Event::TxHash(
                                                            trader_order.uuid,
                                                            trader_order.account_id,
                                                            arg.to_string(),
                                                            trader_order.order_type,
                                                            OrderStatus::UtxoError,
                                                            ServerTime::now().epoch,
                                                            None,
                                                            request_id
                                                        ),
                                                        String::from("tx_hash_error"),
                                                        CORE_EVENT_LOG.clone().to_string()
                                                    );
                                                }
                                            }
                                        }
                                        Err(arg) => {
                                            println!(
                                                "Error:ZkosTxCommand::CreateTraderOrderTX : arg:{:#?}",
                                                arg
                                            );
                                            Event::new(
                                                Event::TxHash(
                                                    trader_order.uuid,
                                                    trader_order.account_id,
                                                    arg.to_string(),
                                                    trader_order.order_type,
                                                    OrderStatus::SerializationError,
                                                    ServerTime::now().epoch,
                                                    None,
                                                    request_id
                                                ),
                                                String::from("tx_hash_error"),
                                                CORE_EVENT_LOG.clone().to_string()
                                            );
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                });
                drop(buffer);
            }

            ZkosTxCommand::CreateLendOrderTX(
                lend_order,
                rpc_command,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || {
                    match rpc_command {
                        RpcCommand::CreateLendOrder(
                            order_request,
                            meta,
                            zkos_hex_string,
                            request_id,
                        ) => {
                            let zkos_create_order_result =
                                ZkosCreateOrder::decode_from_hex_string(zkos_hex_string);

                            match zkos_create_order_result {
                                Ok(zkos_create_order) => {
                                    let contract_owner_sk = get_sk_from_fixed_wallet();
                                    let contract_owner_pk = get_pk_from_fixed_wallet();

                                    let lock_error = get_lock_error_for_lend_create(
                                        lend_order.clone()
                                    );

                                    let result_poolshare_output = update_lender_output_memo(
                                        zkos_create_order.output.clone(),
                                        (lend_order.npoolshare / 10000.0).round() as u64
                                    );

                                    match result_poolshare_output {
                                        Ok(poolshare_output) => {
                                            let output_memo_hex = match
                                                bincode::serialize(&poolshare_output.clone())
                                            {
                                                Ok(output_memo_bin) => {
                                                    Some(hex::encode(&output_memo_bin))
                                                }
                                                Err(_) => None,
                                            };

                                            let transaction = create_lend_order_transaction(
                                                zkos_create_order.input.clone(),
                                                poolshare_output.clone(),
                                                last_state_output.clone(),
                                                next_state_output.clone(),
                                                zkos_create_order.signature,
                                                zkos_create_order.proof,
                                                &ContractManager::import_program(
                                                    &WALLET_PROGRAM_PATH.clone()
                                                ),
                                                Network::Mainnet,
                                                1u64,
                                                last_state_output
                                                    .as_output_data()
                                                    .get_owner_address()
                                                    .unwrap()
                                                    .clone(),
                                                lock_error,
                                                contract_owner_sk,
                                                contract_owner_pk
                                            );

                                            let tx_hash_result = match transaction {
                                                Ok(tx) => {
                                                    
                                                    transaction_queue_to_confirm_relayer_latest_state(last_state_output.clone(),
                                                        tx,next_state_output.clone()
                                                    )
                                                }
                                                Err(arg) => { Err(arg.to_string()) }
                                            };
                                            let sender_clone = sender.clone();
                                            match sender_clone.send(tx_hash_result.clone()) {
                                                Ok(_) => {}
                                                Err(_) => {
                                                    println!("error in sender");
                                                }
                                            }

                                            match tx_hash_result {
                                                Ok(tx_hash) => {
                                                    Event::new(
                                                        Event::TxHash(
                                                            lend_order.uuid,
                                                            lend_order.account_id,
                                                            tx_hash.clone(),
                                                            lend_order.order_type,
                                                            lend_order.order_status,
                                                            ServerTime::now().epoch,
                                                            output_memo_hex,
                                                            request_id
                                                        ),
                                                        format!("tx_hash_result-{:?}",tx_hash),
                                                        CORE_EVENT_LOG.clone().to_string()
                                                    );
                                                }
                                                Err(arg) => {
                                                    Event::new(
                                                        Event::TxHash(
                                                            lend_order.uuid,
                                                            lend_order.account_id,
                                                            arg,
                                                            lend_order.order_type,
                                                            OrderStatus::RejectedFromChain,
                                                            ServerTime::now().epoch,
                                                            None,
                                                            request_id
                                                        ),
                                                        String::from("tx_hash_error"),
                                                        CORE_EVENT_LOG.clone().to_string()
                                                    );
                                                }
                                            }
                                        }
                                        Err(arg) => {
                                            Event::new(
                                                Event::TxHash(
                                                    lend_order.uuid,
                                                    lend_order.account_id,
                                                    arg.to_string(),
                                                    lend_order.order_type,
                                                    OrderStatus::UtxoError,
                                                    ServerTime::now().epoch,
                                                    None,
                                                    request_id
                                                ),
                                                String::from("tx_hash_error"),
                                                CORE_EVENT_LOG.clone().to_string()
                                            );
                                        }
                                    }
                                }
                                Err(arg) => {
                                    println!(
                                        "Error:ZkosTxCommand::CreateLendOrderTX : arg:{:#?}",
                                        arg
                                    );
                                    Event::new(
                                        Event::TxHash(
                                            lend_order.uuid,
                                            lend_order.account_id,
                                            arg,
                                            lend_order.order_type,
                                            OrderStatus::SerializationError,
                                            ServerTime::now().epoch,
                                            None,
                                            request_id
                                        ),
                                        String::from("tx_hash_error"),
                                        CORE_EVENT_LOG.clone().to_string()
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                });
                drop(buffer);
            }

            ZkosTxCommand::ExecuteTraderOrderTX(
                trader_order,
                rpc_command,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    RpcCommand::ExecuteTraderOrder(
                        order_request,
                        meta,
                        zkos_hex_string,
                        request_id,
                    ) => {
                        let zkos_settle_msg_result =
                            ZkosSettleMsg::decode_from_hex_string(zkos_hex_string);

                        match zkos_settle_msg_result {
                            Ok(zkos_settle_msg) => {
                                let contract_owner_sk = get_sk_from_fixed_wallet();

                                let contract_owner_pk = get_pk_from_fixed_wallet();

                                let lock_error =
                                    get_lock_error_for_trader_settle(trader_order.clone());

                                let mut margin_dif = (trader_order.available_margin.round()
                                    - trader_order.initial_margin.round())
                                .clone()
                                    - trader_order.unrealized_pnl.round();

                                let mut program_tag = "SettleTraderOrder".to_string();

                                if margin_dif < 0.0 {
                                    margin_dif = margin_dif * -1.0;
                                    program_tag =
                                        "SettleTraderOrderNegativeMarginDifference".to_string();
                                }

                                let margin_dif_u64 = margin_dif.round() as u64;

                                let transaction = settle_trader_order(
                                    zkos_settle_msg.output.clone(),
                                    trader_order.available_margin.clone().round() as u64,
                                    &ContractManager::import_program(&WALLET_PROGRAM_PATH.clone()),
                                    Network::Mainnet,
                                    trader_order.fee_settled.round() as u64,
                                    last_state_output
                                        .as_output_data()
                                        .get_owner_address()
                                        .unwrap()
                                        .clone(),
                                    last_state_output.clone(),
                                    next_state_output.clone(),
                                    lock_error,
                                    margin_dif_u64,
                                    trader_order.settlement_price.round() as u64,
                                    contract_owner_sk,
                                    contract_owner_pk,
                                    program_tag,
                                );

                                let tx_hash_result = match transaction {
                                    Ok(tx) => transaction_queue_to_confirm_relayer_latest_state(
                                        last_state_output.clone(),
                                        tx,
                                        next_state_output.clone(),
                                    ),
                                    Err(arg) => Err(arg.to_string()),
                                };
                                let sender_clone = sender.clone();
                                match sender_clone.send(tx_hash_result.clone()) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        println!("error in sender");
                                    }
                                }

                                match tx_hash_result {
                                    Ok(tx_hash) => {
                                        Event::new(
                                            Event::TxHash(
                                                trader_order.uuid,
                                                trader_order.account_id,
                                                tx_hash.clone(),
                                                trader_order.order_type,
                                                trader_order.order_status,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id,
                                            ),
                                            format!("tx_hash_result-{:?}",tx_hash),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
                                    }
                                    Err(arg) => {
                                        Event::new(
                                            Event::TxHash(
                                                trader_order.uuid,
                                                trader_order.account_id,
                                                arg,
                                                trader_order.order_type,
                                                OrderStatus::RejectedFromChain,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id,
                                            ),
                                            String::from("tx_hash_error"),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
                                    }
                                }
                            }
                            Err(arg) => {
                                println!(
                                    "Error:ZkosTxCommand::CreateTraderOrderTX : arg:{:#?}",
                                    arg
                                );
                                Event::new(
                                    Event::TxHash(
                                        trader_order.uuid,
                                        trader_order.account_id,
                                        arg,
                                        trader_order.order_type,
                                        OrderStatus::SerializationError,
                                        ServerTime::now().epoch,
                                        None,
                                        request_id,
                                    ),
                                    String::from("tx_hash_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                    }
                    _ => {}
                });

                drop(buffer);
            }

            ZkosTxCommand::ExecuteLendOrderTX(
                lend_order,
                rpc_command,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    RpcCommand::ExecuteLendOrder(
                        order_request,
                        meta,
                        zkos_hex_string,
                        request_id,
                    ) => {
                        let zkos_settle_msg_result =
                            ZkosSettleMsg::decode_from_hex_string(zkos_hex_string);

                        match zkos_settle_msg_result {
                            Ok(zkos_settle_msg) => {
                                let contract_owner_sk = get_sk_from_fixed_wallet();
                                let contract_owner_pk = get_pk_from_fixed_wallet();

                                let lock_error = get_lock_error_for_lend_settle(lend_order.clone());

                                let transaction = create_lend_order_settlement_transaction(
                                    zkos_settle_msg.output,
                                    lend_order.new_lend_state_amount.round() as u64,
                                    &ContractManager::import_program(&WALLET_PROGRAM_PATH.clone()),
                                    Network::Mainnet,
                                    1u64,
                                    last_state_output
                                        .as_output_data()
                                        .get_owner_address()
                                        .unwrap()
                                        .clone(),
                                    last_state_output.clone(),
                                    next_state_output.clone(),
                                    lock_error,
                                    contract_owner_sk,
                                    contract_owner_pk,
                                );

                                let tx_hash_result = match transaction {
                                    Ok(tx) => transaction_queue_to_confirm_relayer_latest_state(
                                        last_state_output.clone(),
                                        tx,
                                        next_state_output.clone(),
                                    ),
                                    Err(arg) => Err(arg.to_string()),
                                };
                                let sender_clone = sender.clone();
                                match sender_clone.send(tx_hash_result.clone()) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        println!("error in sender");
                                    }
                                }

                                match tx_hash_result {
                                    Ok(tx_hash) => {
                                        Event::new(
                                            Event::TxHash(
                                                lend_order.uuid,
                                                lend_order.account_id,
                                                tx_hash.clone(),
                                                lend_order.order_type,
                                                lend_order.order_status,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id,
                                            ),
                                            format!("tx_hash_result-{:?}",tx_hash),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
                                    }
                                    Err(arg) => {
                                        Event::new(
                                            Event::TxHash(
                                                lend_order.uuid,
                                                lend_order.account_id,
                                                arg,
                                                lend_order.order_type,
                                                OrderStatus::RejectedFromChain,
                                                ServerTime::now().epoch,
                                                None,
                                                request_id,
                                            ),
                                            String::from("tx_hash_error"),
                                            CORE_EVENT_LOG.clone().to_string(),
                                        );
                                    }
                                }
                            }
                            Err(arg) => {
                                println!(
                                    "Error:ZkosTxCommand::ExecuteLendOrderTX : arg:{:#?}",
                                    arg
                                );
                                Event::new(
                                    Event::TxHash(
                                        lend_order.uuid,
                                        lend_order.account_id,
                                        arg,
                                        lend_order.order_type,
                                        OrderStatus::SerializationError,
                                        ServerTime::now().epoch,
                                        None,
                                        request_id,
                                    ),
                                    String::from("tx_hash_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                    }
                    _ => {}
                });

                drop(buffer);
            }

            ZkosTxCommand::CancelTraderOrderTX(trader_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || {
                    match rpc_command {
                        RpcCommand::CancelTraderOrder(
                            order_request,
                            meta,
                            zkos_hex_string,
                            _request_id,
                        ) => {
                            let zkos_create_order_result =
                                ZkosCreateOrder::decode_from_hex_string(zkos_hex_string);

                            match zkos_create_order_result {
                                Ok(zkos_create_order) => {
                                    let transaction = create_trade_order(
                                        zkos_create_order.input,
                                        zkos_create_order.output,
                                        zkos_create_order.signature,
                                        zkos_create_order.proof,
                                        &ContractManager::import_program(
                                            &WALLET_PROGRAM_PATH.clone()
                                        ),
                                        Network::Mainnet,
                                        5u64
                                    );
                                    let tx_hash_result = match transaction {
                                        Ok(tx) => {
                                            twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(
                                                tx
                                            )
                                        }
                                        Err(arg) => { Err(arg.to_string()) }
                                    };

                                    let sender_clone = sender.clone();
                                    match sender_clone.send(tx_hash_result.clone()) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            println!("error in sender");
                                        }
                                    }
                                    match tx_hash_result {
                                        Ok(tx_hash) => {
                                            let mut output_hex_storage =
                                                OUTPUT_STORAGE.lock().unwrap();
                                            let _ = output_hex_storage.remove(
                                                trader_order.uuid_to_byte(),
                                                0
                                            );
                                            drop(output_hex_storage);
                                        }
                                        Err(arg) => {}
                                    }
                                }
                                Err(arg) => {
                                    println!(
                                        "Error:ZkosTxCommand::CancelTraderOrderTX : arg:{:#?}",
                                        arg
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                });
                drop(buffer);
            }

            ZkosTxCommand::CreateTraderOrderLIMITTX(trader_order, zkos_string) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || {
                    match trader_order.order_status {
                        OrderStatus::FILLED => {
                            match zkos_string {
                                Some(zkos_create_order_result) => {
                                    // let zkos_create_order_result =
                                    //     ZkosCreateOrder::decode_from_hex_string(
                                    //         zkos_create_order_result
                                    //     );
                                        let tx_result: Result<Transaction, String> = match
                                        hex::decode(zkos_create_order_result)
                                    {
                                        Ok(bytes) =>
                                            match bincode::deserialize(&bytes) {
                                                Ok(tx) => Ok(tx),
                                                Err(arg) => Err(arg.to_string()),
                                            }
                                        Err(arg) => Err(arg.to_string()),
                                    };
                                    // create transaction
                                    match tx_result {
                                        Ok(tx) => {
                                            let updated_tx_result = update_memo_tx_client_order(
                                                &tx,
                                                trader_order.entryprice.round() as u64,
                                                trader_order.positionsize.round() as u64,
                                                trader_order.fee_filled.round() as u64
                                            );
                                            match updated_tx_result {
                                                Ok(tx_and_outputmemo) => {
                                                    let output = tx_and_outputmemo.get_output();
                                                    let output_memo_hex = match
                                                        bincode::serialize(&output.clone())
                                                    {
                                                        Ok(output_memo_bin) => {
                                                            Some(hex::encode(&output_memo_bin))
                                                        }
                                                        Err(_) => None,
                                                    };
                                                    let transaction = tx_and_outputmemo.get_tx();
                                                    let tx_hash_result =
                                                        twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(
                                                            transaction
                                                        );

                                                    let sender_clone = sender.clone();
                                                    match sender_clone.send(tx_hash_result.clone()) {
                                                        Ok(_) => {}
                                                        Err(_) => {
                                                            println!("error in sender");
                                                        }
                                                    }
                                                    match tx_hash_result {
                                                        Ok(tx_hash) => {
                                                            let mut output_hex_storage =
                                                                OUTPUT_STORAGE.lock().unwrap();
                                                            let _ = output_hex_storage.add(
                                                                trader_order.uuid_to_byte(),
                                                                Some(output),
                                                                0
                                                            );
                                                            drop(output_hex_storage);
                                                            Event::new(
                                                                Event::TxHashUpdate(
                                                                    trader_order.uuid,
                                                                    trader_order.account_id,
                                                                    tx_hash.clone(),
                                                                    trader_order.order_type,
                                                                    trader_order.order_status,
                                                                    ServerTime::now().epoch,
                                                                    output_memo_hex,
                                                                ),
                                                                format!("tx_hash_result-{:?}",tx_hash),
                                                                CORE_EVENT_LOG.clone().to_string()
                                                            );
                                                        }
                                                        Err(arg) => {
                                                            Event::new(
                                                                Event::TxHashUpdate(
                                                                    trader_order.uuid,
                                                                    trader_order.account_id,
                                                                    arg,
                                                                    trader_order.order_type,
                                                                    OrderStatus::RejectedFromChain,
                                                                    ServerTime::now().epoch,
                                                                    None
                                                                ),
                                                                String::from("tx_hash_error"),
                                                                CORE_EVENT_LOG.clone().to_string()
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(arg) => {
                                                    Event::new(
                                                        Event::TxHashUpdate(
                                                            trader_order.uuid,
                                                            trader_order.account_id,
                                                            arg.to_string(),
                                                            trader_order.order_type,
                                                            OrderStatus::UtxoError,
                                                            ServerTime::now().epoch,
                                                            None
                                                        ),
                                                        String::from("tx_hash_error"),
                                                        CORE_EVENT_LOG.clone().to_string()
                                                    );
                                                }
                                            }
                                        }
                                        Err(arg) => {
                                            println!(
                                                "Error:ZkosTxCommand::CreateTraderOrderLIMITTX : arg:{:#?}",
                                                arg
                                            );
                                            Event::new(
                                                Event::TxHashUpdate(
                                                    trader_order.uuid,
                                                    trader_order.account_id,
                                                    arg,
                                                    trader_order.order_type,
                                                    OrderStatus::SerializationError,
                                                    ServerTime::now().epoch,
                                                    None
                                                ),
                                                String::from("tx_hash_error"),
                                                CORE_EVENT_LOG.clone().to_string()
                                            );
                                        }
                                    }
                                }
                                // chain tx link for multiple order in one block
                                // batch and seperate orde rby if in trader settle
                                None => {
                                    println!("Zkos Order string not found");
                                    let sender_clone = sender.clone();
                                    let fn_response_tx_hash = Err(
                                        "Zkos Order String Not Found".to_string()
                                    );
                                    sender_clone.send(fn_response_tx_hash.clone()).unwrap();
                                }
                            }
                        }
                        _ => {}
                    }
                });
                drop(buffer);
            }

            ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX(
                trader_order,
                zkos_string,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || {
                    match zkos_string {
                        Some(zkos_settle_msg_result) => {
                            let zkos_settle_msg_result =
                            ZkosSettleMsg::decode_from_hex_string(zkos_settle_msg_result);
                            match zkos_settle_msg_result {
                                Ok(zkos_settle_msg) => {
                                    let contract_owner_sk = get_sk_from_fixed_wallet();

                                    let contract_owner_pk = get_pk_from_fixed_wallet();

                                    let lock_error = get_lock_error_for_trader_settle(
                                        trader_order.clone()
                                    );

                                    let mut margin_dif =
                                        (
                                            trader_order.available_margin.round() -
                                            trader_order.initial_margin.round()
                                        ).clone() - trader_order.unrealized_pnl.round();

                                    let mut program_tag = "SettleTraderOrder".to_string();

                                    if margin_dif < 0.0 {
                                        margin_dif = margin_dif * -1.0;
                                        program_tag =
                                            "SettleTraderOrderNegativeMarginDifference".to_string();
                                    }
                                    let margin_dif_u64 = margin_dif.round() as u64;

                                    let transaction = settle_trader_order(
                                        zkos_settle_msg.output.clone(),
                                        trader_order.available_margin.clone().round() as u64,
                                        &ContractManager::import_program(
                                            &WALLET_PROGRAM_PATH.clone()
                                        ),
                                        Network::Mainnet,
                                        trader_order.fee_settled.round() as u64,
                                        last_state_output
                                            .as_output_data()
                                            .get_owner_address()
                                            .unwrap()
                                            .clone(),
                                        last_state_output.clone(),
                                        next_state_output.clone(),
                                        lock_error,
                                        margin_dif_u64,
                                        trader_order.settlement_price.round() as u64,
                                        contract_owner_sk,
                                        contract_owner_pk,
                                        program_tag
                                    );

                                    let tx_hash_result = match transaction {
                                        Ok(tx) => {
                                            transaction_queue_to_confirm_relayer_latest_state(last_state_output.clone(),
                                                tx,next_state_output.clone()
                                            )
                                        }
                                        Err(arg) => { Err(arg.to_string()) }
                                    };
                                    let sender_clone = sender.clone();
                                    match sender_clone.send(tx_hash_result.clone()) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            println!("error in sender");
                                        }
                                    }

                                    match tx_hash_result {
                                        Ok(tx_hash) => {
                                            let mut tx_hash_storage =
                                                OUTPUT_STORAGE.lock().unwrap();
                                            let _ = tx_hash_storage.remove(
                                                trader_order.uuid_to_byte(),
                                                0
                                            );
                                            drop(tx_hash_storage);
                                            Event::new(
                                                Event::TxHashUpdate(
                                                    trader_order.uuid,
                                                    trader_order.account_id,
                                                    tx_hash.clone(),
                                                    trader_order.order_type,
                                                    trader_order.order_status,
                                                    ServerTime::now().epoch,
                                                    None
                                                ),
                                                format!("tx_hash_result-{:?}",tx_hash),
                                                CORE_EVENT_LOG.clone().to_string()
                                            );
                                        }
                                        Err(arg) => {
                                            Event::new(
                                                Event::TxHashUpdate(
                                                    trader_order.uuid,
                                                    trader_order.account_id,
                                                    format!(
                                                        "Error : {:?}, need to place new limit/market settle request",
                                                        arg
                                                    ),
                                                    trader_order.order_type,
                                                    OrderStatus::RejectedFromChain,
                                                    ServerTime::now().epoch,
                                                    None
                                                ),
                                                String::from("tx_hash_error"),
                                                CORE_EVENT_LOG.clone().to_string()
                                            );
                                        }
                                    }
                                }
                                Err(arg) => {
                                    println!(
                                        "Error:ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX : arg:{:#?}",
                                        arg
                                    );
                                    Event::new(
                                        Event::TxHashUpdate(
                                            trader_order.uuid,
                                            trader_order.account_id,
                                            format!(
                                                "Error : {:?}, need to place new limit/market settle request",
                                                arg
                                            ),
                                            trader_order.order_type,
                                            OrderStatus::SerializationError,
                                            ServerTime::now().epoch,
                                            None
                                        ),
                                        String::from("tx_hash_error"),
                                        CORE_EVENT_LOG.clone().to_string()
                                    );
                                }
                            }
                        }
                        None => {
                            println!("Zkos Order string not found");
                            let sender_clone = sender.clone();
                            let fn_response_tx_hash = Err(
                                "Zkos Order String Not Found".to_string()
                            );
                            sender_clone.send(fn_response_tx_hash.clone()).unwrap();
                        }
                    }
                });
                drop(buffer);
            }

            ZkosTxCommand::RelayerCommandTraderOrderLiquidateTX(
                trader_order,
                output_option,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || match output_option {
                    Some(output_memo) => {
                        let contract_owner_sk = get_sk_from_fixed_wallet();

                        let contract_owner_pk = get_pk_from_fixed_wallet();

                        let program_tag = "LiquidateOrder".to_string();

                        let transaction = settle_trader_order(
                            output_memo.clone(),
                            trader_order.available_margin.clone().round() as u64,
                            &ContractManager::import_program(&WALLET_PROGRAM_PATH.clone()),
                            Network::Mainnet,
                            1u64,
                            last_state_output
                                .as_output_data()
                                .get_owner_address()
                                .unwrap()
                                .clone(),
                            last_state_output.clone(),
                            next_state_output.clone(),
                            0,
                            0,
                            trader_order.settlement_price.round() as u64,
                            contract_owner_sk,
                            contract_owner_pk,
                            program_tag,
                        );
                        let tx_hash_result = match transaction {
                            Ok(tx) => transaction_queue_to_confirm_relayer_latest_state(
                                last_state_output.clone(),
                                tx,
                                next_state_output.clone(),
                            ),
                            Err(arg) => Err(arg.to_string()),
                        };
                        let sender_clone = sender.clone();

                        match sender_clone.send(tx_hash_result.clone()) {
                            Ok(_) => {}
                            Err(_) => {
                                println!("error in sender");
                            }
                        }
                        match tx_hash_result {
                            Ok(tx_hash) => {
                                let mut tx_hash_storage = OUTPUT_STORAGE.lock().unwrap();
                                let _ = tx_hash_storage.remove(trader_order.uuid_to_byte(), 0);
                                drop(tx_hash_storage);
                                Event::new(
                                    Event::TxHashUpdate(
                                        trader_order.uuid,
                                        trader_order.account_id,
                                        tx_hash.clone(),
                                        trader_order.order_type,
                                        trader_order.order_status,
                                        ServerTime::now().epoch,
                                        None,
                                    ),
                                    format!("tx_hash_result-{:?}",tx_hash),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                            Err(arg) => {
                                Event::new(
                                    Event::TxHashUpdate(
                                        trader_order.uuid,
                                        trader_order.account_id,
                                        format!("Error : {:?}, liquidation failed", arg),
                                        trader_order.order_type,
                                        OrderStatus::RejectedFromChain,
                                        ServerTime::now().epoch,
                                        None,
                                    ),
                                    String::from("tx_hash_error"),
                                    CORE_EVENT_LOG.clone().to_string(),
                                );
                            }
                        }
                    }
                    None => {
                        println!("Output memo not Found : {:?}",trader_order.uuid);
                        let sender_clone = sender.clone();
                        let fn_response_tx_hash = Err("Output memo not Found".to_string());
                        sender_clone.send(fn_response_tx_hash.clone()).unwrap();
                        Event::new(
                            Event::TxHashUpdate(
                                trader_order.uuid,
                                trader_order.account_id,
                                "Error : Output memo not Found, liquidation failed".to_string(),
                                trader_order.order_type,
                                OrderStatus::UtxoError,
                                ServerTime::now().epoch,
                                None,
                            ),
                            String::from("tx_hash_error"),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                    }
                });

                drop(buffer);
            }
        }
    } else {
        let fn_response_tx_hash = Ok("ZKOS CHAIN TRANSACTION IS NOT ACTIVE".to_string());
        sender.send(fn_response_tx_hash).unwrap();
    }
    // return Arc::clone(&receiver_mutex);
}

pub fn transaction_queue_to_confirm_relayer_latest_state(
    last_output: Output,
    tx: Transaction,
    next_output: Output,
) -> Result<String, String> {

    let nonce = match last_output.as_out_state() {
        Some(state) => state.nonce.clone(),
        None => 1,
    };

    let account_id = last_output
        .as_output_data()
        .get_owner_address()
        .clone()
        .unwrap()
        .clone();

    let mut flag_chain_update = true;
    let mut latest_nonce: u32 = 0;
    let mut chain_attempt: i32 = 0;
    while flag_chain_update {
        // let updated_output_on_chain =
        match twilight_relayer_sdk::twilight_client_sdk::chain::get_utxo_details_by_address(
            account_id.clone(),
            IOType::State,
        ) {
            Ok(utxo_detail) => {
                match utxo_detail.output.as_out_state() {
                    Some(state) => {
                        latest_nonce = state.nonce.clone();
                        // flag_chain_update = false;
                    }
                    None => {}
                }
            }
            Err(arg) => {
                chain_attempt += 1;
                sleep(Duration::from_secs(2));
                if chain_attempt == 50 {
                    // flag_chain_update = false;
                    return Err("Tx Failed due to missing latest state update".to_string());
                }
            }
        }
        if nonce == latest_nonce {
            // flag_chain_update = false;
            println!("tx: {:?}", tx);
            match twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(tx) {
                Ok(tx_hash) => {
                    Event::new(
                        Event::AdvanceStateQueue((nonce + 1) as usize, next_output),
                        format!("Nonce : {:?}", nonce + 1),
                        RELAYER_STATE_QUEUE.clone().to_string(),
                    );
                    sleep(Duration::from_secs(5));
                    return Ok(tx_hash);
                }
                Err(arg) => {
                    return Err(arg);
                }
            }
        } else {
            flag_chain_update = true;
            chain_attempt += 1;
            sleep(Duration::from_secs(2));
            if chain_attempt == 50 {
                // flag_chain_update = false;
                return Err("Tx Failed due to missing latest state update".to_string());
            }
        }
    }
    return Err("Tx Failed due to missing latest state update".to_string());
}

#[cfg(test)]
mod tests {
    use twilight_relayer_sdk::twilight_client_sdk::transaction;

    #[test]
    fn test_tx_commit_broadcast() {

        // use std::env;
        dotenv::dotenv().ok();
        // let zkos_server_url = env::var("ZKOS_SERVER_URL").expect("ZKOS_SERVER_URL must be set");

        let tx_hex = "010000000100000000000000000000001800000000000000000000000000000001010101000000000000000000000000000000f6a5e681e48137f0834d95ca0b098eb9c1d362c4afc393200d93dc2e5266ccbe006a25d2b02197a68969629d43005e28c0fc213472efb0b8f929534c429ade860a80da17a6d5e50fd4989ab9b4e9310613c70e623f361a4eb0a8509e417038cb218a0000000000000030633032333165303531643231336239626335373631643765336662383663613436376264383937663866393232336536613336383337356539343664313036373563363464333332363966643662336565386539303765646430313732366363663233663630383066303338366635393365396434303366336230643465613763633062656162643700010000000000000001000000010000002a000000000000003138323237323664346265336336623333623166333434633734333263626530343230333861663162388a000000000000003063303233316530353164323133623962633537363164376533666238366361343637626438393766386639323233653661333638333735653934366431303637356336346433333236396664366233656538653930376564643031373236636366323366363038306630333836663539336539643430336633623064346561376363306265616264370000000016eacb0d167fd36cfbf76e26357fb49aee9bac76ecf08957be7cd7c3c1e2204801040000000000000003000000010000006065248d0100000000000000000000000000000000000000000000000000000002000000000000005453d47db383cebcc81d4a355ab3cee0d6aae97fbd2960291c0e0a8788fcb03c0300000001000000c9b10100000000000000000000000000000000000000000000000000000000000300000001000000ecd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010010000001600000000000000060a0403000000060a0405000000060a0d0e1302020200000000010000000000000003000000000000003d107ff89a1f0387ca226522f82bca1fbf43059a200c49bdae998400bfe89ea52c92d3c5e02c98dc6b6f1f91cfd42e136d1d4f1de6bdb351b12be20f9535960c7c1b7c42e22f98283d98314b914cdbfc380ef2bb1c3e3a91685a05614fc4cf6da10100000000000000721c0ca0d6119987df3a91d9bc1396e7ccbec1e87e26ebb6ef1d40d83d74c95dde9a975689eb20c6b3a3b5e22b5442c0a90dbd02aed4197645cb2c9d50d9b5409649551fecbbfb0468e5324761bfc3672bb3b33a551533c2f6408e1128402e11a272535c8e43e528e5c818e5da5802e063793f4a3f56d8b335cabbe7a284db6f40b0f8ba6105c9022e561dc8bad81fdccf2b2d78a1b9cc71ccc964851520452a5c04300f7905e318f69b8665458bbf837673dc0233a36ae34a9bb813c3c2a41c8691e1372af302ec8400bb7c68ab0847af6e3ce6dadd0918e2034e33641aa73188cb7c1255ff9f47340a4c3cf2388e1c1c173d2e395f2d2efe05512d6c4b2b31486658c7ec583abd58ec7ca9c6b4390978268202e4d724504abe8e3060c5490eb64cdd0af58503fe47812af928b1cb7ad96eddac2f82e8699738dccc5427470494fe872642696383fbcc3bfc168742ba4b5649afe00de617e030aa47058c8302d4f2a784a71e40c0cae4f128d7d0ee2aa36cb5221cf4e631e78ada031225260efc3b3e7a5e9a48d11e7e0ea6d013cbd76c425c6442afd93268e796c8227d97030100000000000000020000004000000000000000105fd4af09a1875d1ab963cb976a5f2ab8c2598f8a04e008054b224566297a3dcd76da0cbd27382141b663be233604953b2a60e4c8bfee2f351c70244069f10f0100000001000000000000002fed6454c9ecdc97138568a1cf9b0f5410d6df39d3442b36a99ca2b5ffc76b0a0100000000000000887037ade1700358bc93a5921675dddd982f75ab9cd22b9b45a2620b0b0f070f0000000000000000e9be99376af0194a05ab8ce828db16d7df2909273c6d404238e912077bf27f090102000000000000001e5442c4bf59d8355ad529687ef843fcbce3f4fbace3d4a38ba0b334d95af37d";

        let tx_bytes = hex::decode(tx_hex).expect("Failed to decode hex");
        let tx: transaction::Transaction = bincode::deserialize(&tx_bytes).expect("Failed to deserialize transaction");

        let result = twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(tx);
        println!("Transaction broadcast result: {:?}", result);
    }
}
