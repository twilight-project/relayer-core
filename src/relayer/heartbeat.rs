use crate::config::*;
use crate::db::*;
use crate::pricefeederlib::price_feeder::receive_btc_price;
use crate::relayer::order_handler::client_cmd_receiver;
use crate::relayer::relayer_command_handler::relayer_event_handler;
use crate::relayer::zkos_handler::zkos_order_handler;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
use std::collections::HashMap;
use std::sync::{mpsc, Arc, RwLock};
use std::{thread, time};
use twilight_relayer_sdk::twilight_client_sdk::util::create_output_state_for_trade_lend_order;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;
pub fn heartbeat() {
    dotenv::dotenv().ok();
    // init_psql();
    crate::log_database!(info, "Looking for previous database...");
    match load_from_snapshot() {
        Ok(mut queue_manager) => {
            crate::log_database!(info, "Successfully loaded database snapshot");
            load_relayer_latest_state();
            queue_manager.process_queue();
        }
        Err(arg) => {
            crate::log_database!(error, "Unable to start Relayer - Error: {:?}", arg);
            tracing::error!("Critical startup failure, exiting");
            std::process::exit(0);
        }
    }

    thread::sleep(time::Duration::from_millis(100));

    thread::Builder::new()
        .name(String::from("BTC Binance Websocket Connection"))
        .spawn(move || {
            // Create a tokio runtime for the async function
            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = receive_btc_price().await {
                    crate::log_price!(error, "BTC price feeder error: {}", e);
                }
            });
        })
        .unwrap();
    thread::sleep(time::Duration::from_millis(500));
    let mut true_price = true;
    //get_localdb with single mutex unlock
    while true_price {
        let mut local_storage = LOCALDB.lock().unwrap();
        let currentprice = match local_storage.get("Latest_Price") {
            Some(price) => {
                true_price = false;
                price.clone()
            }
            None => {
                thread::sleep(time::Duration::from_millis(50));
                continue;
            }
        };
        let old_price = match local_storage.get("CurrentPrice") {
            Some(price) => price.clone(),
            None => {
                local_storage.insert("CurrentPrice".to_string(), currentprice);
                currentprice.clone()
            }
        };
        drop(local_storage);
    }

    thread::Builder::new()
        .name(String::from("price_check_and_update"))
        .spawn(move || loop {
            thread::sleep(time::Duration::from_millis(250));
            thread::spawn(move || {
                price_check_and_update();
            });
        })
        .unwrap();
    thread::sleep(time::Duration::from_millis(100));
    // main thread for scheduler
    thread::Builder::new()
        .name(String::from("heartbeat scheduler"))
        .spawn(move || {
            let mut scheduler = Scheduler::with_tz(chrono::Utc);

            // funding update every 1 hour //comments for local test
            // scheduler.every(600.seconds()).run(move || {
            scheduler.every((1).hour()).run(move || {
                if get_relayer_status() {
                    updatefundingrate_localdb(1.0);
                }
            });
            // scheduler.every(1.seconds()).run(move || {
            //     relayer_event_handler(RelayerCommand::RpcCommandPoolupdate());
            // });
            scheduler.every((15).minute()).run(move || {
                let result = snapshot();
                if let Err(arg) = result {
                    crate::log_heartbeat!(error, "Error taking snapshot: {:?}", arg);
                }
            });

            let thread_handle = scheduler.watch_thread(time::Duration::from_millis(1000));
            loop {
                thread::sleep(time::Duration::from_millis(100000000));
            }
        })
        .unwrap();

    thread::Builder::new()
        .name(String::from("json-RPC startserver"))
        .spawn(move || loop {
            crate::log_heartbeat!(info, "json-RPC server started");
            startserver();
            crate::log_heartbeat!(info, "json-RPC server stopped");
            crate::log_heartbeat!(info, "restarting json-RPC server");
            thread::sleep(time::Duration::from_millis(1000));
        })
        .unwrap();

    thread::Builder::new()
        .name(String::from("client_cmd_receiver"))
        .spawn(move || {
            thread::sleep(time::Duration::from_millis(10000));
            loop {
                client_cmd_receiver();
                thread::sleep(time::Duration::from_millis(1000));
            }
        })
        .unwrap();

    crate::log_heartbeat!(
        info,
        "Initialization done.................................."
    );
}

pub fn price_check_and_update() {
    let current_time = std::time::SystemTime::now();

    //get_localdb with single mutex unlock
    let local_storage = LOCALDB.lock().unwrap();
    let mut currentprice = match local_storage.get(&String::from("Latest_Price")) {
        Some(price) => price.clone(),
        None => {
            return;
        }
    };
    drop(local_storage);
    if get_relayer_status() {
        crate::log_price!(debug, "Updating current price to: {}", currentprice);

        Event::new(
            Event::CurrentPriceUpdate(currentprice.clone(), iso8601(&current_time.clone())),
            String::from("insert_CurrentPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );

        currentprice = currentprice.round();
        set_localdb("CurrentPrice", currentprice);
        crate::log_price!(debug, "Price updated to: {}", currentprice);
        let treadpool_pending_order = THREADPOOL_PRICE_CHECK_PENDING_ORDER.lock().unwrap();
        treadpool_pending_order.execute(move || {
            check_pending_limit_order_on_price_ticker_update_localdb(currentprice.clone());
        });
        let treadpool_liquidation_order = THREADPOOL_PRICE_CHECK_LIQUIDATION.lock().unwrap();
        treadpool_liquidation_order.execute(move || {
            check_liquidating_orders_on_price_ticker_update_localdb(currentprice.clone());
        });
        let treadpool_settling_order = THREADPOOL_PRICE_CHECK_SETTLE_PENDING.lock().unwrap();
        treadpool_settling_order.execute(move || {
            check_settling_limit_order_on_price_ticker_update_localdb(currentprice.clone());
        });
        drop(treadpool_pending_order);
        drop(treadpool_liquidation_order);
        drop(treadpool_settling_order);
    }
}

pub fn check_pending_limit_order_on_price_ticker_update_localdb(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    let mut get_open_order_short_list = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    if orderid_list_short_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveOpenLimitPrice(
                current_price.clone(),
                PositionType::SHORT,
            )),
            String::from("BulkSearchRemoveOpenLimitPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    if orderid_list_long_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveOpenLimitPrice(
                current_price.clone(),
                PositionType::LONG,
            )),
            String::from("BulkSearchRemoveOpenLimitPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(ServerTime::now().epoch),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }
        relayer_event_handler(RelayerCommand::PriceTickerOrderFill(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(limit_lock);
}

pub fn check_liquidating_orders_on_price_ticker_update_localdb(current_price: f64) {
    let liquidation_lock = LIQUIDATIONTICKERSTATUS.lock().unwrap();

    let mut get_open_order_short_list = TRADER_LP_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LP_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    if orderid_list_short_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveLiquidationPrice(
                current_price.clone(),
                PositionType::SHORT,
            )),
            String::from("BulkSearchRemoveLiquidationPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    if orderid_list_long_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveLiquidationPrice(
                current_price.clone(),
                PositionType::LONG,
            )),
            String::from("BulkSearchRemoveLiquidationPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(ServerTime::now().epoch),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }

        relayer_event_handler(RelayerCommand::PriceTickerLiquidation(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(liquidation_lock);
}
pub fn check_settling_limit_order_on_price_ticker_update_localdb(current_price: f64) {
    let limit_lock = SETTLEMENTLIMITSTATUS.lock().unwrap();
    let mut get_open_order_short_list = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);

    // Stop Loss and Take Profit Close LIMIT Price
    let mut get_sltp_order_short_list = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();
    let orderid_list_sltp_short: Vec<Uuid> =
        get_sltp_order_short_list.search_gt((current_price * 10000.0) as i64);
    drop(get_sltp_order_short_list);
    let mut get_sltp_order_long_list = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
    let orderid_list_sltp_long: Vec<Uuid> =
        get_sltp_order_long_list.search_lt((current_price * 10000.0) as i64);
    drop(get_sltp_order_long_list);
    // End of Stop Loss and Take Profit Close LIMIT Price

    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();

    // Stop Loss and Take Profit Close LIMIT Price
    let orderid_list_sltp_short_len = orderid_list_sltp_short.len();
    let orderid_list_sltp_long_len = orderid_list_sltp_long.len();
    // End of Stop Loss and Take Profit Close LIMIT Price

    if orderid_list_short_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveCloseLimitPrice(
                current_price.clone(),
                PositionType::SHORT,
            )),
            String::from("BulkSearchRemoveCloseLimitPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    if orderid_list_long_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveCloseLimitPrice(
                current_price.clone(),
                PositionType::LONG,
            )),
            String::from("BulkSearchRemoveCloseLimitPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    // SLTP
    if orderid_list_sltp_short_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(
                current_price.clone(),
                PositionType::SHORT,
            )),
            String::from("BulkSearchRemoveSLTPCloseLIMITPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    if orderid_list_sltp_long_len > 0 {
        Event::new(
            Event::SortedSetDBUpdate(SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(
                current_price.clone(),
                PositionType::LONG,
            )),
            String::from("BulkSearchRemoveSLTPCloseLIMITPrice"),
            CORE_EVENT_LOG.clone().to_string(),
        );
    }
    let total_order_count = orderid_list_short_len
        + orderid_list_long_len
        + orderid_list_sltp_short_len
        + orderid_list_sltp_long_len;
    if total_order_count > 0 {
        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(ServerTime::now().epoch),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }
        if orderid_list_sltp_short_len > 0 {
            orderid_list_short.extend(orderid_list_sltp_short);
        }
        if orderid_list_sltp_long_len > 0 {
            orderid_list_short.extend(orderid_list_sltp_long);
        }

        relayer_event_handler(RelayerCommand::PriceTickerOrderSettle(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(limit_lock);
}

pub fn updatefundingrate_localdb(psi: f64) {
    let current_time = std::time::SystemTime::now();
    let current_price = get_localdb(&String::from("CurrentPrice"));
    let fee = 0.0;
    let position_size_log = POSITION_SIZE_LOG.lock().unwrap();
    let totalshort = position_size_log.total_short_positionsize.clone();
    let totallong = position_size_log.total_long_positionsize.clone();
    let allpositionsize = position_size_log.totalpositionsize.clone();
    drop(position_size_log);
    crate::log_heartbeat!(
        info,
        "totalshort:{}, totallong:{}, allpositionsize:{}",
        totalshort,
        totallong,
        allpositionsize
    );
    //powi is faster then powf
    //psi = 8.0 or  ψ = 8.0
    let mut fundingrate;
    if allpositionsize == 0.0 {
        fundingrate = 0.0;
    } else {
        fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);
    }

    //positive funding if totallong > totalshort else negative funding
    if totallong > totalshort {
    } else {
        fundingrate = fundingrate * -1.0;
    }
    // updatefundingrateindb(fundingrate.clone(), current_price, current_time);
    crate::log_heartbeat!(info, "funding cycle processing...");
    let meta = Meta {
        metadata: {
            let mut hashmap = HashMap::new();
            hashmap.insert(
                String::from("request_server_time"),
                Some(
                    current_time
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                        .to_string(),
                ),
            );
            hashmap.insert(
                String::from("CurrentPrice"),
                Some(current_price.to_string()),
            );
            hashmap.insert(String::from("FundingRate"), Some(fundingrate.to_string()));
            hashmap.insert(
                String::from("FilledOnMarket"),
                Some(get_fee(FeeType::FilledOnMarket).to_string()),
            );
            hashmap.insert(
                String::from("FilledOnLimit"),
                Some(get_fee(FeeType::FilledOnLimit).to_string()),
            );
            hashmap
        },
    };
    // get_and_update_all_orders_on_funding_cycle(current_price, fundingrate.clone());
    fundingcycle(current_price, fundingrate, fee, current_time, meta);
    crate::log_heartbeat!(info, "fundingrate:{}", fundingrate);
}
use stopwatch::Stopwatch;
pub fn fundingcycle(
    current_price: f64,
    fundingrate: f64,
    fee: f64,
    current_time: std::time::SystemTime,
    metadata: Meta,
) {
    let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
    let orderdetails_array = trader_order_db.getall_mut();
    drop(trader_order_db);
    crate::log_heartbeat!(
        info,
        "applying funding rate to orders: {:#?}",
        orderdetails_array.len()
    );
    let length = orderdetails_array.len();
    let sw = Stopwatch::start_new();
    if length > 0 {
        let threadpool = ThreadPool::new(10, String::from("Funding cycle pool"));
        let mut poolbatch = PoolBatchOrder::new();
        let (send, recv) = mpsc::channel();
        let mut liquidity_update_short: Vec<(Uuid, i64)> = Vec::new();
        let mut liquidity_update_long: Vec<(Uuid, i64)> = Vec::new();
        for ordertx in orderdetails_array {
            let meta_clone = metadata.clone();
            let sender_clone = send.clone();
            threadpool.execute(move || {
                updatechangesineachordertxonfundingratechange_localdb(
                    ordertx,
                    fundingrate,
                    current_price,
                    fee,
                    meta_clone,
                    sender_clone,
                );
            });
        }

        for _i in 0..length {
            let (funding_payment, order, (uuid, price, position_type)) = recv.recv().unwrap();
            if funding_payment != 0.0 {
                poolbatch.add(funding_payment, order);

                match position_type {
                    PositionType::LONG => {
                        liquidity_update_long.push((uuid, price));
                    }
                    PositionType::SHORT => {
                        liquidity_update_short.push((uuid, price));
                    }
                }
            }
        }
        let mut liquidation_list_long = TRADER_LP_LONG.lock().unwrap();
        let _ = liquidation_list_long.update_bulk(liquidity_update_long);
        drop(liquidation_list_long);
        let mut liquidation_list_short = TRADER_LP_SHORT.lock().unwrap();
        let _ = liquidation_list_short.update_bulk(liquidity_update_short);
        drop(liquidation_list_short);

        // println!(
        //     "funding complete, poolbatch amount: {:#?}",
        //     poolbatch.amount
        // );
        relayer_event_handler(RelayerCommand::FundingCycle(
            poolbatch,
            metadata,
            fundingrate,
        ));
    } else {
        let poolbatch = PoolBatchOrder::new();
        relayer_event_handler(RelayerCommand::FundingCycle(
            poolbatch,
            metadata,
            fundingrate,
        ));
    }
    crate::log_heartbeat!(info, "funding cycle took {:#?}", sw.elapsed());
}

pub fn updatechangesineachordertxonfundingratechange_localdb(
    order: Arc<RwLock<TraderOrder>>,
    fundingratechange: f64,
    current_price: f64,
    mut fee: f64,
    meta: Meta,
    sender: mpsc::Sender<(f64, TraderOrder, (Uuid, i64, PositionType))>,
) {
    let mut ordertx = order.write().unwrap();
    if ordertx.order_status == OrderStatus::FILLED {
        //update available margin
        let funding_payment = (fundingratechange * ordertx.positionsize) / (current_price * 100.0);
        match ordertx.position_type {
            PositionType::LONG => {
                ordertx.available_margin -= funding_payment;
            }
            PositionType::SHORT => {
                ordertx.available_margin += funding_payment;
            }
        }

        if ordertx.available_margin < 0.0 {
            ordertx.available_margin = 0.0;
        }
        if ordertx.order_type == OrderType::MARKET {
            fee = get_fee(FeeType::FilledOnMarket);
        } else if ordertx.order_type == OrderType::LIMIT {
            fee = get_fee(FeeType::FilledOnLimit);
        }
        // update maintenancemargin
        ordertx.maintenance_margin = maintenancemargin(
            entryvalue(ordertx.initial_margin, ordertx.leverage),
            ordertx.bankruptcy_value,
            fee,
            fundingratechange,
        );

        // check if AM <= MM if true then call liquidate position else update liquidation price
        if ordertx.available_margin <= ordertx.maintenance_margin {
            //call liquidation
            let payment = ordertx.liquidate(current_price);
            ordertx.order_status = OrderStatus::LIQUIDATE;

            let output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
            let uuid_to_byte = match bincode::serialize(&ordertx.uuid.clone()) {
                Ok(uuid_v_u8) => uuid_v_u8,
                Err(_) => Vec::new(),
            };
            let output_option = match output_hex_storage.get_utxo_by_id(uuid_to_byte, 0) {
                Ok(output) => output,
                Err(_) => None,
            };

            drop(output_hex_storage);

            let mut lendpool = LEND_POOL_DB.lock().unwrap();

            let next_output_state = create_output_state_for_trade_lend_order(
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
                (lendpool.total_locked_value.round() - payment.round()) as u64,
                lendpool.total_pool_share.round() as u64,
                0,
            );
            let (sender, zkos_receiver) = mpsc::channel();
            crate::log_trading!(
                debug,
                "applying liquidation for order_id:{} due to low maintenance margin",
                ordertx.uuid
            );
            zkos_order_handler(
                ZkosTxCommand::RelayerCommandTraderOrderLiquidateTX(
                    ordertx.clone(),
                    output_option,
                    lendpool.last_output_state.clone(),
                    next_output_state.clone(),
                ),
                sender,
            );

            // let zkos_receiver = chain_result_receiver.lock().unwrap();

            match zkos_receiver.recv() {
                Ok(chain_message) => match chain_message {
                    Ok(tx_hash) => {
                        //remove position size
                        PositionSizeLog::remove_order(
                            ordertx.position_type.clone(),
                            ordertx.positionsize.clone(),
                        );
                        //remove liquidation price from liqudation set
                        match ordertx.position_type {
                            PositionType::LONG => {
                                let mut add_to_liquidation_list = TRADER_LP_LONG.lock().unwrap();
                                let _ = add_to_liquidation_list.remove(ordertx.uuid);
                                drop(add_to_liquidation_list);
                            }
                            PositionType::SHORT => {
                                let mut add_to_liquidation_list = TRADER_LP_SHORT.lock().unwrap();
                                let _ = add_to_liquidation_list.remove(ordertx.uuid);
                                drop(add_to_liquidation_list);
                            }
                        }

                        Event::new(
                            Event::SortedSetDBUpdate(SortedSetCommand::RemoveLiquidationPrice(
                                ordertx.uuid.clone(),
                                ordertx.position_type.clone(),
                            )),
                            format!("RemoveLiquidationPrice-{}", ordertx.uuid.clone()),
                            CORE_EVENT_LOG.clone().to_string(),
                        );

                        lendpool.add_transaction(LendPoolCommand::AddTraderOrderLiquidation(
                            RelayerCommand::PriceTickerLiquidation(
                                vec![ordertx.uuid.clone()],
                                meta,
                                current_price,
                            ),
                            ordertx.clone(),
                            payment,
                            next_output_state,
                        ));
                        // println!("dropping mutex LEND_POOL_DB");
                    }
                    Err(verification_error) => {
                        crate::log_trading!(
                            error,
                            "Error in line heartbeat.rs 631 for order_id:{} : {:?}",
                            ordertx.uuid,
                            verification_error
                        );
                    }
                },
                Err(arg) => {
                    crate::log_trading!(
                        error,
                        "Error in line heartbeat.rs 640 for order_id:{} : {:?}",
                        ordertx.uuid,
                        arg
                    );
                }
            }

            drop(lendpool);
        } else {
            ordertx.liquidation_price = liquidationprice(
                ordertx.entryprice,
                ordertx.positionsize,
                positionside(&ordertx.position_type),
                ordertx.maintenance_margin,
                ordertx.initial_margin,
            );
            relayer_event_handler(RelayerCommand::FundingOrderEventUpdate(
                ordertx.clone(),
                meta,
            ));
        }

        match ordertx.position_type {
            PositionType::LONG => {
                sender
                    .send((
                        funding_payment * -1.0,
                        ordertx.clone(),
                        (
                            ordertx.clone().uuid,
                            (ordertx.clone().liquidation_price * 10000.0) as i64,
                            ordertx.clone().position_type,
                        ),
                    ))
                    .unwrap();
            }
            PositionType::SHORT => {
                sender
                    .send((
                        funding_payment,
                        ordertx.clone(),
                        (
                            ordertx.uuid.clone(),
                            (ordertx.liquidation_price.clone() * 10000.0) as i64,
                            ordertx.position_type.clone(),
                        ),
                    ))
                    .unwrap();
            }
        }
    } else {
        sender
            .send((
                0.0,
                ordertx.clone(),
                (
                    ordertx.uuid.clone(),
                    (ordertx.liquidation_price.clone() * 10000.0) as i64,
                    ordertx.position_type.clone(),
                ),
            ))
            .unwrap();
    }
}
