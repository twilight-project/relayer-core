use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use twilight_relayer_sdk::utxo_in_memory;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use twilight_relayer_sdk::zkvm;
use uuid::Uuid;

// ── Generic OrderDBSnapShot ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShot<T> {
    pub ordertable: HashMap<Uuid, T>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}

pub type OrderDBSnapShotTO = OrderDBSnapShot<TraderOrder>;
pub type OrderDBSnapShotLO = OrderDBSnapShot<LendOrder>;

impl<T> OrderDBSnapShot<T> {
    pub fn new() -> Self {
        OrderDBSnapShot {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            zkos_msg: HashMap::new(),
            hash: HashSet::new(),
        }
    }
    pub fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    pub fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
}

// ── V11 snapshot format (current — QueueState with SL/TP settle queues) ──────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDB {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sl_close_long_sortedset_db: SortedSet,
    pub sl_close_short_sortedset_db: SortedSet,
    pub tp_close_long_sortedset_db: SortedSet,
    pub tp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskState,
    pub risk_params: Option<RiskParams>,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub(crate) queue_manager: QueueState,
    pub group_name: String,
}

/// Current snapshot version written by this binary.
/// V11 = QueueState with SL/TP settle queues.
pub(crate) const SNAPSHOT_FORMAT_VERSION: u32 = 11;

impl SnapshotDB {
    pub(crate) fn new() -> Self {
        SnapshotDB {
            orderdb_traderorder: OrderDBSnapShotTO::new(),
            orderdb_lendorder: OrderDBSnapShotLO::new(),
            lendpool_database: LendPool::default(),
            liquidation_long_sortedset_db: SortedSet::new(),
            liquidation_short_sortedset_db: SortedSet::new(),
            open_long_sortedset_db: SortedSet::new(),
            open_short_sortedset_db: SortedSet::new(),
            close_long_sortedset_db: SortedSet::new(),
            close_short_sortedset_db: SortedSet::new(),
            sl_close_long_sortedset_db: SortedSet::new(),
            sl_close_short_sortedset_db: SortedSet::new(),
            tp_close_long_sortedset_db: SortedSet::new(),
            tp_close_short_sortedset_db: SortedSet::new(),
            position_size_log: PositionSizeLog::new(),
            risk_state: RiskState::new(),
            risk_params: Some(RiskParams::from_env()),
            localdb_hashmap: {
                let mut hashmap = HashMap::new();

                let filled_on_market = std::env::var("FILLED_ON_MARKET")
                    .unwrap_or("0.04".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.04);
                let filled_on_limit = std::env::var("FILLED_ON_LIMIT")
                    .unwrap_or("0.02".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.02);
                let settled_on_market = std::env::var("SETTLED_ON_MARKET")
                    .unwrap_or("0.04".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.04);
                let settled_on_limit = std::env::var("SETTLED_ON_LIMIT")
                    .unwrap_or("0.02".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.02);

                hashmap.insert(FeeType::FilledOnMarket.into(), filled_on_market);
                hashmap.insert(FeeType::FilledOnLimit.into(), filled_on_limit);
                hashmap.insert(FeeType::SettledOnMarket.into(), settled_on_market);
                hashmap.insert(FeeType::SettledOnLimit.into(), settled_on_limit);
                let event_time = systemtime_to_utc();
                let _ = Event::send_event_to_kafka_queue(
                    Event::FeeUpdate(
                        RelayerCommand::UpdateFees(
                            filled_on_market,
                            filled_on_limit,
                            settled_on_market,
                            settled_on_limit,
                        ),
                        event_time.clone(),
                    ),
                    CORE_EVENT_LOG.clone().to_string(),
                    format!("FeeUpdate-{}", event_time),
                );
                // Emit initial risk params event
                let initial_risk_params = RiskParams::from_env();
                let _ = Event::send_event_to_kafka_queue(
                    Event::RiskParamsUpdate(initial_risk_params),
                    CORE_EVENT_LOG.clone().to_string(),
                    format!("RiskParamsUpdate-{}", event_time),
                );
                hashmap
            },
            event_offset_partition: (0, 0),
            event_timestamp: ServerTime::now().epoch,
            output_hex_storage:
                utxo_in_memory::db::LocalStorage::<Option<zkvm::zkos_types::Output>>::new(1),
            queue_manager: QueueState::new(),
            group_name: format!(
                "{}-{}-{}",
                *RELAYER_SNAPSHOT_FILE_LOCATION,
                *SNAPSHOT_VERSION,
                Uuid::new_v4()
            ),
        }
    }

    pub(crate) fn print_status(&self) {
        if self.orderdb_traderorder.sequence > 0 {
            crate::log_heartbeat!(debug, "TraderOrder Database Loaded ....");
        } else {
            crate::log_heartbeat!(
                debug,
                "No old TraderOrder Database found ....\nCreating new TraderOrder_database"
            );
        }
        if self.orderdb_lendorder.sequence > 0 {
            crate::log_heartbeat!(debug, "LendOrder Database Loaded ....");
        } else {
            crate::log_heartbeat!(
                debug,
                "No old LendOrder Database found ....\nCreating new LendOrder_database"
            );
        }
        if self.lendpool_database.aggrigate_log_sequence > 0 {
            crate::log_heartbeat!(debug, "LendPool Database Loaded ....");
        } else {
            crate::log_heartbeat!(
                debug,
                "No old LendPool Database found ....\nCreating new LendPool_database"
            );
        }
    }

    pub(crate) fn log_metadata(&self) {
        crate::log_heartbeat!(
            info,
            "Snapshot : version={}, group_name={}, {} trader orders, {} lend orders, \
             liq_long={}, liq_short={}, open_long={}, open_short={}, \
             close_long={}, close_short={}, sl_long={}, sl_short={}, tp_long={}, tp_short={}, \
             offset=({}, {}), timestamp={}",
            SNAPSHOT_FORMAT_VERSION,
            self.group_name,
            self.orderdb_traderorder.ordertable.len(),
            self.orderdb_lendorder.ordertable.len(),
            self.liquidation_long_sortedset_db.len,
            self.liquidation_short_sortedset_db.len,
            self.open_long_sortedset_db.len,
            self.open_short_sortedset_db.len,
            self.close_long_sortedset_db.len,
            self.close_short_sortedset_db.len,
            self.sl_close_long_sortedset_db.len,
            self.sl_close_short_sortedset_db.len,
            self.tp_close_long_sortedset_db.len,
            self.tp_close_short_sortedset_db.len,
            self.event_offset_partition.0,
            self.event_offset_partition.1,
            self.event_timestamp
        );
    }
}

// ── SnapshotBuilder ──────────────────────────────────────────────────────────

pub(crate) struct SnapshotBuilder {
    pub(crate) orderdb_traderorder: OrderDBSnapShotTO,
    pub(crate) orderdb_lendorder: OrderDBSnapShotLO,
    pub(crate) lendpool_database: LendPool,
    pub(crate) liquidation_long_sortedset_db: SortedSet,
    pub(crate) liquidation_short_sortedset_db: SortedSet,
    pub(crate) open_long_sortedset_db: SortedSet,
    pub(crate) open_short_sortedset_db: SortedSet,
    pub(crate) close_long_sortedset_db: SortedSet,
    pub(crate) close_short_sortedset_db: SortedSet,
    pub(crate) sl_close_long_sortedset_db: SortedSet,
    pub(crate) sl_close_short_sortedset_db: SortedSet,
    pub(crate) tp_close_long_sortedset_db: SortedSet,
    pub(crate) tp_close_short_sortedset_db: SortedSet,
    pub(crate) position_size_log: PositionSizeLog,
    pub(crate) risk_state: RiskState,
    pub(crate) risk_params: Option<RiskParams>,
    pub(crate) localdb_hashmap: HashMap<String, f64>,
    pub(crate) event_offset_partition: (i32, i64),
    pub(crate) event_timestamp: String,
    pub(crate) output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub(crate) queue_manager: QueueState,
    pub(crate) group_name: String,
}

impl SnapshotBuilder {
    pub(crate) fn from_snapshot(snapshot_db: SnapshotDB, event_timestamp: String) -> Self {
        SnapshotBuilder {
            orderdb_traderorder: snapshot_db.orderdb_traderorder,
            orderdb_lendorder: snapshot_db.orderdb_lendorder,
            lendpool_database: snapshot_db.lendpool_database,
            liquidation_long_sortedset_db: snapshot_db.liquidation_long_sortedset_db,
            liquidation_short_sortedset_db: snapshot_db.liquidation_short_sortedset_db,
            open_long_sortedset_db: snapshot_db.open_long_sortedset_db,
            open_short_sortedset_db: snapshot_db.open_short_sortedset_db,
            close_long_sortedset_db: snapshot_db.close_long_sortedset_db,
            close_short_sortedset_db: snapshot_db.close_short_sortedset_db,
            sl_close_long_sortedset_db: snapshot_db.sl_close_long_sortedset_db,
            sl_close_short_sortedset_db: snapshot_db.sl_close_short_sortedset_db,
            tp_close_long_sortedset_db: snapshot_db.tp_close_long_sortedset_db,
            tp_close_short_sortedset_db: snapshot_db.tp_close_short_sortedset_db,
            position_size_log: snapshot_db.position_size_log,
            risk_state: snapshot_db.risk_state,
            risk_params: snapshot_db.risk_params,
            localdb_hashmap: snapshot_db.localdb_hashmap,
            event_offset_partition: snapshot_db.event_offset_partition,
            event_timestamp,
            output_hex_storage: snapshot_db.output_hex_storage,
            queue_manager: snapshot_db.queue_manager,
            group_name: snapshot_db.group_name,
        }
    }

    pub(crate) fn build(mut self) -> SnapshotDB {
        if self.lendpool_database.aggrigate_log_sequence == 0 {
            self.lendpool_database = LendPool::new();
        }
        self.queue_manager
            .bulk_remove_queue(&self.orderdb_traderorder);

        SnapshotDB {
            orderdb_traderorder: self.orderdb_traderorder,
            orderdb_lendorder: self.orderdb_lendorder,
            lendpool_database: self.lendpool_database,
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db,
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db,
            open_long_sortedset_db: self.open_long_sortedset_db,
            open_short_sortedset_db: self.open_short_sortedset_db,
            close_long_sortedset_db: self.close_long_sortedset_db,
            close_short_sortedset_db: self.close_short_sortedset_db,
            sl_close_long_sortedset_db: self.sl_close_long_sortedset_db,
            sl_close_short_sortedset_db: self.sl_close_short_sortedset_db,
            tp_close_long_sortedset_db: self.tp_close_long_sortedset_db,
            tp_close_short_sortedset_db: self.tp_close_short_sortedset_db,
            position_size_log: self.position_size_log,
            risk_state: self.risk_state,
            risk_params: self.risk_params,
            localdb_hashmap: self.localdb_hashmap,
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp,
            output_hex_storage: self.output_hex_storage,
            queue_manager: self.queue_manager,
            group_name: self.group_name,
        }
    }

    pub(crate) fn handle_event(&mut self, data: &EventLog) {
        match data.value.clone() {
            Event::TraderOrder(order, cmd, seq) => self.handle_trader_order(order, cmd, seq),
            Event::TraderOrderLimitUpdate(order, cmd, seq) => {
                self.handle_trader_order_limit_update(order, cmd, seq)
            }
            Event::TraderOrderUpdate(order, _cmd, seq) => {
                self.handle_trader_order_update(order, seq)
            }
            Event::TraderOrderFundingUpdate(order, _cmd) => {
                self.handle_trader_order_funding_update(order)
            }
            Event::TraderOrderLiquidation(order, _cmd, seq) => {
                self.handle_trader_order_liquidation(order, seq)
            }
            Event::Stop(timex) => {
                let _ = timex;
            }
            Event::LendOrder(order, cmd, seq) => self.handle_lend_order(order, cmd, seq),
            Event::PoolUpdate(_cmd, lend_pool, _seq) => {
                self.lendpool_database = lend_pool;
            }
            Event::FundingRateUpdate(funding_rate, _current_price, _time) => {
                self.localdb_hashmap
                    .insert("FundingRate".to_string(), funding_rate);
            }
            Event::FeeUpdate(cmd, _time) => self.handle_fee_update(cmd),
            Event::CurrentPriceUpdate(current_price, _time) => {
                self.handle_current_price_update(current_price)
            }
            Event::SortedSetDBUpdate(cmd, _time) => self.handle_sorted_set_update(cmd),
            Event::PositionSizeLogDBUpdate(_cmd, event) => {
                self.position_size_log = event;
            }
            Event::RiskEngineUpdate(_cmd, event) => {
                self.risk_state = event;
            }
            Event::RiskParamsUpdate(params) => {
                self.risk_params = Some(params);
            }
            Event::TxHash(data) => {
                self.handle_output_storage(data.order_id, data.order_type, data.order_status, data.output);
            }
            Event::TxHashUpdate(data) => {
                self.handle_output_storage(data.order_id, data.order_type, data.order_status, data.output);
            }
            Event::AdvanceStateQueue(_, _) => {}
        }
    }

    fn handle_output_storage(
        &mut self,
        orderid: Uuid,
        order_type: OrderType,
        order_status: OrderStatus,
        option_output: Option<String>,
    ) {
        match order_type {
            OrderType::LIMIT | OrderType::MARKET => match order_status {
                OrderStatus::FILLED => {
                    let uuid_to_byte = match bincode::serialize(&orderid) {
                        Ok(uuid_v_u8) => uuid_v_u8,
                        Err(_) => Vec::new(),
                    };
                    let output_memo_option = match option_output {
                        Some(output_hex_string) => match hex::decode(output_hex_string) {
                            Ok(output_byte) => match bincode::deserialize(&output_byte) {
                                Ok(output_memo) => Some(output_memo),
                                Err(_) => None,
                            },
                            Err(_) => None,
                        },
                        None => None,
                    };
                    if let Some(output_memo) = output_memo_option {
                        let _ = self
                            .output_hex_storage
                            .add(uuid_to_byte, Some(output_memo), 0);
                    }
                }
                OrderStatus::SETTLED | OrderStatus::CANCELLED | OrderStatus::LIQUIDATE => {
                    let uuid_to_byte = match bincode::serialize(&orderid) {
                        Ok(uuid_v_u8) => uuid_v_u8,
                        Err(_) => Vec::new(),
                    };
                    let _ = self.output_hex_storage.remove(uuid_to_byte, 0);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_trader_order(&mut self, order: TraderOrder, cmd: RpcCommand, seq: usize) {
        match cmd {
            RpcCommand::CreateTraderOrder(
                _rpc_request,
                _metadata,
                zkos_hex_string,
                _request_id,
            ) => {
                self.orderdb_traderorder
                    .ordertable
                    .insert(order.uuid, order.clone());
                if self.orderdb_traderorder.sequence < order.entry_sequence {
                    self.orderdb_traderorder.sequence = order.entry_sequence;
                }
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }

                match order.order_status {
                    OrderStatus::FILLED => match order.position_type {
                        PositionType::LONG => {
                            let _ = self
                                .liquidation_long_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = self
                                .liquidation_short_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                    },
                    OrderStatus::PENDING => match order.position_type {
                        PositionType::LONG => {
                            let _ = self
                                .open_long_sortedset_db
                                .add(order.uuid, (order.entryprice * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = self
                                .open_short_sortedset_db
                                .add(order.uuid, (order.entryprice * 10000.0) as i64);
                        }
                    },
                    _ => {}
                }
                self.orderdb_traderorder
                    .zkos_msg
                    .insert(order.uuid, zkos_hex_string);
                let _ = self.orderdb_traderorder.set_order_check(order.account_id);
            }
            RpcCommand::CancelTraderOrder(
                _rpc_request,
                _metadata,
                _zkos_hex_string,
                _request_id,
            ) => {
                let order_clone = order.clone();
                if self
                    .orderdb_traderorder
                    .ordertable
                    .contains_key(&order.uuid)
                {
                    self.orderdb_traderorder.ordertable.remove(&order.uuid);
                    let _ = self.orderdb_traderorder.zkos_msg.remove(&order.uuid);
                    let _ = self
                        .orderdb_traderorder
                        .remove_order_check(order.account_id);
                }
                if self.orderdb_traderorder.sequence < order_clone.entry_sequence {
                    self.orderdb_traderorder.sequence = order_clone.entry_sequence;
                }
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }
                self.queue_manager.remove_fill(&order.uuid);
            }
            RpcCommand::ExecuteTraderOrder(
                _rpc_request,
                _metadata,
                _zkos_hex_string,
                _request_id,
            ) => {
                if self
                    .orderdb_traderorder
                    .ordertable
                    .contains_key(&order.uuid)
                {
                    self.orderdb_traderorder.ordertable.remove(&order.uuid);
                    let _ = self.orderdb_traderorder.zkos_msg.remove(&order.uuid);
                    let _ = self
                        .orderdb_traderorder
                        .remove_order_check(order.account_id);
                }
                if self.orderdb_traderorder.sequence < order.entry_sequence {
                    self.orderdb_traderorder.sequence = order.entry_sequence;
                }
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }
                match order.order_status {
                    OrderStatus::SETTLED => match order.position_type {
                        PositionType::LONG => {
                            let _ = self.liquidation_long_sortedset_db.remove(order.uuid);
                        }
                        PositionType::SHORT => {
                            let _ = self.liquidation_short_sortedset_db.remove(order.uuid);
                        }
                    },
                    _ => {}
                }
                self.queue_manager.remove_settle(&order.uuid);
                self.queue_manager.remove_settle_sl(&order.uuid);
                self.queue_manager.remove_settle_tp(&order.uuid);
            }
            RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                _rpc_request,
                _metadata,
                _payment,
            ) => {
                let order_clone = order.clone();
                if self
                    .orderdb_traderorder
                    .ordertable
                    .contains_key(&order.uuid)
                {
                    self.orderdb_traderorder.ordertable.remove(&order.uuid);
                    let _ = self.orderdb_traderorder.zkos_msg.remove(&order.uuid);
                    let _ = self
                        .orderdb_traderorder
                        .remove_order_check(order.account_id);
                }
                if self.orderdb_traderorder.sequence < order_clone.entry_sequence {
                    self.orderdb_traderorder.sequence = order_clone.entry_sequence;
                }
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }
                self.queue_manager.remove_fill(&order.uuid);
                self.queue_manager.remove_settle(&order.uuid);
                self.queue_manager.remove_settle_sl(&order.uuid);
                self.queue_manager.remove_settle_tp(&order.uuid);
            }
            RpcCommand::CreateTraderOrderSlTp(_, _, _, _, _, _) => {}
            RpcCommand::CancelTraderOrderSlTp(_, _, _, _, _) => {}
            RpcCommand::ExecuteLendOrder(_, _, _, _) => {}
            RpcCommand::CreateLendOrder(_, _, _, _) => {}
            RpcCommand::ExecuteTraderOrderSlTp(_, _, _, _, _) => {}
        }
    }

    fn handle_trader_order_limit_update(
        &mut self,
        order: TraderOrder,
        cmd: RpcCommand,
        seq: usize,
    ) {
        match cmd {
            RpcCommand::ExecuteTraderOrder(
                _rpc_request,
                _metadata,
                zkos_hex_string,
                _request_id,
            ) => {
                self.orderdb_traderorder
                    .zkos_msg
                    .insert(order.uuid, zkos_hex_string);
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }
            }
            RpcCommand::ExecuteTraderOrderSlTp(
                _rpc_request,
                _option_sltp,
                _metadata,
                zkos_hex_string,
                _request_id,
            ) => {
                self.orderdb_traderorder
                    .zkos_msg
                    .insert(order.uuid, zkos_hex_string);
                if self.orderdb_traderorder.aggrigate_log_sequence < seq {
                    self.orderdb_traderorder.aggrigate_log_sequence = seq;
                }
            }
            _ => {}
        }
    }

    fn handle_trader_order_update(&mut self, order: TraderOrder, seq: usize) {
        self.orderdb_traderorder
            .ordertable
            .insert(order.uuid, order.clone());
        if self.orderdb_traderorder.sequence < order.entry_sequence {
            self.orderdb_traderorder.sequence = order.entry_sequence;
        }
        if self.orderdb_traderorder.aggrigate_log_sequence < seq {
            self.orderdb_traderorder.aggrigate_log_sequence = seq;
        }

        match order.order_status {
            OrderStatus::FILLED => match order.position_type {
                PositionType::LONG => {
                    let _ = self
                        .liquidation_long_sortedset_db
                        .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .liquidation_short_sortedset_db
                        .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                }
            },
            _ => {}
        }
        self.queue_manager.remove_fill(&order.uuid);
    }

    fn handle_trader_order_funding_update(&mut self, order: TraderOrder) {
        self.orderdb_traderorder
            .ordertable
            .insert(order.uuid, order.clone());

        match order.position_type {
            PositionType::LONG => {
                let _ = self
                    .liquidation_long_sortedset_db
                    .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
            }
            PositionType::SHORT => {
                let _ = self
                    .liquidation_short_sortedset_db
                    .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
            }
        }
    }

    fn handle_trader_order_liquidation(&mut self, order: TraderOrder, seq: usize) {
        if self
            .orderdb_traderorder
            .ordertable
            .contains_key(&order.uuid)
        {
            self.orderdb_traderorder.ordertable.remove(&order.uuid);
            let _ = self.orderdb_traderorder.zkos_msg.remove(&order.uuid);
            let _ = self
                .orderdb_traderorder
                .remove_order_check(order.account_id);
        }
        if self.orderdb_traderorder.sequence < order.entry_sequence {
            self.orderdb_traderorder.sequence = order.entry_sequence;
        }
        if self.orderdb_traderorder.aggrigate_log_sequence < seq {
            self.orderdb_traderorder.aggrigate_log_sequence = seq;
        }
        self.queue_manager.remove_liquidate(&order.uuid);
    }

    fn handle_lend_order(&mut self, order: LendOrder, cmd: RpcCommand, seq: usize) {
        match cmd {
            RpcCommand::CreateLendOrder(_rpc_request, _metadata, zkos_hex_string, _request_id) => {
                let order_clone = order.clone();
                self.orderdb_lendorder.ordertable.insert(order.uuid, order);
                if self.orderdb_lendorder.sequence < order_clone.entry_sequence {
                    self.orderdb_lendorder.sequence = order_clone.entry_sequence;
                }
                if self.orderdb_lendorder.aggrigate_log_sequence < seq {
                    self.orderdb_lendorder.aggrigate_log_sequence = seq;
                }
                self.orderdb_lendorder
                    .zkos_msg
                    .insert(order_clone.uuid, zkos_hex_string);
                let _ = self
                    .orderdb_traderorder
                    .set_order_check(order_clone.account_id);
            }
            RpcCommand::ExecuteLendOrder(
                _rpc_request,
                _metadata,
                _zkos_hex_string,
                _request_id,
            ) => {
                if self.orderdb_lendorder.ordertable.contains_key(&order.uuid) {
                    self.orderdb_lendorder.ordertable.remove(&order.uuid);
                    let _ = self.orderdb_lendorder.zkos_msg.remove(&order.uuid);
                    let _ = self
                        .orderdb_traderorder
                        .remove_order_check(order.account_id);
                }
                if self.orderdb_lendorder.aggrigate_log_sequence < seq {
                    self.orderdb_lendorder.aggrigate_log_sequence = seq;
                }
                if self.orderdb_lendorder.sequence < order.entry_sequence {
                    self.orderdb_lendorder.sequence = order.entry_sequence;
                }
            }
            _ => {}
        }
    }

    fn handle_fee_update(&mut self, cmd: RelayerCommand) {
        match cmd {
            RelayerCommand::UpdateFees(
                order_filled_on_market,
                order_filled_on_limit,
                order_settled_on_market,
                order_settled_on_limit,
            ) => {
                self.localdb_hashmap
                    .insert(FeeType::FilledOnMarket.into(), order_filled_on_market);
                self.localdb_hashmap
                    .insert(FeeType::FilledOnLimit.into(), order_filled_on_limit);
                self.localdb_hashmap
                    .insert(FeeType::SettledOnMarket.into(), order_settled_on_market);
                self.localdb_hashmap
                    .insert(FeeType::SettledOnLimit.into(), order_settled_on_limit);
            }
            _ => {}
        }
    }

    fn handle_current_price_update(&mut self, current_price: f64) {
        self.localdb_hashmap
            .insert("CurrentPrice".to_string(), current_price);

        self.queue_manager.bulk_insert_to_fill(
            &mut self.open_short_sortedset_db,
            &mut self.open_long_sortedset_db,
            current_price,
        );
        self.queue_manager.bulk_insert_to_liquidate(
            &mut self.liquidation_short_sortedset_db,
            &mut self.liquidation_long_sortedset_db,
            current_price,
        );
        self.queue_manager.bulk_insert_to_settle(
            &mut self.close_short_sortedset_db,
            &mut self.close_long_sortedset_db,
            current_price,
        );
        self.queue_manager.bulk_insert_to_settle_sl(
            &mut self.sl_close_short_sortedset_db,
            &mut self.sl_close_long_sortedset_db,
            current_price,
        );
        self.queue_manager.bulk_insert_to_settle_tp(
            &mut self.tp_close_short_sortedset_db,
            &mut self.tp_close_long_sortedset_db,
            current_price,
        );
    }

    fn handle_sorted_set_update(&mut self, cmd: SortedSetCommand) {
        match cmd {
            SortedSetCommand::AddOpenLimitPrice(order_id, entry_price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .open_long_sortedset_db
                            .add(order_id, (entry_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .open_short_sortedset_db
                            .add(order_id, (entry_price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::AddLiquidationPrice(order_id, liquidation_price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .liquidation_long_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .liquidation_short_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::AddCloseLimitPrice(order_id, execution_price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .close_long_sortedset_db
                            .add(order_id, (execution_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .close_short_sortedset_db
                            .add(order_id, (execution_price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::RemoveOpenLimitPrice(order_id, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self.open_long_sortedset_db.remove(order_id);
                    }
                    PositionType::SHORT => {
                        let _ = self.open_short_sortedset_db.remove(order_id);
                    }
                }
            }
            SortedSetCommand::RemoveLiquidationPrice(order_id, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self.liquidation_long_sortedset_db.remove(order_id);
                    }
                    PositionType::SHORT => {
                        let _ = self.liquidation_short_sortedset_db.remove(order_id);
                    }
                }
            }
            SortedSetCommand::RemoveCloseLimitPrice(order_id, position_type) => match position_type
            {
                PositionType::LONG => {
                    let _ = self.close_long_sortedset_db.remove(order_id);
                }
                PositionType::SHORT => {
                    let _ = self.close_short_sortedset_db.remove(order_id);
                }
            },
            SortedSetCommand::UpdateOpenLimitPrice(order_id, entry_price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .open_long_sortedset_db
                            .update(order_id, (entry_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .open_short_sortedset_db
                            .update(order_id, (entry_price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::UpdateLiquidationPrice(
                order_id,
                liquidation_price,
                position_type,
            ) => match position_type {
                PositionType::LONG => {
                    let _ = self
                        .liquidation_long_sortedset_db
                        .update(order_id, (liquidation_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .liquidation_short_sortedset_db
                        .update(order_id, (liquidation_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::UpdateCloseLimitPrice(order_id, execution_price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .close_long_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .close_short_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::BulkSearchRemoveOpenLimitPrice(price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .open_long_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .open_short_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::BulkSearchRemoveCloseLimitPrice(price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .close_long_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .close_short_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::BulkSearchRemoveLiquidationPrice(price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .liquidation_long_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .liquidation_short_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                    }
                }
            }
            SortedSetCommand::AddStopLossCloseLIMITPrice(
                order_id,
                stop_loss_price,
                position_type,
            ) => match position_type {
                PositionType::SHORT => {
                    let _ = self
                        .sl_close_long_sortedset_db
                        .add(order_id, (stop_loss_price * 10000.0) as i64);
                }
                PositionType::LONG => {
                    let _ = self
                        .sl_close_short_sortedset_db
                        .add(order_id, (stop_loss_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::AddTakeProfitCloseLIMITPrice(
                order_id,
                take_profit_price,
                position_type,
            ) => match position_type {
                PositionType::LONG => {
                    let _ = self
                        .tp_close_long_sortedset_db
                        .add(order_id, (take_profit_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .tp_close_short_sortedset_db
                        .add(order_id, (take_profit_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::RemoveStopLossCloseLIMITPrice(order_id, position_type) => {
                match position_type {
                    PositionType::SHORT => {
                        let _ = self.sl_close_long_sortedset_db.remove(order_id);
                    }
                    PositionType::LONG => {
                        let _ = self.sl_close_short_sortedset_db.remove(order_id);
                    }
                }
            }
            SortedSetCommand::RemoveTakeProfitCloseLIMITPrice(order_id, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self.tp_close_long_sortedset_db.remove(order_id);
                    }
                    PositionType::SHORT => {
                        let _ = self.tp_close_short_sortedset_db.remove(order_id);
                    }
                }
            }
            SortedSetCommand::UpdateStopLossCloseLIMITPrice(
                order_id,
                stop_loss_price,
                position_type,
            ) => match position_type {
                PositionType::SHORT => {
                    let _ = self
                        .sl_close_long_sortedset_db
                        .update(order_id, (stop_loss_price * 10000.0) as i64);
                }
                PositionType::LONG => {
                    let _ = self
                        .sl_close_short_sortedset_db
                        .update(order_id, (stop_loss_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::UpdateTakeProfitCloseLIMITPrice(
                order_id,
                take_profit_price,
                position_type,
            ) => match position_type {
                PositionType::LONG => {
                    let _ = self
                        .tp_close_long_sortedset_db
                        .update(order_id, (take_profit_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .tp_close_short_sortedset_db
                        .update(order_id, (take_profit_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .sl_close_long_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                        let _ = self
                            .tp_close_long_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .sl_close_short_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                        let _ = self
                            .tp_close_short_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                    }
                }
            }
        }
    }
}
