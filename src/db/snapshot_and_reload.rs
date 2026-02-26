#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use bincode;
use crc32fast::Hasher as Crc32Hasher;
use std::io::Cursor;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use twilight_relayer_sdk::utxo_in_memory;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use twilight_relayer_sdk::zkvm;
use uuid::Uuid;

// ── Improvement #5: lock_or_err! macro ──────────────────────────────────────
macro_rules! lock_or_err {
    ($global:expr, $name:expr) => {
        $global.lock().map_err(|e| {
            crate::log_heartbeat!(error, "Error locking {} : {:?}", $name, e);
            e.to_string()
        })?
    };
}

// ── Improvement #6: Generic OrderDBSnapShot ─────────────────────────────────
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
    fn new() -> Self {
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
    fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV1 {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}
impl SnapshotDBOldV1 {
    pub fn new() -> Self {
        SnapshotDBOldV1 {
            orderdb_traderorder: OrderDBSnapShotTO::new(),
            orderdb_lendorder: OrderDBSnapShotLO::new(),
            lendpool_database: LendPool::default(),
            liquidation_long_sortedset_db: SortedSet::new(),
            liquidation_short_sortedset_db: SortedSet::new(),
            open_long_sortedset_db: SortedSet::new(),
            open_short_sortedset_db: SortedSet::new(),
            close_long_sortedset_db: SortedSet::new(),
            close_short_sortedset_db: SortedSet::new(),
            position_size_log: PositionSizeLog::new(),
            localdb_hashmap: HashMap::new(),
            event_offset_partition: (0, 0),
            event_timestamp: ServerTime::now().epoch,
            output_hex_storage:
                utxo_in_memory::db::LocalStorage::<Option<zkvm::zkos_types::Output>>::new(1),
            queue_manager: QueueState::new(),
        }
    }
    pub fn migrate_to_new(&self) -> SnapshotDBOldV2 {
        let snapshot_new = SnapshotDBOldV2 {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: SortedSet::new(),
            sltp_close_short_sortedset_db: SortedSet::new(),
            position_size_log: self.position_size_log.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        };
        snapshot_new
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV2 {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub localdb_hashmap: HashMap<String, f64>,
    // pub output_memo_hashmap: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOldV2 {
    pub fn migrate_to_new(&self) -> SnapshotDBOldV3 {
        SnapshotDBOldV3 {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db.clone(),
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: RiskStateOld {
                total_long_btc: 0.0,
                total_short_btc: 0.0,
                manual_halt: false,
                manual_close_only: false,
            },
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
    fn print_status(&self) {
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
}

// V3 snapshot format (had risk_state, no risk_params)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV3 {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskStateOld,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOldV3 {
    pub fn migrate_to_new(&self) -> SnapshotDBOld {
        SnapshotDBOld {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db.clone(),
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.clone(),
            risk_params: None,
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
}

// V4 snapshot format (has risk_state(old) + risk_params)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOld {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskStateOld,
    pub risk_params: Option<RiskParamsOld>,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOld {
    pub fn migrate_to_new(&self) -> SnapshotDBOldV5 {
        SnapshotDBOldV5 {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db.clone(),
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.migrate_to_new(),
            risk_params: self.risk_params.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
}

// V5 snapshot format (has RiskStateOldV5 with pause_funding + pause_price_feed, no pending fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV5 {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskStateOldV5,
    pub risk_params: Option<RiskParamsOld>,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOldV5 {
    pub fn migrate_to_new(&self) -> SnapshotDBOldV6 {
        SnapshotDBOldV6 {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db.clone(),
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.migrate_to_new(),
            risk_params: self.risk_params.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
}

// V6 snapshot format (old — has RiskParamsOld without mm_ratio)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV6 {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskState,
    pub risk_params: Option<RiskParamsOld>,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOldV6 {
    pub fn migrate_to_new(&self) -> SnapshotDB {
        SnapshotDB {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db.clone(),
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.clone(),
            risk_params: self.risk_params.as_ref().map(|rp| rp.migrate_to_new()),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
}

// V7 snapshot format (current — has RiskParams with mm_ratio)
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
    pub sltp_close_long_sortedset_db: SortedSet,
    pub sltp_close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub risk_state: RiskState,
    pub risk_params: Option<RiskParams>,
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

/// Current snapshot version written by this binary.
/// V8 = V7 struct layout + zstd compression on payload.
const SNAPSHOT_FORMAT_VERSION: u32 = 8;

impl SnapshotDB {
    fn new() -> Self {
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
            sltp_close_long_sortedset_db: SortedSet::new(),
            sltp_close_short_sortedset_db: SortedSet::new(),
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
        }
    }
    fn print_status(&self) {
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

    // ── Improvement #10: Snapshot metadata logging ──────────────────────────
    fn log_metadata(&self) {
        crate::log_heartbeat!(
            info,
            "Snapshot loaded: {} trader orders, {} lend orders, \
             liq_long={}, liq_short={}, open_long={}, open_short={}, \
             close_long={}, close_short={}, sltp_long={}, sltp_short={}, \
             offset=({}, {}), timestamp={}",
            self.orderdb_traderorder.ordertable.len(),
            self.orderdb_lendorder.ordertable.len(),
            self.liquidation_long_sortedset_db.len,
            self.liquidation_short_sortedset_db.len,
            self.open_long_sortedset_db.len,
            self.open_short_sortedset_db.len,
            self.close_long_sortedset_db.len,
            self.close_short_sortedset_db.len,
            self.sltp_close_long_sortedset_db.len,
            self.sltp_close_short_sortedset_db.len,
            self.event_offset_partition.0,
            self.event_offset_partition.1,
            self.event_timestamp
        );
    }
}

// ── Improvement #1+2+8: Version header + CRC32 checksum + zstd compression ─
// Snapshot wire format: [version: 4 bytes LE][crc32: 4 bytes LE][payload ...]
// V8+: payload is zstd-compressed bincode.  V1-V7: payload is raw bincode.
// Legacy (pre-header) files are detected by attempting versioned read first;
// if the version byte is unrecognised we fall back to the old trial-and-error
// deserialization so existing deployments can upgrade without data loss.

/// Decompress zstd payload, returning the raw bincode bytes.
fn zstd_decompress(compressed: &[u8]) -> Result<Vec<u8>, String> {
    zstd::decode_all(Cursor::new(compressed))
        .map_err(|e| format!("zstd decompression failed: {:?}", e))
}

/// Try to decode a snapshot file that has the new version+checksum header.
/// Returns `None` if the data is too short or the version marker doesn't match
/// any known version (which means it's probably a legacy file).
fn decode_versioned_snapshot(data: &[u8]) -> Result<Option<SnapshotDB>, String> {
    if data.len() < 8 {
        return Ok(None); // too short to have header — treat as legacy
    }

    let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let stored_crc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let payload = &data[8..];

    // Verify CRC32 checksum (covers the raw bytes on disk, whether compressed or not)
    let mut hasher = Crc32Hasher::new();
    hasher.update(payload);
    let computed_crc = hasher.finalize();
    if stored_crc != computed_crc {
        return Err(format!(
            "Snapshot checksum mismatch: stored={:#010x}, computed={:#010x}. File may be corrupt.",
            stored_crc, computed_crc
        ));
    }

    match version {
        // V8: zstd-compressed bincode with same struct layout as V7
        8 => {
            let decompressed = zstd_decompress(payload)?;
            bincode::deserialize::<SnapshotDB>(&decompressed)
                .map(|s| Some(s))
                .map_err(|e| format!("V8 deserialize failed: {:#?}", e))
        }
        // V7 and below: raw (uncompressed) bincode
        7 => bincode::deserialize::<SnapshotDB>(payload)
            .map(|s| Some(s))
            .map_err(|e| format!("V7 deserialize failed: {:#?}", e)),
        6 => bincode::deserialize::<SnapshotDBOldV6>(payload)
            .map(|s| Some(s.migrate_to_new()))
            .map_err(|e| format!("V6 deserialize failed: {:#?}", e)),
        5 => bincode::deserialize::<SnapshotDBOldV5>(payload)
            .map(|s| Some(s.migrate_to_new().migrate_to_new()))
            .map_err(|e| format!("V5 deserialize failed: {:#?}", e)),
        4 => bincode::deserialize::<SnapshotDBOld>(payload)
            .map(|s| Some(s.migrate_to_new().migrate_to_new().migrate_to_new()))
            .map_err(|e| format!("V4 deserialize failed: {:#?}", e)),
        3 => bincode::deserialize::<SnapshotDBOldV3>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V3 deserialize failed: {:#?}", e)),
        2 => bincode::deserialize::<SnapshotDBOldV2>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V2 deserialize failed: {:#?}", e)),
        1 => bincode::deserialize::<SnapshotDBOldV1>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V1 deserialize failed: {:#?}", e)),
        _ => Ok(None), // unknown version — treat as legacy
    }
}

/// Fall back to the old trial-and-error deserialization for legacy snapshot
/// files that were written without a version header.
fn decode_legacy_snapshot(data: &[u8]) -> Result<SnapshotDB, String> {
    if let Ok(snap) = bincode::deserialize::<SnapshotDB>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V7");
        return Ok(snap);
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV6>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V6, migrating");
        return Ok(snap.migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV5>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V5, migrating");
        return Ok(snap.migrate_to_new().migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOld>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V4, migrating");
        return Ok(snap.migrate_to_new().migrate_to_new().migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV3>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V3, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV2>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V2, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV1>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V1, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    Err("All legacy snapshot decode attempts failed (V7-V1)".to_string())
}

pub fn snapshot() -> Result<SnapshotDB, String> {
    crate::log_heartbeat!(info, "started taking snapshot");
    let read_snapshot = fs::read(format!(
        "{}-{}",
        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
    ));
    let decoded_snapshot: SnapshotDB;
    let fetchoffset: FetchOffset;
    match read_snapshot {
        Ok(snapshot_data_from_file) => {
            // ── Improvement #1+2: try versioned+checksummed format first ────
            decoded_snapshot = match decode_versioned_snapshot(&snapshot_data_from_file) {
                Ok(Some(snap)) => {
                    crate::log_heartbeat!(info, "Loaded versioned snapshot successfully");
                    fetchoffset = FetchOffset::Earliest;
                    snap
                }
                Ok(None) => {
                    // No valid version header — fall back to legacy trial-and-error
                    crate::log_heartbeat!(
                        info,
                        "No version header detected, trying legacy deserialization"
                    );
                    match decode_legacy_snapshot(&snapshot_data_from_file) {
                        Ok(snap) => {
                            fetchoffset = FetchOffset::Earliest;
                            snap
                        }
                        Err(e) => {
                            if *ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE {
                                crate::log_heartbeat!(
                                    warn,
                                    "All snapshot decode attempts failed. \
                                     ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true, \
                                     creating new empty snapshot. Error: {} \n path: {:?}",
                                    e,
                                    format!(
                                        "{}-{}",
                                        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                                    )
                                );
                                fetchoffset = FetchOffset::Earliest;
                                SnapshotDB::new()
                            } else {
                                return Err(format!(
                                    "Snapshot file exists but could not be decoded by any version (V7-V1). \
                                     Set ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true to discard and start fresh. \
                                     Error: {}, path: {}-{}",
                                    e, *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    // Versioned header was found but checksum failed or deserialization error
                    if *ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE {
                        crate::log_heartbeat!(
                            warn,
                            "Versioned snapshot decode failed: {}. \
                             ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true, creating new empty snapshot.",
                            e
                        );
                        fetchoffset = FetchOffset::Earliest;
                        SnapshotDB::new()
                    } else {
                        return Err(format!(
                            "Versioned snapshot decode failed: {}. \
                             Set ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true to discard and start fresh. \
                             path: {}-{}",
                            e, *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                        ));
                    }
                }
            };
        }
        Err(arg) => {
            crate::log_heartbeat!(
                info,
                "No previous Snapshot Found- Error:{:#?} \n path: {:?} \n Creating new Snapshot",
                arg,
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
            );
            decoded_snapshot = SnapshotDB::new();
            fetchoffset = FetchOffset::Earliest;
        }
    }

    let (snapshot_db_updated, tx_consumed) =
        match create_snapshot_data(fetchoffset, decoded_snapshot) {
            Ok(snap_data) => snap_data,
            Err(arg) => {
                return Err(arg.to_string());
            }
        };

    // ── Serialize with version header + CRC32 + zstd compression ──────────
    let raw_payload = match bincode::serialize(&snapshot_db_updated) {
        Ok(v) => v,
        Err(arg) => {
            crate::log_heartbeat!(error, "Could not encode snapshot data - Error:{:#?}", arg);
            return Err(arg.to_string());
        }
    };

    // Compress with zstd (level 3 = good speed/ratio balance)
    let compressed_payload = match zstd::encode_all(Cursor::new(&raw_payload), 3) {
        Ok(v) => v,
        Err(arg) => {
            crate::log_heartbeat!(error, "zstd compression failed: {:?}", arg);
            return Err(arg.to_string());
        }
    };

    crate::log_heartbeat!(
        info,
        "Snapshot size: raw={} bytes, compressed={} bytes ({:.1}x)",
        raw_payload.len(),
        compressed_payload.len(),
        raw_payload.len() as f64 / compressed_payload.len().max(1) as f64
    );

    // CRC32 is over the compressed bytes (what's actually on disk)
    let mut hasher = Crc32Hasher::new();
    hasher.update(&compressed_payload);
    let crc = hasher.finalize();

    let mut encoded_v = Vec::with_capacity(8 + compressed_payload.len());
    encoded_v.extend_from_slice(&SNAPSHOT_FORMAT_VERSION.to_le_bytes());
    encoded_v.extend_from_slice(&crc.to_le_bytes());
    encoded_v.extend_from_slice(&compressed_payload);

    write_snapshot_atomically(
        &encoded_v,
        &PathBuf::from(format!(
            "{}-{}",
            *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
        )),
        &tx_consumed,
        snapshot_db_updated.event_offset_partition,
    )
    .map_err(|e| {
        crate::log_heartbeat!(error, "snapshot write failed: {:?}", e);
        e.to_string()
    })?;

    crate::log_heartbeat!(info, "Snapshot Done");

    Ok(snapshot_db_updated)
}

// ── Improvement #3: SnapshotBuilder ─────────────────────────────────────────
// ── Improvement #4: Deduplicated TxHash helper ──────────────────────────────

struct SnapshotBuilder {
    orderdb_traderorder: OrderDBSnapShotTO,
    orderdb_lendorder: OrderDBSnapShotLO,
    lendpool_database: LendPool,
    liquidation_long_sortedset_db: SortedSet,
    liquidation_short_sortedset_db: SortedSet,
    open_long_sortedset_db: SortedSet,
    open_short_sortedset_db: SortedSet,
    close_long_sortedset_db: SortedSet,
    close_short_sortedset_db: SortedSet,
    sltp_close_long_sortedset_db: SortedSet,
    sltp_close_short_sortedset_db: SortedSet,
    position_size_log: PositionSizeLog,
    risk_state: RiskState,
    risk_params: Option<RiskParams>,
    localdb_hashmap: HashMap<String, f64>,
    event_offset_partition: (i32, i64),
    event_timestamp: String,
    output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotBuilder {
    fn from_snapshot(snapshot_db: SnapshotDB, event_timestamp: String) -> Self {
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
            sltp_close_long_sortedset_db: snapshot_db.sltp_close_long_sortedset_db,
            sltp_close_short_sortedset_db: snapshot_db.sltp_close_short_sortedset_db,
            position_size_log: snapshot_db.position_size_log,
            risk_state: snapshot_db.risk_state,
            risk_params: snapshot_db.risk_params,
            localdb_hashmap: snapshot_db.localdb_hashmap,
            event_offset_partition: snapshot_db.event_offset_partition,
            event_timestamp,
            output_hex_storage: snapshot_db.output_hex_storage,
            queue_manager: snapshot_db.queue_manager,
        }
    }

    // ── Improvement #9: move owned values instead of cloning ────────────────
    fn build(mut self) -> SnapshotDB {
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
            sltp_close_long_sortedset_db: self.sltp_close_long_sortedset_db,
            sltp_close_short_sortedset_db: self.sltp_close_short_sortedset_db,
            position_size_log: self.position_size_log,
            risk_state: self.risk_state,
            risk_params: self.risk_params,
            localdb_hashmap: self.localdb_hashmap,
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp,
            output_hex_storage: self.output_hex_storage,
            queue_manager: self.queue_manager,
        }
    }

    fn handle_event(&mut self, data: &EventLog) {
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
                // handled in caller
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
            Event::SortedSetDBUpdate(cmd) => self.handle_sorted_set_update(cmd),
            Event::PositionSizeLogDBUpdate(_cmd, event) => {
                self.position_size_log = event;
            }
            Event::RiskEngineUpdate(_cmd, event) => {
                self.risk_state = event;
            }
            Event::RiskParamsUpdate(params) => {
                self.risk_params = Some(params);
            }
            Event::TxHash(
                orderid,
                _account_id,
                _tx_hash,
                order_type,
                order_status,
                _timestamp,
                option_output,
                _request_id,
            ) => {
                self.handle_output_storage(orderid, order_type, order_status, option_output);
            }
            Event::TxHashUpdate(
                orderid,
                _account_id,
                _tx_hash,
                order_type,
                order_status,
                _timestamp,
                option_output,
            ) => {
                self.handle_output_storage(orderid, order_type, order_status, option_output);
            }
            Event::AdvanceStateQueue(_, _) => {}
        }
    }

    // ── Improvement #4: Shared TxHash / TxHashUpdate handler ────────────────
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
                            let _ = self.liquidation_long_sortedset_db.add(
                                order.uuid,
                                (order.liquidation_price * 10000.0) as i64,
                            );
                        }
                        PositionType::SHORT => {
                            let _ = self.liquidation_short_sortedset_db.add(
                                order.uuid,
                                (order.liquidation_price * 10000.0) as i64,
                            );
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
                let _ = self
                    .orderdb_traderorder
                    .set_order_check(order.account_id);
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
                            let _ =
                                self.liquidation_long_sortedset_db.remove(order.uuid);
                        }
                        PositionType::SHORT => {
                            let _ =
                                self.liquidation_short_sortedset_db.remove(order.uuid);
                        }
                    },
                    _ => {}
                }
                self.queue_manager.remove_settle(&order.uuid);
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
                    let _ = self.liquidation_long_sortedset_db.add(
                        order.uuid,
                        (order.liquidation_price * 10000.0) as i64,
                    );
                }
                PositionType::SHORT => {
                    let _ = self.liquidation_short_sortedset_db.add(
                        order.uuid,
                        (order.liquidation_price * 10000.0) as i64,
                    );
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
                let _ = self.liquidation_long_sortedset_db.update(
                    order.uuid,
                    (order.liquidation_price * 10000.0) as i64,
                );
            }
            PositionType::SHORT => {
                let _ = self.liquidation_short_sortedset_db.update(
                    order.uuid,
                    (order.liquidation_price * 10000.0) as i64,
                );
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
            RpcCommand::CreateLendOrder(
                _rpc_request,
                _metadata,
                zkos_hex_string,
                _request_id,
            ) => {
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
            SortedSetCommand::AddLiquidationPrice(
                order_id,
                liquidation_price,
                position_type,
            ) => match position_type {
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
            },
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
            SortedSetCommand::RemoveCloseLimitPrice(order_id, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self.close_long_sortedset_db.remove(order_id);
                    }
                    PositionType::SHORT => {
                        let _ = self.close_short_sortedset_db.remove(order_id);
                    }
                }
            }
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
            SortedSetCommand::UpdateCloseLimitPrice(
                order_id,
                execution_price,
                position_type,
            ) => match position_type {
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
            },
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
            // Stop Loss and Take Profit Close LIMIT Price
            SortedSetCommand::AddStopLossCloseLIMITPrice(
                order_id,
                stop_loss_price,
                position_type,
            ) => match position_type {
                PositionType::SHORT => {
                    let _ = self
                        .sltp_close_long_sortedset_db
                        .add(order_id, (stop_loss_price * 10000.0) as i64);
                }
                PositionType::LONG => {
                    let _ = self
                        .sltp_close_short_sortedset_db
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
                        .sltp_close_long_sortedset_db
                        .add(order_id, (take_profit_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .sltp_close_short_sortedset_db
                        .add(order_id, (take_profit_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::RemoveStopLossCloseLIMITPrice(order_id, position_type) => {
                match position_type {
                    PositionType::SHORT => {
                        let _ = self.sltp_close_long_sortedset_db.remove(order_id);
                    }
                    PositionType::LONG => {
                        let _ = self.sltp_close_short_sortedset_db.remove(order_id);
                    }
                }
            }
            SortedSetCommand::RemoveTakeProfitCloseLIMITPrice(order_id, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self.sltp_close_long_sortedset_db.remove(order_id);
                    }
                    PositionType::SHORT => {
                        let _ = self.sltp_close_short_sortedset_db.remove(order_id);
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
                        .sltp_close_long_sortedset_db
                        .update(order_id, (stop_loss_price * 10000.0) as i64);
                }
                PositionType::LONG => {
                    let _ = self
                        .sltp_close_short_sortedset_db
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
                        .sltp_close_long_sortedset_db
                        .update(order_id, (take_profit_price * 10000.0) as i64);
                }
                PositionType::SHORT => {
                    let _ = self
                        .sltp_close_short_sortedset_db
                        .update(order_id, (take_profit_price * 10000.0) as i64);
                }
            },
            SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(price, position_type) => {
                match position_type {
                    PositionType::LONG => {
                        let _ = self
                            .sltp_close_long_sortedset_db
                            .search_lt((price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = self
                            .sltp_close_short_sortedset_db
                            .search_gt((price * 10000.0) as i64);
                    }
                }
            }
        }
    }
}

pub fn create_snapshot_data(
    fetchoffset: FetchOffset,
    snapshot_db: SnapshotDB,
) -> Result<(SnapshotDB, crossbeam_channel::Sender<(i32, i64)>), String> {
    let time = ServerTime::now().epoch;
    let event_timestamp = time.clone();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    let _ = Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );

    let mut builder = SnapshotBuilder::from_snapshot(snapshot_db, event_timestamp);

    let mut stop_signal: bool = true;
    let mut retry_attempt = 0;
    while retry_attempt < 10 {
        match Event::receive_event_for_snapshot_from_kafka_queue(
            CORE_EVENT_LOG.clone().to_string(),
            format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
            fetchoffset,
            "snapshot handle",
        ) {
            Ok((receiver_lock, tx_consumed)) => {
                let recever1 = match receiver_lock.lock() {
                    Ok(rec_lock) => rec_lock,
                    Err(arg) => {
                        retry_attempt += 1;
                        crate::log_heartbeat!(
                            error,
                            "unable to lock the kafka log receiver :{:?}",
                            arg
                        );
                        continue;
                    }
                };
                let mut last_offset = 0;
                let mut kafka_reconnect_attempt = 0;
                while stop_signal {
                    match recever1.recv() {
                        Ok(data) => {
                            // Handle PoolUpdate offset tracking separately
                            if let Event::PoolUpdate(_, _, _) = &data.value {
                                if data.offset > last_offset {
                                    last_offset = data.offset;
                                    builder.handle_event(&data);
                                }
                            } else if let Event::Stop(ref timex) = data.value {
                                if *timex == event_stoper_string {
                                    stop_signal = false;
                                    builder.event_offset_partition =
                                        (data.partition, data.offset);
                                }
                            } else {
                                builder.handle_event(&data);
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(
                                error,
                                "Error at kafka log receiver : {:?}",
                                arg
                            );
                            kafka_reconnect_attempt += 1;
                            if kafka_reconnect_attempt > 20 {
                                return Err(arg.to_string());
                            }
                            thread::sleep(std::time::Duration::from_millis(1000));
                        }
                    }
                }

                return Ok((builder.build(), tx_consumed));
            }
            Err(arg) => {
                crate::log_heartbeat!(
                    error,
                    "Failed to connect to kafka with error :{:?}\n attempt:{}",
                    arg,
                    retry_attempt
                );
                if retry_attempt == 10 {
                    return Err(arg.to_string());
                }
                retry_attempt += 1;
                thread::sleep(std::time::Duration::from_millis(1000));
            }
        };
    }

    return Err("Unable to connect to kafka".to_string());
}

// ── Improvement #5: load_from_snapshot using lock_or_err! macro ─────────────
pub fn load_from_snapshot() -> Result<QueueState, String> {
    match snapshot() {
        Ok(snapshot_data) => {
            let mut liquidation_long_sortedset_db =
                lock_or_err!(TRADER_LP_LONG, "TRADER_LP_LONG");
            let mut liquidation_short_sortedset_db =
                lock_or_err!(TRADER_LP_SHORT, "TRADER_LP_SHORT");
            let mut open_long_sortedset_db =
                lock_or_err!(TRADER_LIMIT_OPEN_LONG, "TRADER_LIMIT_OPEN_LONG");
            let mut open_short_sortedset_db =
                lock_or_err!(TRADER_LIMIT_OPEN_SHORT, "TRADER_LIMIT_OPEN_SHORT");
            let mut close_long_sortedset_db =
                lock_or_err!(TRADER_LIMIT_CLOSE_LONG, "TRADER_LIMIT_CLOSE_LONG");
            let mut close_short_sortedset_db =
                lock_or_err!(TRADER_LIMIT_CLOSE_SHORT, "TRADER_LIMIT_CLOSE_SHORT");
            let mut sltp_close_long_sortedset_db =
                lock_or_err!(TRADER_SLTP_CLOSE_LONG, "TRADER_SLTP_CLOSE_LONG");
            let mut sltp_close_short_sortedset_db =
                lock_or_err!(TRADER_SLTP_CLOSE_SHORT, "TRADER_SLTP_CLOSE_SHORT");
            let mut position_size_log =
                lock_or_err!(POSITION_SIZE_LOG, "POSITION_SIZE_LOG");
            let mut load_trader_data =
                lock_or_err!(TRADER_ORDER_DB, "TRADER_ORDER_DB");
            let mut load_lend_data = lock_or_err!(LEND_ORDER_DB, "LEND_ORDER_DB");
            let mut load_pool_data = lock_or_err!(LEND_POOL_DB, "LEND_POOL_DB");
            let mut output_hex_storage =
                lock_or_err!(OUTPUT_STORAGE, "OUTPUT_STORAGE");

            snapshot_data.print_status();
            // ── Improvement #10: log metadata ───────────────────────────────
            snapshot_data.log_metadata();

            *output_hex_storage = snapshot_data.output_hex_storage;
            drop(output_hex_storage);

            // ── Improvement #9: remove unnecessary .clone() on Copy types ───
            // add field of Trader order db
            load_trader_data.sequence = snapshot_data.orderdb_traderorder.sequence;
            load_trader_data.nonce = snapshot_data.orderdb_traderorder.nonce;
            load_trader_data.aggrigate_log_sequence =
                snapshot_data.orderdb_traderorder.aggrigate_log_sequence;
            load_trader_data.last_snapshot_id =
                snapshot_data.orderdb_traderorder.last_snapshot_id;
            load_trader_data.zkos_msg = snapshot_data.orderdb_traderorder.zkos_msg.clone();
            load_trader_data.hash = snapshot_data.orderdb_traderorder.hash.clone();
            //end

            // add field of Lend order db
            load_lend_data.sequence = snapshot_data.orderdb_lendorder.sequence;
            load_lend_data.nonce = snapshot_data.orderdb_lendorder.nonce;
            load_lend_data.aggrigate_log_sequence =
                snapshot_data.orderdb_lendorder.aggrigate_log_sequence;
            load_lend_data.last_snapshot_id =
                snapshot_data.orderdb_lendorder.last_snapshot_id;
            load_lend_data.zkos_msg = snapshot_data.orderdb_lendorder.zkos_msg.clone();
            load_lend_data.hash = snapshot_data.orderdb_lendorder.hash.clone();
            // end
            drop(load_trader_data);
            drop(load_lend_data);
            let traderorder_hashmap = snapshot_data.orderdb_traderorder.ordertable.clone();
            let lendorder_hashmap = snapshot_data.orderdb_lendorder.ordertable.clone();

            let trader_order_handle = match thread::Builder::new()
                .name(String::from("trader_order_handle"))
                .spawn(move || {
                    match TRADER_ORDER_DB.lock() {
                        Ok(mut load_trader_data) => {
                            for (key, val) in traderorder_hashmap.iter() {
                                load_trader_data
                                    .ordertable
                                    .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(
                                error,
                                "Error locking TRADER_ORDER_DB : {:?}",
                                arg
                            );
                        }
                    };
                }) {
                Ok(handle) => handle,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error creating trader_order_handle : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let lend_order_handle = match thread::Builder::new()
                .name(String::from("lend_order_handle"))
                .spawn(move || {
                    match LEND_ORDER_DB.lock() {
                        Ok(mut load_lend_data) => {
                            for (key, val) in lendorder_hashmap.iter() {
                                load_lend_data
                                    .ordertable
                                    .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(error, "Error locking LEND_ORDER_DB : {:?}", arg);
                        }
                    };
                }) {
                Ok(handle) => handle,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error creating lend_order_handle : {:?}", arg);
                    return Err(arg.to_string());
                }
            };

            *liquidation_long_sortedset_db = snapshot_data.liquidation_long_sortedset_db.clone();
            *liquidation_short_sortedset_db = snapshot_data.liquidation_short_sortedset_db.clone();
            *open_long_sortedset_db = snapshot_data.open_long_sortedset_db.clone();
            *open_short_sortedset_db = snapshot_data.open_short_sortedset_db.clone();
            *close_long_sortedset_db = snapshot_data.close_long_sortedset_db.clone();
            *close_short_sortedset_db = snapshot_data.close_short_sortedset_db.clone();
            *sltp_close_long_sortedset_db = snapshot_data.sltp_close_long_sortedset_db.clone();
            *sltp_close_short_sortedset_db = snapshot_data.sltp_close_short_sortedset_db.clone();
            *position_size_log = snapshot_data.position_size_log.clone();
            // Restore risk engine state from snapshot
            {
                let mut risk_engine_state = RISK_ENGINE_STATE.lock().unwrap();
                *risk_engine_state = snapshot_data.risk_state.clone();
                drop(risk_engine_state);
            }
            // Restore risk params from snapshot (if previously set via RPC)
            if let Some(ref params) = snapshot_data.risk_params {
                let mut risk_params = RISK_PARAMS.lock().unwrap();
                *risk_params = params.clone();
                drop(risk_params);
                crate::log_heartbeat!(info, "RISK_ENGINE: Restored risk params from snapshot");
            }
            *load_pool_data = snapshot_data.lendpool_database.clone();
            // ── Improvement #9: remove unnecessary .clone() on f64 (Copy) ───
            let current_price = snapshot_data.localdb_hashmap.get("CurrentPrice");
            set_localdb(
                "CurrentPrice",
                match current_price {
                    Some(value) => *value,
                    None => 60000.0,
                },
            );
            let funding_rate = snapshot_data.localdb_hashmap.get("FundingRate");
            set_localdb(
                "FundingRate",
                match funding_rate {
                    Some(value) => *value,
                    None => 0.0,
                },
            );
            let order_filled_on_market = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnMarket.into())
                .unwrap_or(&0.0);
            let order_filled_on_limit = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnLimit.into())
                .unwrap_or(&0.0);
            let order_settled_on_market = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnMarket.into())
                .unwrap_or(&0.0);
            let order_settled_on_limit = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnLimit.into())
                .unwrap_or(&0.0);
            set_fee(FeeType::FilledOnMarket, order_filled_on_market);
            set_fee(FeeType::FilledOnLimit, order_filled_on_limit);
            set_fee(FeeType::SettledOnMarket, order_settled_on_market);
            set_fee(FeeType::SettledOnLimit, order_settled_on_limit);

            match trader_order_handle.join() {
                Ok(_) => {}
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error at trader_order_handle : {:?}", arg);
                }
            }

            match lend_order_handle.join() {
                Ok(_) => {}
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error at lend_order_handle : {:?}", arg);
                }
            }
            return Ok(snapshot_data.queue_manager);
        }

        Err(arg) => {
            crate::log_heartbeat!(
                error,
                "unable to load data from snapshot \n error: {:?}",
                arg
            );
            return Err(arg.to_string());
        }
    }
}

// ── Improvement #7: Remove unnecessary delete before rename ─────────────────
/// Atomically replace the snapshot file and notify the offset channel.
/// On success it returns `Ok(())`; otherwise the caller gets a detailed `io::Error`.
fn write_snapshot_atomically(
    encoded: &[u8],
    final_path: &PathBuf,
    tx_consumed: &crossbeam_channel::Sender<(i32, i64)>,
    offset: (i32, i64),
) -> io::Result<()> {
    // 1. Write to temporary file
    let tmp_path = final_path.with_extension("new");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)?;
    file.write_all(encoded)?;
    file.sync_all()?; // flush file contents

    // 2. Atomically move temp → final (fs::rename replaces atomically on Unix)
    fs::rename(&tmp_path, final_path)?;

    // 3. fsync the directory entry so the rename itself is durable
    if let Some(parent) = final_path.parent() {
        let dir = File::open(parent)?;
        dir.sync_all()?;
    }

    // 4. Push committed offset downstream; ignore channel-full errors
    let _ = tx_consumed.try_send(offset);

    Ok(())
}
