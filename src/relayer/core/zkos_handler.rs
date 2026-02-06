use crate::config::*;
use crate::db::*;
use crate::relayer::core::core_config::*;
use crate::relayer::core::state_tx::*;
use crate::relayer::*;
use std::sync::mpsc;
use twilight_relayer_sdk::address::Network;
use twilight_relayer_sdk::lend::*;
use twilight_relayer_sdk::order::*;
use twilight_relayer_sdk::transaction::Transaction;
use twilight_relayer_sdk::twilight_client_sdk::programcontroller::ContractManager;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;

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
                                                            crate::log_trading!(
                                                                error,
                                                                "error in sender at line 1231"
                                                            );
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
                                                                format!(
                                                                    "tx_hash_result-{:?}",
                                                                    tx_hash
                                                                ),
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
                                            crate::log_trading!(
                                                error,
                                                "Error:ZkosTxCommand::CreateTraderOrderTX : arg:{:#?} for account_id:{}",
                                                arg,
                                                order_request.account_id
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
                                                    transaction_queue_to_confirm_relayer_latest_state(
                                                        last_state_output.clone(),
                                                        tx,
                                                        next_state_output.clone()
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
                                                        format!("tx_hash_result-{:?}", tx_hash),
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
                                    crate::log_trading!(
                                        error,
                                        "Error:ZkosTxCommand::CreateLendOrderTX : arg:{:#?} for account_id:{}",
                                        arg,
                                        lend_order.account_id
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
                buffer.execute(move || {
                    match rpc_command {
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
                                    let is_state_updated = is_state_updated(&last_state_output);
                                    if !is_state_updated {
                                        let sender_clone = sender.clone();
                                        match
                                            sender_clone.send(
                                                Err(
                                                    "Tx Failed due to missing latest state update".to_string()
                                                )
                                            )
                                        {
                                            Ok(_) => {
                                                crate::log_trading!(
                                                    error,
                                                    "Tx Failed due to missing latest state update"
                                                );
                                            }
                                            Err(_) => {
                                                crate::log_trading!(
                                                    error,
                                                    "Tx Failed due to missing latest state update"
                                                );
                                            }
                                        }
                                        return;
                                    }
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
                                        Ok(tx) =>
                                            transaction_queue_to_confirm_relayer_latest_state(
                                                last_state_output.clone(),
                                                tx,
                                                next_state_output.clone()
                                            ),
                                        Err(arg) => Err(arg.to_string()),
                                    };
                                    let sender_clone = sender.clone();
                                    match sender_clone.send(tx_hash_result.clone()) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            crate::log_trading!(
                                                error,
                                                "error in sender at line 1564"
                                            );
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
                                                    request_id
                                                ),
                                                format!("tx_hash_result-{:?}", tx_hash),
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
                                    crate::log_trading!(
                                        error,
                                        "Error:ZkosTxCommand::ExecuteTraderOrderTX : arg:{:#?} for order_id:{}",
                                        arg,
                                        order_request.uuid
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

            ZkosTxCommand::ExecuteLendOrderTX(
                lend_order,
                rpc_command,
                last_state_output,
                next_state_output,
            ) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || {
                    match rpc_command {
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

                                    let lock_error = get_lock_error_for_lend_settle(
                                        lend_order.clone()
                                    );

                                    let transaction = create_lend_order_settlement_transaction(
                                        zkos_settle_msg.output,
                                        lend_order.new_lend_state_amount.round() as u64,
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
                                        last_state_output.clone(),
                                        next_state_output.clone(),
                                        lock_error,
                                        contract_owner_sk,
                                        contract_owner_pk
                                    );

                                    let tx_hash_result = match transaction {
                                        Ok(tx) =>
                                            transaction_queue_to_confirm_relayer_latest_state(
                                                last_state_output.clone(),
                                                tx,
                                                next_state_output.clone()
                                            ),
                                        Err(arg) => Err(arg.to_string()),
                                    };
                                    let sender_clone = sender.clone();
                                    match sender_clone.send(tx_hash_result.clone()) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            crate::log_trading!(
                                                error,
                                                "error in sender at line 1684"
                                            );
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
                                                    request_id
                                                ),
                                                format!("tx_hash_result-{:?}", tx_hash),
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
                                    crate::log_trading!(
                                        error,
                                        "Error:ZkosTxCommand::ExecuteLendOrderTX : arg:{:#?} for order_id:{}",
                                        arg,
                                        order_request.uuid
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
                                            crate::log_trading!(
                                                error,
                                                "error in sender at line 1789"
                                            );
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
                                    crate::log_trading!(
                                        error,
                                        "Error:ZkosTxCommand::CancelTraderOrderTX : arg:{:#?} for order_id:{}",
                                        arg,
                                        order_request.uuid
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
                                                            crate::log_trading!(
                                                                error,
                                                                "error in sender at line 1866"
                                                            );
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
                                                                    output_memo_hex
                                                                ),
                                                                format!(
                                                                    "tx_hash_result-{:?}",
                                                                    tx_hash
                                                                ),
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
                                            crate::log_trading!(
                                                error,
                                                "Error:ZkosTxCommand::CreateTraderOrderLIMITTX : arg:{:#?} for order_id:{}",
                                                arg,
                                                trader_order.uuid
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
                                    crate::log_trading!(
                                        error,
                                        "Zkos Order string not found for order_id:{}",
                                        trader_order.uuid
                                    );
                                    let sender_clone = sender.clone();
                                    let fn_response_tx_hash = Err(
                                        format!(
                                            "Zkos Order String Not Found for order_id:{}",
                                            trader_order.uuid
                                        )
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
                                    let is_state_updated = is_state_updated(&last_state_output);
                                    if !is_state_updated {
                                        let sender_clone = sender.clone();
                                        match
                                            sender_clone.send(
                                                Err(
                                                    "Tx Failed due to missing latest state update".to_string()
                                                )
                                            )
                                        {
                                            Ok(_) => {
                                                crate::log_trading!(
                                                    error,
                                                    "Tx Failed due to missing latest state update"
                                                );
                                            }
                                            Err(_) => {
                                                crate::log_trading!(
                                                    error,
                                                    "Tx Failed due to missing latest state update"
                                                );
                                            }
                                        }
                                        return;
                                    }
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
                                            transaction_queue_to_confirm_relayer_latest_state(
                                                last_state_output.clone(),
                                                tx,
                                                next_state_output.clone()
                                            )
                                        }
                                        Err(arg) => { Err(arg.to_string()) }
                                    };
                                    let sender_clone = sender.clone();
                                    match sender_clone.send(tx_hash_result.clone()) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            crate::log_trading!(
                                                error,
                                                "error in sender at line 2037"
                                            );
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
                                                format!("tx_hash_result-{:?}", tx_hash),
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
                                    crate::log_trading!(
                                        error,
                                        "Error:ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX : arg:{:#?} for order_id:{}",
                                        arg,
                                        trader_order.uuid
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
                            crate::log_trading!(
                                error,
                                "Zkos Order string not found for order_id:{}",
                                trader_order.uuid
                            );
                            let sender_clone = sender.clone();
                            let fn_response_tx_hash = Err(
                                format!(
                                    "Zkos Order String Not Found for order_id:{}",
                                    trader_order.uuid
                                )
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
                        let is_state_updated = is_state_updated(&last_state_output);
                        if !is_state_updated {
                            let sender_clone = sender.clone();
                            match sender_clone.send(Err(
                                "Tx Failed due to missing latest state update".to_string(),
                            )) {
                                Ok(_) => {
                                    crate::log_trading!(
                                        error,
                                        "Tx Failed due to missing latest state update"
                                    );
                                }
                                Err(_) => {
                                    crate::log_trading!(
                                        error,
                                        "Tx Failed due to missing latest state update"
                                    );
                                }
                            }
                            return;
                        }
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
                                crate::log_trading!(error, "error in sender at line 2168");
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
                                    format!("tx_hash_result-{:?}", tx_hash),
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
                        crate::log_trading!(
                            error,
                            "Output memo not Found for order_id:{}",
                            trader_order.uuid
                        );
                        let sender_clone: mpsc::Sender<Result<String, String>> = sender.clone();
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
