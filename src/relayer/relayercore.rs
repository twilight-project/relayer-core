use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
use crate::relayer::*;
use address::Network;
use relayerwalletlib::zkoswalletlib::util::create_output_state_for_trade_lend_order;
use std::sync::{Arc, Mutex, RwLock,mpsc};
use std::fs::File;
use std::io::prelude::*;
use relayerwalletlib::order::*;
use relayerwalletlib::lend::*;
use relayerwalletlib::zkoswalletlib::programcontroller::ContractManager;
use utxo_in_memory::db::LocalDBtrait;
// use stopwatch::Stopwatch;
lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_NORMAL_ORDER")));
    pub static ref THREADPOOL_NORMAL_ORDER_INSERT: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(8, String::from("THREADPOOL_NORMAL_ORDER_INSERT")));
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(2, String::from("THREADPOOL_URGENT_ORDER")));
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_BULK_PENDING_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(12, String::from("THREADPOOL_FIFO_ORDER")));
    pub static ref THREADPOOL_ZKOS_TRADER_ORDER: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(5, String::from("THREADPOOL_ZKOS")));
    pub static ref THREADPOOL_ZKOS_FIFO: Mutex<ThreadPool> =
        Mutex::new(ThreadPool::new(1, String::from("THREADPOOL_ZKOS")));
    pub static ref CONTRACTMANAGER: Arc<Mutex<ContractManager>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let contract_manager = ContractManager::import_program(&WALLET_PROGRAM_PATH.clone());
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
    let command_clone_for_zkos = command.clone();
    match command {
        RpcCommand::CreateTraderOrder(rpc_request, metadata, _zkos_hex_string) => {
            let buffer = THREADPOOL_NORMAL_ORDER.lock().unwrap();
            buffer.execute(move || {

                let (orderdata, status) = TraderOrder::new_order(rpc_request.clone());
                let orderdata_clone=orderdata.clone();
                let orderdata_clone_for_zkos=orderdata.clone();
            
                let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let  is_order_duplicate = trader_order_db.set_order_check(orderdata.account_id.clone()).clone();
                drop(trader_order_db);
                if is_order_duplicate{

                    if orderdata_clone.order_status == OrderStatus::FILLED {

                        let buffer_insert = THREADPOOL_NORMAL_ORDER_INSERT.lock().unwrap();
                            buffer_insert.execute(move || {

                                let chain_result_receiver=  zkos_order_handler(ZkosTxCommand::CreateTraderOrderTX(orderdata_clone_for_zkos, command_clone_for_zkos));
                                let zkos_receiver =  chain_result_receiver.lock().unwrap();
                                    match zkos_receiver.recv() {
                                        Ok(chain_message)=>{
                                            match chain_message{
                                                Ok(tx_hash)=>{
                                                    let order_state = orderdata.orderinsert_localdb(status);
                                                    let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                                    let completed_order = trader_order_db.add(orderdata_clone, command_clone);
                                                    drop(trader_order_db);
                                                }
                                                Err(verification_error)=>{
                                                    let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                                    let _ = trader_order_db.remove_order_check(orderdata_clone.account_id.clone());
                                                    drop(trader_order_db);
                                                }
                                            }
                                        }
                                        Err(arg)=>{
                                            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                                                    let _ = trader_order_db.remove_order_check(orderdata_clone.account_id.clone());
                                                    drop(trader_order_db);
                                        }


                                    }
                            
                            });
                        drop(buffer_insert);

                        


                    }
                    else if  orderdata_clone.order_status == OrderStatus::PENDING {
                        //submit limit order after checking from chain or your db about order existanse of utxo
                        let order_state = orderdata.orderinsert_localdb(false);
                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                        let completed_order = trader_order_db.add(orderdata_clone, command_clone);
                        drop(trader_order_db);
                    }
                }
                else{
                    // send event for txhash with error saying order already exist in the relayer
                    Event::new(Event::TxHash(orderdata.uuid, orderdata.account_id, "Duplicate order found in the database with same public key".to_string(), orderdata.order_type, OrderStatus::CANCELLED, std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    .to_string(),None), String::from("tx_duplicate_error"),
                    LENDPOOL_EVENT_LOG.clone().to_string());
                }

            });
            drop(buffer);
        }
        RpcCommand::ExecuteTraderOrder(rpc_request, metadata, _zkos_hex_string) => {
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
                        let mut order_updated_clone= order.clone();
                        match order_updated_clone.order_status {
                            OrderStatus::FILLED => {
                                let (payment, order_status) = order_updated_clone.check_for_settlement(
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
                                            (lendpool.nonce+1) as u32,
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
                                            (lendpool.total_locked_value.round() + payment.round()) as u64,
                                            lendpool.total_pool_share.round() as u64,
                                            0,
                                        );
                                     let  chain_result_receiver=   zkos_order_handler(ZkosTxCommand::ExecuteTraderOrderTX(
                                            order_clone.clone(),
                                            command_clone.clone(),
                                            lendpool.last_output_state.clone(),
                                            next_output_state.clone(),
                                        ));
                                        let zkos_receiver =  chain_result_receiver.lock().unwrap();


                                        match zkos_receiver.recv() {
                                            Ok(chain_message)=>{
                                                match chain_message{
                                                    Ok(tx_hash)=>{
                                                        *order =order_clone.clone();
                                                        order_updated_clone.order_remove_from_localdb();
                                                        lendpool.add_transaction(
                                                            LendPoolCommand::AddTraderOrderSettlement(
                                                                command_clone,
                                                                order_clone.clone(),
                                                                payment,
                                                                next_output_state
                                                            ),
                                                        );
                                                    }
                                                    Err(verification_error)=>{
                                                        
                                                    }
                                                }
                                            }
                                            Err(arg)=>{
                                               
                                            }
    
    
                                        }
                                       
                                        drop(lendpool);

                                      
                                        drop(order);
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
                let is_order_exist:bool;
                
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                is_order_exist = lendorder_db.set_order_check(rpc_request.account_id.clone());
                drop(lendorder_db);

                if is_order_exist{
                let (tlv0, tps0) = lend_pool.get_lendpool();
                let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, tlv0, tps0);
                // let mut lendorder: LendOrder = LendOrder::new_order(rpc_request, 20048615383.0, 2000000.0);
                lendorder.order_status = OrderStatus::FILLED;
                lend_pool.add_transaction(LendPoolCommand::LendOrderCreateOrder(
                    command_clone,
                    lendorder.clone(),
                    lendorder.deposit,
                ));
                }

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

pub fn zkos_order_handler(command: ZkosTxCommand)->Arc<Mutex<mpsc::Receiver<Result<String, std::string::String>>>>{
    let command_clone = command.clone();
    let (sender, receiver) = mpsc::channel();
    let receiver_mutex = Arc::new(Mutex::new(receiver));
    if ENABLE_ZKOS_CHAIN_TRANSACTION.clone(){
        match command {

            ZkosTxCommand::CreateTraderOrderTX(trader_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || match trader_order.order_status {
                    OrderStatus::FILLED => {
                        match rpc_command {
                            RpcCommand::CreateTraderOrder(order_request, meta,_zkos_hex_string,) => {
                               
                                let zkos_create_order_result=ZkosCreateOrder::decode_from_hex_string(_zkos_hex_string);
                                // create transaction
                                match zkos_create_order_result {
                                    Ok(zkos_create_order) => {
                                        let mut file = File::create(format!("./{:?}_zkos_create_order.txt",trader_order.uuid.clone())).unwrap();
                                        file.write_all(
                                            &serde_json::to_vec(&zkos_create_order.clone()).unwrap(),
                                        )
                                        .unwrap();
                                        let mut file_bin =
                                            File::create(format!("./{:?}_zkos_create_order_file_bin.txt",trader_order.uuid.clone())).unwrap();
                                        file_bin
                                            .write_all(
                                                &serde_json::to_vec(
                                                    &bincode::serialize(&zkos_create_order.clone())
                                                        .unwrap(),
                                                )
                                                .unwrap(),
                                            )
                                            .unwrap();

                                 let result_output = update_trader_output_memo(zkos_create_order.output, trader_order.entryprice.round() as u64, trader_order.positionsize.round() as u64);

println!("trader_order.entryprice.round() : {:?} \n trader_order.positionsize.round() : {:?} ,",trader_order.entryprice.round() as u64, trader_order.positionsize.round() as u64);

                                 let output_memo_bin = bincode::serialize(&result_output.clone().unwrap().clone()).unwrap();
                                 let output_memo_hex = hex::encode(&output_memo_bin);
                                 println!("\n output_memo_hex: {:?} \n", output_memo_hex);
                                 println!("\n output_memo_orignal: {:?} \n", result_output.clone().unwrap());
                                        let transaction = create_trade_order(
                                            zkos_create_order.input,
                                            result_output.unwrap(),
                                            zkos_create_order.signature,
                                            zkos_create_order.proof,
                                            &ContractManager::import_program(
                                                &WALLET_PROGRAM_PATH.clone(),
                                            ),
                                            Network::Mainnet,
                                            5u64,
                                        );
                                    
                                        let mut file = File::create("./traderOrder_transaction.txt").unwrap();
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
                                    let sender_clone = sender.clone();
                                    sender_clone.send(tx_hash_result.clone()).unwrap();
    
                                    match tx_hash_result{
                                        Ok(tx_hash)=>{
                                            let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&tx_hash).unwrap(),
                                            0,
                                            );
                                            Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, tx_hash, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                            .unwrap()
                                            .as_micros()
                                            .to_string(),Some(output_memo_hex)), String::from("tx_hash_result"),
                                            LENDPOOL_EVENT_LOG.clone().to_string());
                                        }
                                        Err(arg)=>{
                                            let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&arg).unwrap(),
                                            0,
                                            );
                                    
                                            Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, arg, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                            .unwrap()
                                            .as_micros()
                                            .to_string(),None), String::from("tx_hash_error"),
                                            LENDPOOL_EVENT_LOG.clone().to_string());
                                        }
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
            
            ZkosTxCommand::CreateLendOrderTX(lend_order, rpc_command,last_state_output,next_state_output) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    RpcCommand::CreateLendOrder(order_request, meta,_zkos_hex_string,) =>{

                        let zkos_create_order_result=ZkosCreateOrder::decode_from_hex_string(_zkos_hex_string);

                        match zkos_create_order_result {
                            Ok(zkos_create_order) => {
                                let mut file = File::create("./zkos_lend_create_order.txt").unwrap();
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

                                let contract_owner_sk =get_sk_from_fixed_wallet();
                                let contract_owner_pk = get_pk_from_fixed_wallet();
            
                                let lock_error =get_lock_error_for_lend_create(lend_order.clone());

                              let result_poolshare_output =   update_lender_output_memo(
                                zkos_create_order.output.clone(),
                                (lend_order.npoolshare/10000.0).round() as u64,
                                );
                                let output_memo_bin_old = bincode::serialize(&zkos_create_order.output.clone()).unwrap();
                                let output_memo_hex_old = hex::encode(&output_memo_bin_old);
                                println!("\n output_memo_hex_old: {:?} \n", output_memo_hex_old);
                                println!("\n output_memo_orignal_old: {:?} \n", zkos_create_order.output);

                                let output_memo_bin = bincode::serialize(&result_poolshare_output.clone().unwrap().clone()).unwrap();
                                let output_memo_hex = hex::encode(&output_memo_bin);
                                println!("\n output_memo_hex: {:?} \n", output_memo_hex);
                                println!("\n output_memo_orignal: {:?} \n", result_poolshare_output.clone().unwrap());

                                println!("lock_error : {:?}",lock_error);
                                println!("zkos_create_order.input.clone() : {:?}",zkos_create_order.input.clone());
                                println!("zkos_create_order.output.clone() : {:?}",result_poolshare_output.clone().unwrap());
                                println!("last_state_output.clone() : {:?}",last_state_output.clone());
                                println!("next_state_output.clone() : {:?}",next_state_output.clone());
                                println!("lock_error : {:?}",lock_error);
                                println!("lock_error : {:?}",lock_error);


                                let transaction =  create_lend_order_transaction(
                                    zkos_create_order.input.clone(), 
                                    // zkos_create_order.output,
                                    result_poolshare_output.unwrap(),
                                    last_state_output.clone(),   
                                    next_state_output.clone(), 
                                    zkos_create_order.signature, 
                                    zkos_create_order.proof, 
                                    &ContractManager::import_program(
                                        &WALLET_PROGRAM_PATH.clone()
                                    ),
                                    Network::Mainnet,
                                    1u64,                      
                                    last_state_output.as_output_data().get_owner_address().unwrap().clone(), 
                                    lock_error, 
                                    contract_owner_sk,
                                    contract_owner_pk, 
                                );

                                
                            
                                let mut file = File::create("./transaction.txt").unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&transaction.clone()).unwrap(),
                                )
                                .unwrap();

                                match transaction.clone() {
                                    Ok(tx)=>{
                                    let verify_tx =  tx.verify();
                                    println!("tx hex : {:?}",hex::encode(bincode::serialize(&tx).unwrap()));
                                        println!("verify:{:?}",verify_tx);

                                    }
                                    Err(arg)=>println!("error at line 859 :{:?}",arg)
                                }


                                let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                    Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                    Err(arg)=>{Err(arg.to_string())}
                                };
                                let sender_clone = sender.clone();
                                    sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                    bincode::serialize(&lend_order.uuid).unwrap(),
                                    serde_json::to_string(&tx_hash).unwrap(),
                                    0,
                                );
                                Event::new(Event::TxHash(lend_order.uuid, lend_order.account_id, tx_hash, lend_order.order_type, lend_order.order_status, std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),Some(output_memo_hex)), String::from("tx_hash_result"),
                                LENDPOOL_EVENT_LOG.clone().to_string());
                            
                            }
                                Err(arg)=>{let _ = tx_hash_storage.add(
                                    bincode::serialize(&lend_order.uuid).unwrap(),
                                    serde_json::to_string(&arg).unwrap(),
                                    0,
                                );
                            
                                Event::new(Event::TxHash(lend_order.uuid, lend_order.account_id, arg, lend_order.order_type, lend_order.order_status, std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),None), String::from("tx_hash_error"),
                                LENDPOOL_EVENT_LOG.clone().to_string());
                            
                            }
                            }
                            drop(tx_hash_storage);
                                
                            }
                            Err(arg) => {
                                println!(
                                    "Error:ZkosTxCommand::CreateLendOrderTX : arg:{:#?}",
                                    arg
                                );
                            }
                        }


                    }
                    _ => {}
            });
            drop(buffer);
            }
                       
            ZkosTxCommand::ExecuteLendOrderTX(lend_order, rpc_command,last_state_output,next_state_output) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    RpcCommand::ExecuteLendOrder(order_request, meta,_zkos_hex_string,) =>{

                        println!("lend_order:{:#?}",lend_order.clone());
                        
                        let zkos_settle_msg_result=ZkosSettleMsg::decode_from_hex_string(_zkos_hex_string);

                        match zkos_settle_msg_result {
                            Ok(zkos_settle_msg) => {
                                let mut file = File::create(format!("./zkos_create_order_{:?}.txt",lend_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&zkos_settle_msg.clone()).unwrap(),
                                )
                                .unwrap();
                                let mut file_bin =
                                    File::create(format!("./zkos_create_order_file_bin_{:?}.txt",lend_order.uuid.clone())).unwrap();
                                file_bin
                                    .write_all(
                                        &serde_json::to_vec(
                                            &bincode::serialize(&zkos_settle_msg.clone())
                                                .unwrap(),
                                        )
                                        .unwrap(),
                                    )
                                    .unwrap();

                           
                                let contract_owner_sk =get_sk_from_fixed_wallet();
                                let contract_owner_pk = get_pk_from_fixed_wallet();
        
                                let lock_error =get_lock_error_for_lend_settle(lend_order.clone());

                                println!("lock_error: {:?}",lock_error);
                                println!("zkos_settle_msg.output: {:?}",zkos_settle_msg.output);
                                println!("last_state_output: {:?}",last_state_output);
                                println!("next_state_output: {:?}",next_state_output);



                                let transaction = create_lend_order_settlement_transaction(
                                zkos_settle_msg.output, 
                                lend_order.new_lend_state_amount.round() as u64, 
                                &ContractManager::import_program(
                                    &WALLET_PROGRAM_PATH.clone()
                                ),
                                Network::Mainnet,
                                1u64,                      
                                last_state_output.as_output_data().get_owner_address().unwrap().clone(),  
                                last_state_output, 
                                next_state_output,
                                lock_error,
                                contract_owner_sk, 
                                contract_owner_pk, 
                            );
                            match transaction.clone() {
                                Ok(tx)=>{
                                let verify_tx =  tx.verify();
                                println!("tx hex : {:?}",hex::encode(bincode::serialize(&tx).unwrap()));
                                    println!("verify:{:?}",verify_tx);

                                }
                                Err(arg)=>println!("error at line 859 :{:?}",arg)
                            }
                            
                                let mut file = File::create(format!("./transaction_{:?}.txt",lend_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&transaction.clone()).unwrap(),
                                )
                                .unwrap();

                                let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                    Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                    Err(arg)=>{Err(arg.to_string())}
                                };
                                let sender_clone = sender.clone();
                                sender_clone.send(tx_hash_result.clone()).unwrap();
                                let mut tx_hash_storage =
                                TXHASH_STORAGE.lock().unwrap();

                                let mut file =
                                File::create(format!("./ZKOS_TRANSACTION_RPC_ENDPOINT_{:?}.txt",lend_order.uuid.clone()))
                                    .unwrap();
                            file.write_all(
                                &serde_json::to_vec(&tx_hash_result.clone())
                                    .unwrap(),
                            )
                            .unwrap();

                            match tx_hash_result{
                                Ok(tx_hash)=>{let _ = tx_hash_storage.add(
                                    bincode::serialize(&lend_order.uuid).unwrap(),
                                    serde_json::to_string(&tx_hash).unwrap(),
                                    0,
                                );
                                Event::new(Event::TxHash(lend_order.uuid, lend_order.account_id, tx_hash, lend_order.order_type, lend_order.order_status, std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),None), String::from("tx_hash_result"),
                                LENDPOOL_EVENT_LOG.clone().to_string());
                            }
                                Err(arg)=>{let _ = tx_hash_storage.add(
                                    bincode::serialize(&lend_order.uuid).unwrap(),
                                    serde_json::to_string(&arg).unwrap(),
                                    0,
                                );
                                Event::new(Event::TxHash(lend_order.uuid, lend_order.account_id, arg, lend_order.order_type, lend_order.order_status, std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),None), String::from("tx_hash_error"),
                                LENDPOOL_EVENT_LOG.clone().to_string());
                            
                            }
                            }
                            drop(tx_hash_storage);
                                
                            }
                            Err(arg) => {
                                println!(
                                    "Error:ZkosTxCommand::ExecuteLendOrderTX : arg:{:#?}",
                                    arg
                                );
                            }
                        }


                    }
                    _ => {}
            });
            drop(buffer);
            }
            
            ZkosTxCommand::ExecuteTraderOrderTX(trader_order, rpc_command,last_state_output,next_state_output) => { 
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || 
                    match rpc_command {
                    RpcCommand::ExecuteTraderOrder(order_request, meta,zkos_hex_string,) =>{

                        let zkos_settle_msg_result=ZkosSettleMsg::decode_from_hex_string(zkos_hex_string);

                        match zkos_settle_msg_result {
                            Ok(zkos_settle_msg) => {
                                let mut file = File::create(format!("./zkos_settle_msg_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&zkos_settle_msg.clone()).unwrap(),
                                )
                                .unwrap();
                                let mut file_bin =
                                    File::create(format!("./zkos_settle_msg_file_bin_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file_bin
                                    .write_all(
                                        &serde_json::to_vec(
                                            &bincode::serialize(&zkos_settle_msg.clone())
                                                .unwrap(),
                                        )
                                        .unwrap(),
                                    )
                                    .unwrap();

                                

                                let contract_owner_sk =get_sk_from_fixed_wallet();

                                let contract_owner_pk = get_pk_from_fixed_wallet();

                                let lock_error= get_lock_error_for_trader_settle(trader_order.clone());

                                let margin_dif =( trader_order.available_margin.round()  - trader_order.initial_margin.round()).clone() as u64  - trader_order.unrealized_pnl.round() as u64;
                                
                                println!("\n\n next_state_output.clone() : {:?}",next_state_output.clone());
                                println!("\n\n last_state_output.clone() : {:?}",last_state_output.clone());

                                println!("margin_dif : {:?}",margin_dif);
                                println!(" \n zkos_settle_msg.output.clone() : {:?}",zkos_settle_msg.output.clone());
                                println!("\n\n trader_order.available_margin.clone().round() : {:?}",trader_order.available_margin.clone().round() as u64);
                                println!("\n\n trader_order.settlement_price.round() : {:?}",trader_order.settlement_price.round() as u64);
                                println!("\n\n lock_error : {:?}",lock_error);


                                let transaction=  settle_trader_order(
                                        zkos_settle_msg.output.clone(),  
                                        trader_order.available_margin.clone().round() as u64,                            
                                        &ContractManager::import_program(
                                            &WALLET_PROGRAM_PATH.clone()
                                        ),
                                        Network::Mainnet,
                                        1u64,                      
                                        last_state_output.as_output_data().get_owner_address().unwrap().clone(), 
                                    last_state_output.clone(),   
                                    next_state_output.clone(), 
                                    lock_error,  
                                    margin_dif, 
                                        trader_order.settlement_price.round() as u64, 
                                        contract_owner_sk, 
                                        contract_owner_pk,
                                    );

                                                                    
                                let mut file = File::create(format!("./settle_transaction_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&transaction.clone()).unwrap(),
                                )
                                .unwrap();


                                match transaction.clone() {
                                    Ok(tx)=>{
                                    let verify_tx =  tx.verify();
                                    println!("tx hex : {:?}",hex::encode(bincode::serialize(&tx).unwrap()));
                                        println!("verify:{:?}",verify_tx);

                                    }
                                    Err(arg)=>println!("error at line 859 :{:?}",arg)
                                }

                                        let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                            Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                            Err(arg)=>{Err(arg.to_string())}
                                        };
                                        let sender_clone = sender.clone();
                                        sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                        );
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, tx_hash, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),None), String::from("tx_hash_result"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());
                                    
                                    }
                                        Err(arg)=>{let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&arg).unwrap(),
                                            0,
                                        );
                                    
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, arg, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),None), String::from("tx_hash_error"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());}
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
        );
            drop(buffer);
        }
           
            ZkosTxCommand::CancelTraderOrderTX(trader_order, rpc_command) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || match rpc_command {
                    RpcCommand::CancelTraderOrder(order_request, meta,_zkos_hex_string,) =>{

                        let zkos_create_order_result=ZkosCreateOrder::decode_from_hex_string(_zkos_hex_string);

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
                                let sender_clone = sender.clone();
                                sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                    "Error:ZkosTxCommand::CancelTraderOrderTX : arg:{:#?}",
                                    arg
                                );
                            }
                        }


                    }
                    _ => {}
            });
            drop(buffer);}
            
            ZkosTxCommand::CreateTraderOrderLIMITTX(trader_order, zkos_string) => {
                let buffer = THREADPOOL_ZKOS_TRADER_ORDER.lock().unwrap();
                buffer.execute(move || match trader_order.order_status {
                    OrderStatus::FILLED => {
                       match zkos_string {
                        Some(zkos_create_order_result)=>{

                       
                                let zkos_create_order_result=ZkosCreateOrder::decode_from_hex_string(zkos_create_order_result);
                                // create transaction
                                match zkos_create_order_result {
                                    Ok(zkos_create_order) => {
                                        let mut file = File::create(format!("./{:?}_zkos_create_order.txt",trader_order.uuid.clone())).unwrap();
                                        file.write_all(
                                            &serde_json::to_vec(&zkos_create_order.clone()).unwrap(),
                                        )
                                        .unwrap();
                                        let mut file_bin =
                                            File::create(format!("./{:?}_zkos_create_order_file_bin.txt",trader_order.uuid.clone())).unwrap();
                                        file_bin
                                            .write_all(
                                                &serde_json::to_vec(
                                                    &bincode::serialize(&zkos_create_order.clone())
                                                        .unwrap(),
                                                )
                                                .unwrap(),
                                            )
                                            .unwrap();

                                 let result_output = update_trader_output_memo(zkos_create_order.output, trader_order.entryprice.round() as u64, trader_order.positionsize.round() as u64);

println!("trader_order.entryprice.round() : {:?} \n trader_order.positionsize.round() : {:?} ,",trader_order.entryprice.round() as u64, trader_order.positionsize.round() as u64);

                                 let output_memo_bin = bincode::serialize(&result_output.clone().unwrap().clone()).unwrap();
                                 let output_memo_hex = hex::encode(&output_memo_bin);
                                 println!("\n output_memo_hex: {:?} \n", output_memo_hex);
                                 println!("\n output_memo_orignal: {:?} \n", result_output.clone().unwrap());
                                        let transaction = create_trade_order(
                                            zkos_create_order.input,
                                            result_output.unwrap(),
                                            zkos_create_order.signature,
                                            zkos_create_order.proof,
                                            &ContractManager::import_program(
                                                &WALLET_PROGRAM_PATH.clone(),
                                            ),
                                            Network::Mainnet,
                                            5u64,
                                        );
                                    
                                        let mut file = File::create("./traderOrder_transaction.txt").unwrap();
                                        file.write_all(
                                            &serde_json::to_vec(&transaction.clone()).unwrap(),
                                        )
                                        .unwrap();
    
                                        let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                            Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                            Err(arg)=>{Err(arg.to_string())}
                                        };
                                        let sender_clone = sender.clone();
                                        sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                        );
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, tx_hash, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),Some(output_memo_hex)), String::from("tx_hash_result"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());
                                    }
                                        Err(arg)=>{let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&arg).unwrap(),
                                            0,
                                        );
                                    
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, arg, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),None), String::from("tx_hash_error"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());}
                                    }
                                    drop(tx_hash_storage);
                                              
    
    
                                      
                                    }
                                    Err(arg) => {
                                        println!(
                                            "Error:ZkosTxCommand::CreateTraderOrderLIMITTX : arg:{:#?}",
                                            arg
                                        );
                                    }
                                }
                            }
                                // chain tx link for multiple order in one block
                                // batch and seperate orde rby if in trader settle  
                                None=> {
                                    println!(
                                        "Zkos Order string not found"
                                    );
                                    let sender_clone = sender.clone();
                                    let fn_response_tx_hash = Err("Zkos Order String Not Found".to_string());
                                    sender_clone.send(fn_response_tx_hash.clone()).unwrap();
                                }
                            }
                      
                    }
                    _ => {}
                });
                drop(buffer);




            }
            
            ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX(trader_order, zkos_string, last_state_output, next_state_output) => {

            let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
            buffer.execute(move || 
               match zkos_string {
                    Some(zkos_settle_msg_result) =>{

                        println!("lend_order:{:#?}",trader_order.clone());
                        let zkos_settle_msg_result=ZkosCreateOrder::decode_from_hex_string(zkos_settle_msg_result);
                       
                        match zkos_settle_msg_result {
                            Ok(zkos_settle_msg) => {
                            let mut file = File::create(format!("./zkos_settle_msg_{:?}.txt",trader_order.uuid.clone())).unwrap();
                            file.write_all(
                                &serde_json::to_vec(&zkos_settle_msg.clone()).unwrap(),
                            )
                            .unwrap();
                            let mut file_bin =
                                File::create(format!("./zkos_settle_msg_file_bin_{:?}.txt",trader_order.uuid.clone())).unwrap();
                            file_bin
                                .write_all(
                                    &serde_json::to_vec(
                                        &bincode::serialize(&zkos_settle_msg.clone())
                                            .unwrap(),
                                    )
                                    .unwrap(),
                                )
                                .unwrap();

                            

                            let contract_owner_sk =get_sk_from_fixed_wallet();

                            let contract_owner_pk = get_pk_from_fixed_wallet();

                            let lock_error= get_lock_error_for_trader_settle(trader_order.clone());

                            let margin_dif =( trader_order.available_margin.round()  - trader_order.initial_margin.round()).clone() as u64  - trader_order.unrealized_pnl.round() as u64;
                            
                            println!("\n\n next_state_output.clone() : {:?}",next_state_output.clone());
                            println!("\n\n last_state_output.clone() : {:?}",last_state_output.clone());

                            println!("margin_dif : {:?}",margin_dif);
                            println!(" \n zkos_settle_msg.output.clone() : {:?}",zkos_settle_msg.output.clone());
                            println!("\n\n trader_order.available_margin.clone().round() : {:?}",trader_order.available_margin.clone().round() as u64);
                            println!("\n\n trader_order.settlement_price.round() : {:?}",trader_order.settlement_price.round() as u64);
                            println!("\n\n lock_error : {:?}",lock_error);


                            let transaction=  settle_trader_order(
                                    zkos_settle_msg.output.clone(),  
                                    trader_order.available_margin.clone().round() as u64,                            
                                    &ContractManager::import_program(
                                        &WALLET_PROGRAM_PATH.clone()
                                    ),
                                    Network::Mainnet,
                                    1u64,                      
                                    last_state_output.as_output_data().get_owner_address().unwrap().clone(), 
                                last_state_output.clone(),   
                                next_state_output.clone(), 
                                lock_error,  
                                margin_dif, 
                                    trader_order.settlement_price.round() as u64, 
                                    contract_owner_sk, 
                                    contract_owner_pk,
                                );

                                                                
                            let mut file = File::create(format!("./settle_transaction_{:?}.txt",trader_order.uuid.clone())).unwrap();
                            file.write_all(
                                &serde_json::to_vec(&transaction.clone()).unwrap(),
                            )
                            .unwrap();


                            match transaction.clone() {
                                Ok(tx)=>{
                                let verify_tx =  tx.verify();
                                println!("tx hex : {:?}",hex::encode(bincode::serialize(&tx).unwrap()));
                                    println!("verify:{:?}",verify_tx);

                                }
                                Err(arg)=>println!("error at line 859 :{:?}",arg)
                            }

                                    let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                        Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                        Err(arg)=>{Err(arg.to_string())}
                                    };
                                    let sender_clone = sender.clone();
                                    sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                    );
                                    Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, tx_hash, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_micros()
                                    .to_string(),None), String::from("tx_hash_result"),
                                    LENDPOOL_EVENT_LOG.clone().to_string());
                                
                                }
                                    Err(arg)=>{let _ = tx_hash_storage.add(
                                        bincode::serialize(&trader_order.uuid).unwrap(),
                                        serde_json::to_string(&arg).unwrap(),
                                        0,
                                    );
                                
                                    Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, arg, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_micros()
                                    .to_string(),None), String::from("tx_hash_error"),
                                    LENDPOOL_EVENT_LOG.clone().to_string());}
                                }
                                drop(tx_hash_storage);
                            
                        }
                        Err(arg) => {
                            println!(
                                "Error:ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX : arg:{:#?}",
                                arg
                            );
                        }
                    }


                }
                None => {println!(
                    "Zkos Order string not found"
                );
                let sender_clone = sender.clone();
                                    let fn_response_tx_hash = Err("Zkos Order String Not Found".to_string());
                                    sender_clone.send(fn_response_tx_hash.clone()).unwrap();
            }
        }
    );
        drop(buffer);

            }
            ZkosTxCommand::RelayerCommandTraderOrderLiquidateTX(trader_order, zkos_string, last_state_output, next_state_output) => {
                let buffer = THREADPOOL_ZKOS_FIFO.lock().unwrap();
                buffer.execute(move || 
                   match zkos_string {
                        Some(zkos_settle_msg_result) =>{
    
                            println!("lend_order:{:#?}",trader_order.clone());
                            let zkos_settle_msg_result=ZkosCreateOrder::decode_from_hex_string(zkos_settle_msg_result);
                           
                            match zkos_settle_msg_result {
                                Ok(zkos_settle_msg) => {
                                let mut file = File::create(format!("./zkos_settle_msg_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&zkos_settle_msg.clone()).unwrap(),
                                )
                                .unwrap();
                                let mut file_bin =
                                    File::create(format!("./zkos_settle_msg_file_bin_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file_bin
                                    .write_all(
                                        &serde_json::to_vec(
                                            &bincode::serialize(&zkos_settle_msg.clone())
                                                .unwrap(),
                                        )
                                        .unwrap(),
                                    )
                                    .unwrap();
    
                                
    
                                let contract_owner_sk =get_sk_from_fixed_wallet();
    
                                let contract_owner_pk = get_pk_from_fixed_wallet();
    
                                let lock_error= get_lock_error_for_trader_settle(trader_order.clone());
    
                                let margin_dif =( trader_order.available_margin.round()  - trader_order.initial_margin.round()).clone() as u64  - trader_order.unrealized_pnl.round() as u64;
                                
                                println!("\n\n next_state_output.clone() : {:?}",next_state_output.clone());
                                println!("\n\n last_state_output.clone() : {:?}",last_state_output.clone());
    
                                println!("margin_dif : {:?}",margin_dif);
                                println!(" \n zkos_settle_msg.output.clone() : {:?}",zkos_settle_msg.output.clone());
                                println!("\n\n trader_order.available_margin.clone().round() : {:?}",trader_order.available_margin.clone().round() as u64);
                                println!("\n\n trader_order.settlement_price.round() : {:?}",trader_order.settlement_price.round() as u64);
                                println!("\n\n lock_error : {:?}",lock_error);
    
    
                                let transaction=  settle_trader_order(
                                        zkos_settle_msg.output.clone(),  
                                        trader_order.available_margin.clone().round() as u64,                            
                                        &ContractManager::import_program(
                                            &WALLET_PROGRAM_PATH.clone()
                                        ),
                                        Network::Mainnet,
                                        1u64,                      
                                        last_state_output.as_output_data().get_owner_address().unwrap().clone(), 
                                    last_state_output.clone(),   
                                    next_state_output.clone(), 
                                    lock_error,  
                                    margin_dif, 
                                        trader_order.settlement_price.round() as u64, 
                                        contract_owner_sk, 
                                        contract_owner_pk,
                                    );
    
                                                                    
                                let mut file = File::create(format!("./settle_transaction_{:?}.txt",trader_order.uuid.clone())).unwrap();
                                file.write_all(
                                    &serde_json::to_vec(&transaction.clone()).unwrap(),
                                )
                                .unwrap();
    
    
                                match transaction.clone() {
                                    Ok(tx)=>{
                                    let verify_tx =  tx.verify();
                                    println!("tx hex : {:?}",hex::encode(bincode::serialize(&tx).unwrap()));
                                        println!("verify:{:?}",verify_tx);
    
                                    }
                                    Err(arg)=>println!("error at line 859 :{:?}",arg)
                                }
    
                                        let tx_hash_result:Result<std::string::String, std::string::String>=match transaction{
                                            Ok(tx)=>{relayerwalletlib::zkoswalletlib::chain::tx_commit_broadcast_transaction(tx)}
                                            Err(arg)=>{Err(arg.to_string())}
                                        };
                                        let sender_clone = sender.clone();
                                    sender_clone.send(tx_hash_result.clone()).unwrap();
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
                                        );
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, tx_hash, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),None), String::from("tx_hash_result"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());
                                    
                                    }
                                        Err(arg)=>{let _ = tx_hash_storage.add(
                                            bincode::serialize(&trader_order.uuid).unwrap(),
                                            serde_json::to_string(&arg).unwrap(),
                                            0,
                                        );
                                    
                                        Event::new(Event::TxHash(trader_order.uuid, trader_order.account_id, arg, trader_order.order_type, trader_order.order_status, std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_micros()
                                        .to_string(),None), String::from("tx_hash_error"),
                                        LENDPOOL_EVENT_LOG.clone().to_string());}
                                    }
                                    drop(tx_hash_storage);
                                
                            }
                            Err(arg) => {
                                println!(
                                    "Error:ZkosTxCommand::RelayerCommandTraderOrderSettleOnLimitTX : arg:{:#?}",
                                    arg
                                );
                            }
                        }
    
    
                    }
                    None => {println!(
                        "Zkos Order string not found"
                    );
                    let sender_clone = sender.clone();
                    let fn_response_tx_hash = Err("Zkos Order String Not Found".to_string());
                    sender_clone.send(fn_response_tx_hash.clone()).unwrap();
                }
            }
        );
            drop(buffer);
            }
        }
    
    
    }
    else {
        let fn_response_tx_hash=Err("ZKOS CHAIN TRANSACTION IS NOT ACTIVE".to_string());
        sender.send(fn_response_tx_hash).unwrap();
    }
return   Arc::clone(&receiver_mutex);
}





