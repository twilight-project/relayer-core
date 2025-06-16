use crate::config::*;
use crate::db::*;
// use crate::ordertest;
use crate::pricefeederlib::price_feeder::receive_btc_price;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
use relayerwalletlib::zkoswalletlib::util::create_output_state_for_trade_lend_order;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, RwLock};
use std::{thread, time};
use tracing::{error, info, warn};
use utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;

pub struct HealthMonitor {
    last_heartbeat: Arc<std::sync::atomic::AtomicU64>,
    is_healthy: Arc<std::sync::atomic::AtomicBool>,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            last_heartbeat: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            is_healthy: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub fn update_heartbeat(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_heartbeat
            .store(now, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn check_health(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last = self
            .last_heartbeat
            .load(std::sync::atomic::Ordering::SeqCst);

        if now - last > 30 {
            self.is_healthy
                .store(false, std::sync::atomic::Ordering::SeqCst);
            warn!(
                "Health check failed: no heartbeat for {} seconds",
                now - last
            );
            false
        } else {
            self.is_healthy
                .store(true, std::sync::atomic::Ordering::SeqCst);
            true
        }
    }
}

pub fn heartbeat() -> Result<()> {
    dotenv::dotenv().ok();
    info!("Starting relayer heartbeat...");

    // Initialize health monitor
    let health_monitor = Arc::new(HealthMonitor::new());
    let health_monitor_clone = Arc::clone(&health_monitor);

    // Load previous state
    info!("Looking for previous database...");
    match load_from_snapshot() {
        Ok(mut queue_manager) => {
            load_relayer_latest_state()?;
            queue_manager.process_queue();
        }
        Err(e) => {
            error!("Unable to start Relayer: {}", e);
            return Err(RelayerError::Database(e));
        }
    }

    thread::sleep(time::Duration::from_millis(100));

    // Start price feed
    thread::Builder::new()
        .name(String::from("BTC Binance Websocket Connection"))
        .spawn(move || {
            if let Err(e) = receive_btc_price() {
                error!("Price feed error: {}", e);
            }
        })
        .map_err(|e| {
            RelayerError::ThreadPool(format!("Failed to spawn price feed thread: {}", e))
        })?;

    // Initialize price
    initialize_price()?;

    // Start price check thread
    start_price_check_thread(health_monitor_clone)?;

    // Start scheduler
    start_scheduler()?;

    // Start RPC server
    start_rpc_server()?;

    // Start command receiver
    start_command_receiver()?;

    info!("Initialization complete");
    Ok(())
}

fn initialize_price() -> Result<()> {
    let mut local_storage = LOCALDB
        .lock()
        .map_err(|e| RelayerError::ThreadPool(format!("Failed to lock local storage: {}", e)))?;

    let current_price = match local_storage.get("Latest_Price") {
        Some(price) => *price,
        None => {
            warn!("No latest price available, waiting...");
            return Ok(());
        }
    };

    local_storage.insert("CurrentPrice", current_price);
    Ok(())
}

fn start_price_check_thread(health_monitor: Arc<HealthMonitor>) -> Result<()> {
    thread::Builder::new()
        .name(String::from("price_check_and_update"))
        .spawn(move || loop {
            thread::sleep(time::Duration::from_millis(1000));
            if let Err(e) = price_check_and_update() {
                error!("Price check error: {}", e);
            }
            health_monitor.update_heartbeat();
        })
        .map_err(|e| {
            RelayerError::ThreadPool(format!("Failed to spawn price check thread: {}", e))
        })?;

    Ok(())
}

fn start_scheduler() -> Result<()> {
    thread::Builder::new()
        .name(String::from("heartbeat scheduler"))
        .spawn(move || {
            let mut scheduler = Scheduler::with_tz(chrono::Utc);

            scheduler.every(1.hour()).run(move || {
                if get_relayer_status() {
                    if let Err(e) = updatefundingrate_localdb(1.0) {
                        error!("Failed to update funding rate: {}", e);
                    }
                }
            });

            scheduler.every(15.minute()).run(move || {
                if let Err(e) = snapshot() {
                    error!("Failed to create snapshot: {}", e);
                }
            });

            let thread_handle = scheduler.watch_thread(time::Duration::from_millis(1000));
            loop {
                thread::sleep(time::Duration::from_millis(100000000));
            }
        })
        .map_err(|e| {
            RelayerError::ThreadPool(format!("Failed to spawn scheduler thread: {}", e))
        })?;

    Ok(())
}

fn start_rpc_server() -> Result<()> {
    thread::Builder::new()
        .name(String::from("json-RPC startserver"))
        .spawn(move || {
            if let Err(e) = startserver() {
                error!("RPC server error: {}", e);
            }
        })
        .map_err(|e| {
            RelayerError::ThreadPool(format!("Failed to spawn RPC server thread: {}", e))
        })?;

    Ok(())
}

fn start_command_receiver() -> Result<()> {
    thread::Builder::new()
        .name(String::from("client_cmd_receiver"))
        .spawn(move || {
            thread::sleep(time::Duration::from_millis(10000));
            if let Err(e) = client_cmd_receiver() {
                error!("Command receiver error: {}", e);
            }
        })
        .map_err(|e| {
            RelayerError::ThreadPool(format!("Failed to spawn command receiver thread: {}", e))
        })?;

    Ok(())
}

pub fn price_check_and_update() -> Result<()> {
    let current_time = SystemTime::now();

    let mut local_storage = LOCALDB
        .lock()
        .map_err(|e| RelayerError::ThreadPool(format!("Failed to lock local storage: {}", e)))?;

    let current_price = match local_storage.get("Latest_Price") {
        Some(price) => *price,
        None => {
            warn!("No latest price available");
            return Ok(());
        }
    };

    if !get_relayer_status() {
        warn!("Relayer is not healthy, skipping price update");
        return Ok(());
    }

    // Update price
    update_price(current_price, &current_time)?;

    // Process orders
    process_pending_orders(current_price)?;
    process_liquidation_orders(current_price)?;
    process_settling_orders(current_price)?;

    Ok(())
}

fn update_price(price: f64, timestamp: &SystemTime) -> Result<()> {
    let rounded_price = price.round();

    Event::new(
        Event::CurrentPriceUpdate(price, iso8601(timestamp)),
        "insert_CurrentPrice".to_string(),
        TRADERORDER_EVENT_LOG.clone().to_string(),
    )?;

    set_localdb("CurrentPrice", rounded_price);
    Ok(())
}

fn process_pending_orders(current_price: f64) -> Result<()> {
    let thread_pool = THREADPOOL_PRICE_CHECK_PENDING_ORDER
        .lock()
        .map_err(|e| RelayerError::ThreadPool(format!("Failed to lock thread pool: {}", e)))?;

    thread_pool.execute(move || {
        if let Err(e) = check_pending_limit_order_on_price_ticker_update_localdb(current_price) {
            error!("Failed to process pending orders: {}", e);
        }
    })?;

    Ok(())
}

fn process_liquidation_orders(current_price: f64) -> Result<()> {
    let thread_pool = THREADPOOL_PRICE_CHECK_LIQUIDATION
        .lock()
        .map_err(|e| RelayerError::ThreadPool(format!("Failed to lock thread pool: {}", e)))?;

    thread_pool.execute(move || {
        if let Err(e) = check_liquidating_orders_on_price_ticker_update_localdb(current_price) {
            error!("Failed to process liquidation orders: {}", e);
        }
    })?;

    Ok(())
}

fn process_settling_orders(current_price: f64) -> Result<()> {
    let thread_pool = THREADPOOL_PRICE_CHECK_SETTLE_PENDING
        .lock()
        .map_err(|e| RelayerError::ThreadPool(format!("Failed to lock thread pool: {}", e)))?;

    thread_pool.execute(move || {
        if let Err(e) = check_settling_limit_order_on_price_ticker_update_localdb(current_price) {
            error!("Failed to process settling orders: {}", e);
        }
    })?;

    Ok(())
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
        // println!(
        //     "long:{:#?},\n Short:{:#?}",
        //     orderid_list_long, orderid_list_short
        // );

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
        // println!(
        //     "long:{:#?},\n Short:{:#?}",
        //     orderid_list_long, orderid_list_short
        // );

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
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
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
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        // println!(
        //     "long:{:#?},\n Short:{:#?}",
        //     orderid_list_long, orderid_list_short
        // );

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
    let current_price = get_localdb("CurrentPrice");
    let fee = get_localdb("Fee");
    let position_size_log = POSITION_SIZE_LOG.lock().unwrap();
    let totalshort = position_size_log.total_short_positionsize.clone();
    let totallong = position_size_log.total_long_positionsize.clone();
    let allpositionsize = position_size_log.totalpositionsize.clone();
    drop(position_size_log);
    println!(
        "totalshort:{}, totallong:{}, allpositionsize:{}",
        totalshort, totallong, allpositionsize
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
        fundingrate = fundingrate * (-1.0);
    }
    // comment below code and add kafka msg producer for fl=unding rate
    // updatefundingrateindb(fundingrate.clone(), current_price, current_time);
    println!("funding cycle processing...");
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
            hashmap.insert(String::from("Fee"), Some(fee.to_string()));
            hashmap
        },
    };
    // get_and_update_all_orders_on_funding_cycle(current_price, fundingrate.clone());
    fundingcycle(current_price, fundingrate, fee, current_time, meta);
    println!("fundingrate:{}", fundingrate);
}
use stopwatch::Stopwatch;
pub fn fundingcycle(
    current_price: f64,
    fundingrate: f64,
    fee: f64,
    current_time: std::time::SystemTime,
    metadata: Meta,
) {
    // let mut orderdetails_array: Vec<Arc<RwLock<TraderOrder>>> = Vec::new();
    let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
    let orderdetails_array = trader_order_db.getall_mut();
    drop(trader_order_db);

    let length = orderdetails_array.len();
    println!("length : {}", length);
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
        // println!("funding test 1");
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
    }
    println!("funding cycle took {:#?}", sw.elapsed());
}

pub fn updatechangesineachordertxonfundingratechange_localdb(
    order: Arc<RwLock<TraderOrder>>,
    fundingratechange: f64,
    current_price: f64,
    fee: f64,
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
                        println!("Error in line heartbeat.rs 607 : {:?}", verification_error);
                    }
                },
                Err(arg) => {
                    println!("Error in line heartbeat.rs 612 : {:?}", arg);
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
                        (funding_payment * -1.0),
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
