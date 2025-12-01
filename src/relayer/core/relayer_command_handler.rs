use crate::config::*;
use crate::db::*;
use crate::relayer::core::core_config::*;
use crate::relayer::core::zkos_handler::zkos_order_handler;
use crate::relayer::*;
use std::sync::{mpsc, Arc, RwLock};
use twilight_relayer_sdk::twilight_client_sdk::util::create_output_state_for_trade_lend_order;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use twilight_relayer_sdk::zkvm::Output;

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
                            order_mut_ref.order_status = OrderStatus::LIQUIDATE;
                            drop(order_mut_ref);
                            match order.order_status {
                                OrderStatus::FILLED | OrderStatus::LIQUIDATE => {
                                    order.order_status = OrderStatus::LIQUIDATE;
                                    //update batch process
                                    let payment = order.liquidate(current_price_clone);
                                    // println!("order:{:?}",order);
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
                                        ZkosTxCommand::RelayerCommandTraderOrderLiquidateTX(
                                            order.clone(),
                                            output_option,
                                            lendpool.last_output_state.clone(),
                                            next_output_state.clone()
                                        ),
                                        sender
                                    );

                                    match zkos_receiver.recv() {
                                        Ok(chain_message) => {
                                            match chain_message {
                                                Ok(tx_hash) => {
                                                    PositionSizeLog::remove_order(
                                                        order.position_type.clone(),
                                                        order.positionsize.clone()
                                                    );
                                                    let mut order_mut_ref = order_detail
                                                        .write()
                                                        .unwrap();
                                                    *order_mut_ref = order.clone();
                                                    drop(order_mut_ref);
                                                    // println!("locking mutex LEND_POOL_DB");

                                                    lendpool.add_transaction(
                                                        LendPoolCommand::AddTraderOrderLiquidation(
                                                            RelayerCommand::PriceTickerLiquidation(
                                                                vec![order.uuid.clone()],
                                                                metadata_clone,
                                                                current_price_clone
                                                            ),
                                                            order.clone(),
                                                            payment,
                                                            next_output_state
                                                        )
                                                    );
                                                    // println!("dropping mutex LEND_POOL_DB");
                                                }
                                                Err(verification_error) => {
                                                    crate::log_trading!(
                                                        error,
                                                        "Error in line relayercore.rs 835 : {:?}, uuid: {:?}, nonce: {:?}, \n next_output:{:?}",
                                                        verification_error,
                                                        order.uuid,
                                                        lendpool.nonce,
                                                        next_output_state
                                                    );
                                                }
                                            }
                                        }
                                        Err(arg) => {
                                            crate::log_trading!(
                                                error,
                                                "Error in line relayercore.rs 842 : {:?}",
                                                arg
                                            );
                                        }
                                    }

                                    drop(lendpool);
                                }
                                _ => {
                                    // drop(order_mut_ref);
                                    crate::log_trading!(
                                        debug,
                                        "Invalid order status !!\n, order: {:?}",
                                        order
                                    );
                                }
                            }
                        }
                        Err(arg) => {
                            crate::log_trading!(debug, "Error found:{:#?}", arg);
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
                                            ),
                                            sender
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
                                            }
                                        }
                                    });
                                }
                                _ => {
                                    crate::log_trading!(
                                        debug,
                                        "Invalid order status !!, order: {:?}",
                                        order
                                    );
                                    drop(order);
                                }
                            }
                            drop(order_read_only_ref);
                        }
                        Err(arg) => {
                            crate::log_trading!(debug, "Error found:{:#?}", arg);
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
                                        OrderType::MARKET,
                                        None,
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
                                                        .as_output_data()
                                                        .get_owner_address()
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
                                                ),
                                                sender
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
                                                            crate::log_trading!(
                                                                debug,
                                                                "Error in line relayercore.rs 1086 : {:?}",
                                                                verification_error
                                                            );
                                                            // settle limit removed from the db, need to place new limit/market settle request
                                                        }
                                                    }
                                                }
                                                Err(arg) => {
                                                    crate::log_trading!(
                                                        debug,
                                                        "Error in line relayercore.rs 1094 : {:?}",
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
                                    crate::log_trading!(
                                        debug,
                                        "Invalid order status for order_id:{} and order_status:{:#?} !!\n",
                                        order.uuid,
                                        order.order_status
                                    );
                                    drop(order);
                                }
                            }
                        }
                        Err(arg) => {
                            crate::log_trading!(debug, "Error found:{:#?}", arg);
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
        RelayerCommand::UpdateFees(
            order_filled_on_market,
            order_filled_on_limit,
            order_settled_on_market,
            order_settled_on_limit,
        ) => {
            let event_time = systemtime_to_utc();
            let mut local_storage = LOCALDB.lock().unwrap();
            let old_order_filled_on_market =
                local_storage.insert(FeeType::FilledOnMarket.into(), order_filled_on_market);
            let old_order_filled_on_limit =
                local_storage.insert(FeeType::FilledOnLimit.into(), order_filled_on_limit);
            let old_order_settled_on_market =
                local_storage.insert(FeeType::SettledOnMarket.into(), order_settled_on_market);
            let old_order_settled_on_limit =
                local_storage.insert(FeeType::SettledOnLimit.into(), order_settled_on_limit);
            drop(local_storage);
            if old_order_filled_on_market.is_some() {
                crate::log_heartbeat!(
                    info,
                    "OrderFilledOnMarket fee updated from {} to {} at {}",
                    old_order_filled_on_market.unwrap_or(0.0),
                    order_filled_on_market,
                    event_time
                );
            }
            if old_order_filled_on_limit.is_some() {
                crate::log_heartbeat!(
                    info,
                    "OrderFilledOnLimit fee updated from {} to {} at {}",
                    old_order_filled_on_limit.unwrap_or(0.0),
                    order_filled_on_limit,
                    event_time
                );
            }
            if old_order_settled_on_limit.is_some() {
                crate::log_heartbeat!(
                    info,
                    "OrderSettledOnLimit fee updated from {} to {} at {}",
                    old_order_settled_on_limit.unwrap_or(0.0),
                    order_settled_on_limit,
                    event_time
                );
            }
            if old_order_settled_on_market.is_some() {
                crate::log_heartbeat!(
                    info,
                    "OrderSettledOnMarket fee updated from {} to {} at {}",
                    old_order_settled_on_market.unwrap_or(0.0),
                    order_settled_on_market,
                    event_time
                );
            }
            crate::log_heartbeat!(info, "Fee update completed");
            Event::new(
                Event::FeeUpdate(
                    RelayerCommand::UpdateFees(
                        order_filled_on_market,
                        order_filled_on_limit,
                        order_settled_on_market,
                        order_settled_on_limit,
                    ),
                    event_time.clone(),
                ),
                format!("FeeUpdate-{}", event_time),
                CORE_EVENT_LOG.clone().to_string(),
            );
        }
    }
}
