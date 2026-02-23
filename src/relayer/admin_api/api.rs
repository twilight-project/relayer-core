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
use std::collections::HashMap;
use stopwatch::Stopwatch;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
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
                        println!("\n TRADER_LP_LONG : {:#?}", trader_lp_long.sorted_order);
                        drop(trader_lp_long);
                    }
                    8 => {
                        let trader_lp_short = TRADER_LP_SHORT.lock().unwrap();
                        println!("\n TRADER_LP_SHORT : {:#?}", trader_lp_short.len);
                        println!("\n TRADER_LP_SHORT : {:#?}", trader_lp_short.sorted_order);
                        drop(trader_lp_short);
                    }
                    9 => {
                        let trader_lp_long = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
                        println!("\n TRADER_LIMIT_CLOSE_LONG : {:#?}", trader_lp_long);
                        drop(trader_lp_long);
                    }
                    10 => {
                        let trader_lp_long = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
                        println!("\n TRADER_LIMIT_CLOSE_SHORT : {:#?}", trader_lp_long);
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

    /*****************Risk Engine Stats */
    io.add_method_with_meta("GetRiskStats", move |_params: Params, _meta: Meta| async move {
        let pool_equity_btc = {
            let pool = LEND_POOL_DB.lock().unwrap();
            pool.total_locked_value
        };
        let mark_price = get_localdb("CurrentPrice");
        let stats = get_market_stats(pool_equity_btc, mark_price);
        Ok(serde_json::to_value(&stats).unwrap())
    });

    /*****************Set Market Halt */
    io.add_method_with_meta("SetMarketHalt", move |params: Params, _meta: Meta| async move {
        match params.parse::<SetMarketHaltRequest>() {
            Ok(req) => {
                let mut pending_cancelled: u64 = 0;
                let mut settling_cancelled: u64 = 0;
                let mut funding_paused = false;
                let mut price_feed_paused = false;
                let mut exit_initiated = false;
                let mut snapshot_taken = false;

                if req.enabled {
                    // 1. Set halt flag
                    RiskState::set_manual_halt(true);
                    crate::log_heartbeat!(warn, "RISK_ENGINE: Manual halt ENABLED");

                    // 2. Pause funding if requested
                    if req.pause_funding.unwrap_or(false) {
                        RiskState::set_pause_funding(true);
                        funding_paused = true;
                        crate::log_heartbeat!(warn, "RISK_ENGINE: Funding paused");
                    }

                    // 3. Pause price feed if requested
                    if req.pause_price_feed.unwrap_or(false) {
                        RiskState::set_pause_price_feed(true);
                        price_feed_paused = true;
                        crate::log_heartbeat!(warn, "RISK_ENGINE: Price feed paused");
                    }

                    // 4. Cancel pending limit orders if requested (background thread)
                    if req.cancel_pending_limit_orders.unwrap_or(false) {
                        // Collect UUIDs from the sorted sets (much faster than iterating all orders)
                        let open_long = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
                        let open_short = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
                        let mut pending_uuids: Vec<uuid::Uuid> = open_long.sorted_order
                            .iter()
                            .map(|(uuid, _)| *uuid)
                            .collect();
                        pending_uuids.extend(open_short.sorted_order.iter().map(|(uuid, _)| *uuid));
                        drop(open_long);
                        drop(open_short);

                        pending_cancelled = pending_uuids.len() as u64;

                        if pending_cancelled > 0 {
                            crate::log_heartbeat!(
                                warn,
                                "RISK_ENGINE: Scheduling cancellation of {} pending limit orders",
                                pending_cancelled
                            );
                            std::thread::Builder
                                ::new()
                                .name(String::from("halt_cancel_pending_limits"))
                                .spawn(move || {
                                    let mut cancelled_count: u64 = 0;
                                    let mut metadata = HashMap::new();
                                    metadata.insert(
                                        String::from("reason"),
                                        Some("Halt".to_string())
                                    );
                                    let meta = commands::Meta { metadata: metadata };
                                    for uuid in &pending_uuids {
                                        // Acquire TRADER_ORDER_DB, get the Arc, then drop the lock
                                        // before calling cancelorder_localdb to avoid deadlock
                                        // (cancelorder_localdb locks TRADER_LIMIT_OPEN_LONG/SHORT internally)
                                        let order_arc = {
                                            let mut trader_order_db =
                                                TRADER_ORDER_DB.lock().unwrap();
                                            match trader_order_db.get_mut(*uuid) {
                                                Ok(arc) => arc,
                                                Err(_) => {
                                                    continue;
                                                }
                                            }
                                        }; // TRADER_ORDER_DB lock dropped here

                                        let mut order = order_arc.write().unwrap();
                                        if
                                            order.order_status == OrderStatus::PENDING &&
                                            order.order_type == OrderType::LIMIT
                                        {
                                            let (cancelled, _) = order.cancelorder_localdb();
                                            let order_clone = order.clone();
                                            drop(order);
                                            if cancelled {
                                                cancelled_count += 1;
                                                let command = RpcCommand::CancelTraderOrder(
                                                    CancelTraderOrder {
                                                        account_id: order_clone.account_id.clone(),
                                                        uuid: order_clone.uuid.clone(),
                                                        order_type: OrderType::LIMIT,
                                                        order_status: OrderStatus::CANCELLED,
                                                    },
                                                    meta.clone(),
                                                    ZkosHexString::new(),
                                                    RequestID::new()
                                                );
                                                // Re-acquire TRADER_ORDER_DB for remove
                                                let mut trader_order_db =
                                                    TRADER_ORDER_DB.lock().unwrap();
                                                let _ = trader_order_db.remove(
                                                    order_clone.clone(),
                                                    command
                                                );
                                                drop(trader_order_db);
                                                Event::new(
                                                    Event::TxHash(
                                                        order_clone.uuid,
                                                        order_clone.account_id,
                                                        "Order successfully cancelled !!".to_string(),
                                                        order_clone.order_type,
                                                        OrderStatus::CANCELLED,
                                                        ServerTime::now().epoch,
                                                        None,
                                                        RequestID::new().to_string()
                                                    ),
                                                    format!(
                                                        "tx_hash_result-{:?}",
                                                        "Order successfully cancelled !!".to_string()
                                                    ),
                                                    CORE_EVENT_LOG.clone().to_string()
                                                );
                                            }
                                        }
                                    }

                                    crate::log_heartbeat!(
                                        warn,
                                        "RISK_ENGINE: Cancelled {} pending limit orders (background)",
                                        cancelled_count
                                    );
                                })
                                .unwrap();
                        }
                    }

                    // 5. Cancel settling limit orders if requested (background thread)
                    if req.cancel_settling_limit_orders.unwrap_or(false) {
                        // Count entries before spawning thread for the response
                        let close_long_count = TRADER_LIMIT_CLOSE_LONG.lock().unwrap().len as u64;
                        let close_short_count = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap().len as u64;
                        let sltp_long_count = TRADER_SLTP_CLOSE_LONG.lock().unwrap().len as u64;
                        let sltp_short_count = TRADER_SLTP_CLOSE_SHORT.lock().unwrap().len as u64;
                        settling_cancelled =
                            close_long_count +
                            close_short_count +
                            sltp_long_count +
                            sltp_short_count;

                        if settling_cancelled > 0 {
                            crate::log_heartbeat!(
                                warn,
                                "RISK_ENGINE: Scheduling clearing of {} settling limit order entries",
                                settling_cancelled
                            );
                            std::thread::Builder
                                ::new()
                                .name(String::from("halt_cancel_settling_limits"))
                                .spawn(move || {
                                    // Close limit orders
                                    let mut close_long = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
                                    let mut close_short = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();

                                    for (uuid, _) in &close_long.sorted_order {
                                        Event::new(
                                            Event::SortedSetDBUpdate(
                                                SortedSetCommand::RemoveCloseLimitPrice(
                                                    *uuid,
                                                    PositionType::LONG
                                                )
                                            ),
                                            format!("HaltRemoveCloseLimitPrice-{}", uuid),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }
                                    for (uuid, _) in &close_short.sorted_order {
                                        Event::new(
                                            Event::SortedSetDBUpdate(
                                                SortedSetCommand::RemoveCloseLimitPrice(
                                                    *uuid,
                                                    PositionType::SHORT
                                                )
                                            ),
                                            format!("HaltRemoveCloseLimitPrice-{}", uuid),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }

                                    *close_long = SortedSet::new();
                                    *close_short = SortedSet::new();
                                    drop(close_long);
                                    drop(close_short);

                                    // SLTP close limit orders
                                    let mut sltp_long = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
                                    let mut sltp_short = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();

                                    for (uuid, _) in &sltp_long.sorted_order {
                                        Event::new(
                                            Event::SortedSetDBUpdate(
                                                SortedSetCommand::RemoveStopLossCloseLIMITPrice(
                                                    *uuid,
                                                    PositionType::LONG
                                                )
                                            ),
                                            format!("HaltRemoveSLTPCloseLIMITPrice-{}", uuid),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }
                                    for (uuid, _) in &sltp_short.sorted_order {
                                        Event::new(
                                            Event::SortedSetDBUpdate(
                                                SortedSetCommand::RemoveStopLossCloseLIMITPrice(
                                                    *uuid,
                                                    PositionType::SHORT
                                                )
                                            ),
                                            format!("HaltRemoveSLTPCloseLIMITPrice-{}", uuid),
                                            CORE_EVENT_LOG.clone().to_string()
                                        );
                                    }

                                    *sltp_long = SortedSet::new();
                                    *sltp_short = SortedSet::new();
                                    drop(sltp_long);
                                    drop(sltp_short);

                                    crate::log_heartbeat!(
                                        warn,
                                        "RISK_ENGINE: Cleared settling limit order entries (background)"
                                    );
                                })
                                .unwrap();
                        }
                    }

                    // 6. Exit relayer if requested
                    if req.exit_relayer.unwrap_or(false) {
                        exit_initiated = true;

                        // Force pause funding + price feed regardless of individual flags
                        if !funding_paused {
                            RiskState::set_pause_funding(true);
                            funding_paused = true;
                        }
                        if !price_feed_paused {
                            RiskState::set_pause_price_feed(true);
                            price_feed_paused = true;
                        }

                        // Stop all service loops
                        set_relayer_status(false);
                        crate::log_heartbeat!(
                            warn,
                            "RISK_ENGINE: Relayer set to idle (exit_relayer)"
                        );

                        // Take final snapshot
                        match snapshot() {
                            Ok(_) => {
                                snapshot_taken = true;
                                crate::log_heartbeat!(warn, "RISK_ENGINE: Final snapshot taken");
                            }
                            Err(e) => {
                                crate::log_heartbeat!(
                                    error,
                                    "RISK_ENGINE: Failed to take snapshot: {:?}",
                                    e
                                );
                            }
                        }
                    }
                } else {
                    // Disabling halt
                    RiskState::set_manual_halt(false);
                    crate::log_heartbeat!(warn, "RISK_ENGINE: Manual halt DISABLED");

                    // Resume funding if explicitly requested
                    if req.pause_funding == Some(false) {
                        RiskState::set_pause_funding(false);
                        crate::log_heartbeat!(warn, "RISK_ENGINE: Funding resumed");
                    }

                    // Resume price feed if explicitly requested
                    if req.pause_price_feed == Some(false) {
                        RiskState::set_pause_price_feed(false);
                        crate::log_heartbeat!(warn, "RISK_ENGINE: Price feed resumed");
                    }

                    // Read current state for response
                    let state = RISK_ENGINE_STATE.lock().unwrap();
                    funding_paused = state.pause_funding;
                    price_feed_paused = state.pause_price_feed;
                    drop(state);
                }

                let response =
                    serde_json::json!({
                    "halt_enabled": req.enabled,
                    "pending_orders_cancelled": pending_cancelled,
                    "settling_orders_cancelled": settling_cancelled,
                    "funding_paused": funding_paused,
                    "price_feed_paused": price_feed_paused,
                    "exit_initiated": exit_initiated,
                    "snapshot_taken": snapshot_taken,
                });
                Ok(response)
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(
                    format!(
                        "Invalid parameters, {:?}. Expected: {{ \"enabled\": bool, \"cancel_pending_limit_orders\": bool?, \"cancel_settling_limit_orders\": bool?, \"pause_funding\": bool?, \"pause_price_feed\": bool?, \"exit_relayer\": bool? }}",
                        args
                    )
                );
                Err(err)
            }
        }
    });

    /*****************Set Market Close Only */
    io.add_method_with_meta("SetMarketCloseOnly", move |params: Params, _meta: Meta| async move {
        match params.parse::<SetMarketFlag>() {
            Ok(value) => {
                RiskState::set_manual_close_only(value.enabled);
                crate::log_heartbeat!(
                    warn,
                    "RISK_ENGINE: Manual close-only set to {}",
                    value.enabled
                );
                Ok(
                    serde_json
                        ::to_value(format!("Manual close-only set to {}", value.enabled))
                        .unwrap()
                )
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(
                    format!("Invalid parameters, {:?}. Expected: {{ \"enabled\": bool }}", args)
                );
                Err(err)
            }
        }
    });

    /*****************Update Risk Params */
    io.add_method_with_meta("UpdateRiskParams", move |params: Params, _meta: Meta| async move {
        match params.parse::<UpdateRiskParamsRequest>() {
            Ok(value) => {
                let current_params = RISK_PARAMS.lock().unwrap().clone();
                let new_params = RiskParams {
                    max_oi_mult: value.max_oi_mult.unwrap_or(current_params.max_oi_mult),
                    max_net_mult: value.max_net_mult.unwrap_or(current_params.max_net_mult),
                    max_position_pct: value.max_position_pct.unwrap_or(
                        current_params.max_position_pct
                    ),
                    min_position_btc: value.min_position_btc.unwrap_or(
                        current_params.min_position_btc
                    ),
                    max_leverage: value.max_leverage.unwrap_or(current_params.max_leverage),
                    mm_ratio: value.mm_ratio.unwrap_or(current_params.mm_ratio),
                };
                RiskState::update_risk_params(new_params.clone());
                crate::log_heartbeat!(
                    warn,
                    "RISK_ENGINE: Risk params updated: max_oi_mult={}, max_net_mult={}, max_position_pct={}, min_position_btc={}, max_leverage={}, mm_ratio={}",
                    new_params.max_oi_mult,
                    new_params.max_net_mult,
                    new_params.max_position_pct,
                    new_params.min_position_btc,
                    new_params.max_leverage,
                    new_params.mm_ratio
                );
                Ok(serde_json::to_value(&new_params).unwrap())
            }
            Err(args) => {
                let err = JsonRpcError::invalid_params(
                    format!(
                        "Invalid parameters, {:?}. Expected: {{ \"max_oi_mult\": f64?, \"max_net_mult\": f64?, \"max_position_pct\": f64?, \"min_position_btc\": f64?, \"max_leverage\": f64?, \"mm_ratio\": f64? }}",
                        args
                    )
                );
                Err(err)
            }
        }
    });

    /*****************Get Risk Params */
    io.add_method_with_meta("GetRiskParams", move |_params: Params, _meta: Meta| async move {
        let params = RISK_PARAMS.lock().unwrap().clone();
        Ok(serde_json::to_value(&params).unwrap())
    });

    /*****************Recalculate Risk State */
    io.add_method_with_meta("RecalculateRiskState", move |_params: Params, _meta: Meta| async move {
        std::thread::Builder::new()
            .name(String::from("recalculate_risk_state"))
            .spawn(move || {
                let trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                let mut total_long_btc: f64 = 0.0;
                let mut total_short_btc: f64 = 0.0;
                let mut total_pending_long_btc: f64 = 0.0;
                let mut total_pending_short_btc: f64 = 0.0;

                for (_uuid, order_arc) in trader_order_db.ordertable.iter() {
                    let order = order_arc.read().unwrap();
                    let entry_value = entryvalue(order.initial_margin, order.leverage);
                    match (&order.order_status, &order.position_type) {
                        (OrderStatus::FILLED, PositionType::LONG) => {
                            total_long_btc += entry_value;
                        }
                        (OrderStatus::FILLED, PositionType::SHORT) => {
                            total_short_btc += entry_value;
                        }
                        (OrderStatus::PENDING, PositionType::LONG) => {
                            total_pending_long_btc += entry_value;
                        }
                        (OrderStatus::PENDING, PositionType::SHORT) => {
                            total_pending_short_btc += entry_value;
                        }
                        _ => {}
                    }
                }
                drop(trader_order_db);

                crate::log_heartbeat!(
                    warn,
                    "RISK_ENGINE: Recalculated exposure - filled_long={}, filled_short={}, pending_long={}, pending_short={}",
                    total_long_btc,
                    total_short_btc,
                    total_pending_long_btc,
                    total_pending_short_btc
                );

                RiskState::recalculate_exposure(
                    total_long_btc,
                    total_short_btc,
                    total_pending_long_btc,
                    total_pending_short_btc,
                );
            })
            .unwrap();

        Ok(serde_json::to_value("Risk state recalculation started in background").unwrap())
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
