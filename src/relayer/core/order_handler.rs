use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::zkos_handler::zkos_order_handler;
use crate::relayer::*;
use std::sync::{ mpsc, Arc };
use std::time::Duration;
use twilight_relayer_sdk::twilight_client_sdk::util::create_output_state_for_trade_lend_order;
use uuid::Uuid;
use crate::relayer::core::core_config::*;

pub fn client_cmd_receiver() {
    match
        receive_from_kafka_queue(
            RPC_CLIENT_REQUEST.clone(),
            RELAYER_CORE_GROUP_CLIENT_REQUEST.clone()
        )
    {
        Ok((rpc_cmd_receiver, tx_consumed)) => {
            let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);

            loop {
                let rpc_client_cmd_request = match rpc_cmd_receiver1.lock() {
                    Ok(rpc_client_cmd_request) => rpc_client_cmd_request,
                    Err(arg) => {
                        crate::log_heartbeat!(error, "rpc_cmd_receiver1 lock failed: {:#?}", arg);
                        std::thread::sleep(Duration::from_secs(1));
                        break;
                    }
                };

                match rpc_client_cmd_request.recv() {
                    Ok((rpc_command, offset_complition)) => {
                        rpc_event_handler(rpc_command, tx_consumed.clone(), offset_complition);
                    }
                    Err(arg) => {
                        crate::log_heartbeat!(
                            info,
                            "Relayer turn off command receiver from client-request"
                        );
                        break;
                    }
                }
            }
        }
        Err(arg) => {
            crate::log_heartbeat!(info, "error in client_cmd_receiver: {:#?}", arg);
        }
    }
}

pub fn rpc_event_handler(
    command: RpcCommand,
    tx_consumed: crossbeam_channel::Sender<OffsetCompletion>,
    offset_complition: OffsetCompletion
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
                                        command_clone_for_zkos
                                    ),
                                    sender
                                );
                                match zkos_receiver.recv() {
                                    Ok(chain_message) =>
                                        match chain_message {
                                            Ok(tx_hash) => {
                                                let order_state =
                                                    orderdata.orderinsert_localdb(status);
                                                let mut trader_order_db =
                                                    TRADER_ORDER_DB.lock().unwrap();
                                                let completed_order = trader_order_db.add(
                                                    orderdata_clone,
                                                    command_clone
                                                );
                                                drop(trader_order_db);
                                            }
                                            Err(verification_error) => {
                                                let mut trader_order_db =
                                                    TRADER_ORDER_DB.lock().unwrap();
                                                let _ = trader_order_db.remove_order_check(
                                                    orderdata_clone.account_id.clone()
                                                );
                                                drop(trader_order_db);
                                            }
                                        }
                                    Err(arg) => {
                                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                        let _ = trader_order_db.remove_order_check(
                                            orderdata_clone.account_id.clone()
                                        );
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
                            let completed_order = trader_order_db.add(
                                orderdata_clone,
                                command_clone
                            );
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
                                    request_id.clone()
                                ),
                                format!("tx_limit_submit-{:?}", request_id),
                                CORE_EVENT_LOG.clone().to_string()
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
                                "Duplicate order found in the database with same public key".to_string(),
                                orderdata.order_type,
                                OrderStatus::DuplicateOrder,
                                ServerTime::now().epoch,
                                None,
                                request_id
                            ),
                            String::from("tx_duplicate_error"),
                            CORE_EVENT_LOG.clone().to_string()
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
                            request_id
                        ),
                        String::from("tx_wrong_parameter_error"),
                        CORE_EVENT_LOG.clone().to_string()
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
                                let (payment, order_status) =
                                    order_updated_clone.check_for_settlement(
                                        execution_price,
                                        current_price,
                                        rpc_request.order_type
                                    );
                                match order_status {
                                    OrderStatus::SETTLED => {
                                        let order_clone = order_updated_clone.clone();

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
                                            ZkosTxCommand::ExecuteTraderOrderTX(
                                                order_clone.clone(),
                                                command_clone.clone(),
                                                lendpool.last_output_state.clone(),
                                                next_output_state.clone()
                                            ),
                                            sender
                                        );

                                        match zkos_receiver.recv() {
                                            Ok(chain_message) =>
                                                match chain_message {
                                                    Ok(tx_hash) => {
                                                        *order = order_clone.clone();
                                                        order_updated_clone.order_remove_from_localdb();
                                                        drop(order);
                                                        lendpool.add_transaction(
                                                            LendPoolCommand::AddTraderOrderSettlement(
                                                                command_clone,
                                                                order_clone.clone(),
                                                                payment,
                                                                next_output_state
                                                            )
                                                        );
                                                    }
                                                    Err(verification_error) => {}
                                                }
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
                                            zkos_hex_string
                                        );
                                        Event::new(
                                            Event::TraderOrderLimitUpdate(
                                                order_updated_clone.clone(),
                                                command_clone.clone(),
                                                trader_order_db.aggrigate_log_sequence
                                            ),
                                            format!(
                                                "settle_limit_order-{}",
                                                order_updated_clone.uuid
                                            ),
                                            CORE_EVENT_LOG.clone().to_string()
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
                                                request_id.clone()
                                            ),
                                            format!("tx_settle_limit_submit-{:?}", request_id),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }
                                    _ => {
                                        drop(order);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                crate::log_trading!(
                                    debug,
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
                                        request_id
                                    ),
                                    String::from("trader_tx_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string()
                                );
                            }
                        }
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    Err(arg) => {
                        crate::log_trading!(
                            debug,
                            "Error found for order_id {}:{:#?}",
                            rpc_request.uuid,
                            arg
                        );
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
                                request_id
                            ),
                            String::from("trader_tx_not_found_error"),
                            CORE_EVENT_LOG.clone().to_string()
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
                    let mut lendorder: LendOrder = LendOrder::new_order(
                        rpc_request.clone(),
                        tlv0,
                        tps0
                    );
                    lendorder.order_status = OrderStatus::FILLED;
                    // let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, 20048615383.0, 2000000.0);

                    let next_output_state = create_output_state_for_trade_lend_order(
                        (lend_pool.nonce + 1) as u32,
                        lend_pool.last_output_state
                            .clone()
                            .as_output_data()
                            .get_script_address()
                            .unwrap()
                            .clone(),
                        lend_pool.last_output_state
                            .clone()
                            .as_output_data()
                            .get_owner_address()
                            .clone()
                            .unwrap()
                            .clone(),
                        lendorder.tlv1 as u64,
                        lendorder.tps1 as u64,
                        0
                    );

                    let (sender, zkos_receiver) = mpsc::channel();
                    zkos_order_handler(
                        ZkosTxCommand::CreateLendOrderTX(
                            lendorder.clone(),
                            command_clone.clone(),
                            lend_pool.last_output_state.clone(),
                            next_output_state.clone()
                        ),
                        sender
                    );

                    match zkos_receiver.recv() {
                        Ok(chain_message) =>
                            match chain_message {
                                Ok(tx_hash) => {
                                    lend_pool.add_transaction(
                                        LendPoolCommand::LendOrderCreateOrder(
                                            command_clone,
                                            lendorder.clone(),
                                            lendorder.deposit,
                                            next_output_state
                                        )
                                    );
                                }
                                Err(verification_error) => {
                                    crate::log_trading!(
                                        error,
                                        "Error in line relayercore.rs 443 for account_id:{} : {:?}",
                                        rpc_request.account_id,
                                        verification_error
                                    );
                                }
                            }
                        Err(arg) => {
                            crate::log_trading!(
                                error,
                                "Error in line relayercore.rs 449 for account_id:{} : {:?}",
                                rpc_request.account_id,
                                arg
                            );
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
                            "Duplicate order found in the database with same public key".to_string(),
                            OrderType::LEND,
                            OrderStatus::DuplicateOrder,
                            ServerTime::now().epoch,
                            None,
                            request_id
                        ),
                        String::from("tx_duplicate_error"),
                        CORE_EVENT_LOG.clone().to_string()
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
                                                lend_pool.last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_script_address()
                                                    .unwrap()
                                                    .clone(),
                                                lend_pool.last_output_state
                                                    .clone()
                                                    .as_output_data()
                                                    .get_owner_address()
                                                    .clone()
                                                    .unwrap()
                                                    .clone(),
                                                order_clone.tlv3.round() as u64,
                                                order_clone.tps3.round() as u64,
                                                0
                                            );

                                        let (sender, zkos_receiver) = mpsc::channel();
                                        zkos_order_handler(
                                            ZkosTxCommand::ExecuteLendOrderTX(
                                                order_clone.clone(),
                                                command_clone.clone(),
                                                lend_pool.last_output_state.clone(),
                                                next_output_state.clone()
                                            ),
                                            sender
                                        );

                                        match zkos_receiver.recv() {
                                            Ok(chain_message) =>
                                                match chain_message {
                                                    Ok(tx_hash) => {
                                                        *order = order_clone.clone();
                                                        drop(order);
                                                        lend_pool.add_transaction(
                                                            LendPoolCommand::LendOrderSettleOrder(
                                                                command_clone,
                                                                order_clone.clone(),
                                                                order_clone.nwithdraw,
                                                                next_output_state
                                                            )
                                                        );
                                                    }
                                                    Err(verification_error) => {
                                                        crate::log_trading!(
                                                            error,
                                                            "Error in line relayercore.rs 552 for order_id:{} : {:?}",
                                                            rpc_request.uuid,
                                                            verification_error
                                                        );
                                                    }
                                                }
                                            Err(arg) => {
                                                crate::log_trading!(
                                                    error,
                                                    "Error in line relayercore.rs 559 for order_id:{} : {:?}",
                                                    rpc_request.uuid,
                                                    arg
                                                );
                                            }
                                        }
                                        drop(lend_pool);
                                    }
                                    Err(arg) => {
                                        crate::log_trading!(
                                            error,
                                            "Error found for lend pool info :{:#?}, Error:{:#?}",
                                            lend_pool.clone(),
                                            arg
                                        );
                                        drop(order);
                                        drop(lend_pool);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                drop(lend_pool);
                                crate::log_trading!(
                                    debug,
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
                                        request_id
                                    ),
                                    String::from("lend_tx_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string()
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
                        crate::log_trading!(
                            debug,
                            "Error found for order_id:{:#?}, Error:{:#?}",
                            rpc_request.uuid,
                            arg
                        );
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
                                request_id
                            ),
                            String::from("lend_tx_not_found_error"),
                            CORE_EVENT_LOG.clone().to_string()
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
                                        let cancelled_order = trader_order_db.remove(
                                            order_clone,
                                            command_clone
                                        );
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
                                                request_id
                                            ),
                                            format!(
                                                "tx_hash_result-{:?}",
                                                "Order successfully cancelled !!".to_string()
                                            ),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }
                                    _ => {
                                        drop(order);
                                    }
                                }
                            }
                            _ => {
                                drop(order);
                                crate::log_trading!(
                                    debug,
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
                                        request_id
                                    ),
                                    String::from("trader_order_not_found_error"),
                                    CORE_EVENT_LOG.clone().to_string()
                                );
                            }
                        }
                        match tx_consumed.send(offset_complition) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    Err(arg) => {
                        crate::log_trading!(
                            debug,
                            "Error found for order_id:{:#?}, Error:{:#?}",
                            rpc_request.uuid,
                            arg
                        );
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

#[cfg(test)]
mod tests {
    use twilight_relayer_sdk::twilight_client_sdk::transaction;

    #[test]
    fn test_tx_commit_broadcast() {
        // use std::env;
        dotenv::dotenv().ok();
        // let zkos_server_url = env::var("ZKOS_SERVER_URL").expect("ZKOS_SERVER_URL must be set");

        let tx_hex =
            "010000000100000000000000000000001800000000000000000000000000000001010101000000000000000000000000000000f6a5e681e48137f0834d95ca0b098eb9c1d362c4afc393200d93dc2e5266ccbe006a25d2b02197a68969629d43005e28c0fc213472efb0b8f929534c429ade860a80da17a6d5e50fd4989ab9b4e9310613c70e623f361a4eb0a8509e417038cb218a0000000000000030633032333165303531643231336239626335373631643765336662383663613436376264383937663866393232336536613336383337356539343664313036373563363464333332363966643662336565386539303765646430313732366363663233663630383066303338366635393365396434303366336230643465613763633062656162643700010000000000000001000000010000002a000000000000003138323237323664346265336336623333623166333434633734333263626530343230333861663162388a000000000000003063303233316530353164323133623962633537363164376533666238366361343637626438393766386639323233653661333638333735653934366431303637356336346433333236396664366233656538653930376564643031373236636366323366363038306630333836663539336539643430336633623064346561376363306265616264370000000016eacb0d167fd36cfbf76e26357fb49aee9bac76ecf08957be7cd7c3c1e2204801040000000000000003000000010000006065248d0100000000000000000000000000000000000000000000000000000002000000000000005453d47db383cebcc81d4a355ab3cee0d6aae97fbd2960291c0e0a8788fcb03c0300000001000000c9b10100000000000000000000000000000000000000000000000000000000000300000001000000ecd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010010000001600000000000000060a0403000000060a0405000000060a0d0e1302020200000000010000000000000003000000000000003d107ff89a1f0387ca226522f82bca1fbf43059a200c49bdae998400bfe89ea52c92d3c5e02c98dc6b6f1f91cfd42e136d1d4f1de6bdb351b12be20f9535960c7c1b7c42e22f98283d98314b914cdbfc380ef2bb1c3e3a91685a05614fc4cf6da10100000000000000721c0ca0d6119987df3a91d9bc1396e7ccbec1e87e26ebb6ef1d40d83d74c95dde9a975689eb20c6b3a3b5e22b5442c0a90dbd02aed4197645cb2c9d50d9b5409649551fecbbfb0468e5324761bfc3672bb3b33a551533c2f6408e1128402e11a272535c8e43e528e5c818e5da5802e063793f4a3f56d8b335cabbe7a284db6f40b0f8ba6105c9022e561dc8bad81fdccf2b2d78a1b9cc71ccc964851520452a5c04300f7905e318f69b8665458bbf837673dc0233a36ae34a9bb813c3c2a41c8691e1372af302ec8400bb7c68ab0847af6e3ce6dadd0918e2034e33641aa73188cb7c1255ff9f47340a4c3cf2388e1c1c173d2e395f2d2efe05512d6c4b2b31486658c7ec583abd58ec7ca9c6b4390978268202e4d724504abe8e3060c5490eb64cdd0af58503fe47812af928b1cb7ad96eddac2f82e8699738dccc5427470494fe872642696383fbcc3bfc168742ba4b5649afe00de617e030aa47058c8302d4f2a784a71e40c0cae4f128d7d0ee2aa36cb5221cf4e631e78ada031225260efc3b3e7a5e9a48d11e7e0ea6d013cbd76c425c6442afd93268e796c8227d97030100000000000000020000004000000000000000105fd4af09a1875d1ab963cb976a5f2ab8c2598f8a04e008054b224566297a3dcd76da0cbd27382141b663be233604953b2a60e4c8bfee2f351c70244069f10f0100000001000000000000002fed6454c9ecdc97138568a1cf9b0f5410d6df39d3442b36a99ca2b5ffc76b0a0100000000000000887037ade1700358bc93a5921675dddd982f75ab9cd22b9b45a2620b0b0f070f0000000000000000e9be99376af0194a05ab8ce828db16d7df2909273c6d404238e912077bf27f090102000000000000001e5442c4bf59d8355ad529687ef843fcbce3f4fbace3d4a38ba0b334d95af37d";

        let tx_bytes = hex::decode(tx_hex).expect("Failed to decode hex");
        let tx: transaction::Transaction = bincode
            ::deserialize(&tx_bytes)
            .expect("Failed to deserialize transaction");
        println!("Transaction result: {:?}", tx.verify());
        // let result =
        //     twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(tx);
        // println!("Transaction broadcast result: {:?}", result);
    }
}
