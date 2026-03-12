use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use twilight_relayer_sdk::utxo_in_memory;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use twilight_relayer_sdk::zkvm;
use uuid::Uuid;

use super::current::{OrderDBSnapShotLO, OrderDBSnapShotTO, SnapshotDB};

// ═══════════════════════════════════════════════════════════════════════════════
// V1
// ═══════════════════════════════════════════════════════════════════════════════

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
    queue_manager: QueueStateOldV10,
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
            queue_manager: QueueStateOldV10 {
                to_liquidate: HashMap::new(),
                to_settle: HashMap::new(),
                to_fill: HashMap::new(),
                funding_update: HashMap::new(),
                to_fill_remove: Vec::new(),
                to_settle_remove: Vec::new(),
                to_liquidate_remove: Vec::new(),
            },
        }
    }
    pub fn migrate_to_new(&self) -> SnapshotDBOldV2 {
        SnapshotDBOldV2 {
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
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V2
// ═══════════════════════════════════════════════════════════════════════════════

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
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueStateOldV10,
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

// ═══════════════════════════════════════════════════════════════════════════════
// V3 (had risk_state, no risk_params)
// ═══════════════════════════════════════════════════════════════════════════════

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
    queue_manager: QueueStateOldV10,
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

// ═══════════════════════════════════════════════════════════════════════════════
// V4 (has risk_state(old) + risk_params)
// ═══════════════════════════════════════════════════════════════════════════════

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
    queue_manager: QueueStateOldV10,
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

// ═══════════════════════════════════════════════════════════════════════════════
// V5 (RiskStateOldV5 with pause_funding + pause_price_feed, no pending fields)
// ═══════════════════════════════════════════════════════════════════════════════

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
    queue_manager: QueueStateOldV10,
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

// ═══════════════════════════════════════════════════════════════════════════════
// V6 (old — has RiskParamsOld without mm_ratio)
// ═══════════════════════════════════════════════════════════════════════════════

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
    queue_manager: QueueStateOldV10,
}

impl SnapshotDBOldV6 {
    pub fn migrate_to_new(&self) -> SnapshotDBOldV7 {
        SnapshotDBOldV7 {
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

// ═══════════════════════════════════════════════════════════════════════════════
// V7 (old — has RiskParams with mm_ratio, no group_name)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV7 {
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
    queue_manager: QueueStateOldV10,
}

impl SnapshotDBOldV7 {
    pub fn migrate_to_new(&self) -> SnapshotDBOldV9 {
        SnapshotDBOldV9 {
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
            risk_params: self.risk_params.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
            group_name: format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V8/V9 (has group_name, merged SLTP sorted sets)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV9 {
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
    queue_manager: QueueStateOldV10,
    pub group_name: String,
}

impl SnapshotDBOldV9 {
    /// Split old merged SLTP sorted sets into 4 separate SL/TP sets using the order table.
    pub fn migrate_to_new(&self) -> SnapshotDBOldV10 {
        let mut sl_close_long = SortedSet::new();
        let mut sl_close_short = SortedSet::new();
        let mut tp_close_long = SortedSet::new();
        let mut tp_close_short = SortedSet::new();

        // sltp_close_long contained: SHORT StopLoss + LONG TakeProfit (search_lt direction)
        for &(uuid, price) in &self.sltp_close_long_sortedset_db.sorted_order {
            if let Some(order) = self.orderdb_traderorder.ordertable.get(&uuid) {
                match order.position_type {
                    PositionType::SHORT => { let _ = sl_close_long.add(uuid, price); }
                    PositionType::LONG => { let _ = tp_close_long.add(uuid, price); }
                }
            }
        }

        // sltp_close_short contained: LONG StopLoss + SHORT TakeProfit (search_gt direction)
        for &(uuid, price) in &self.sltp_close_short_sortedset_db.sorted_order {
            if let Some(order) = self.orderdb_traderorder.ordertable.get(&uuid) {
                match order.position_type {
                    PositionType::LONG => { let _ = sl_close_short.add(uuid, price); }
                    PositionType::SHORT => { let _ = tp_close_short.add(uuid, price); }
                }
            }
        }

        crate::log_heartbeat!(
            info,
            "Migrated SLTP sorted sets: sl_long={}, sl_short={}, tp_long={}, tp_short={}",
            sl_close_long.len, sl_close_short.len, tp_close_long.len, tp_close_short.len
        );

        SnapshotDBOldV10 {
            orderdb_traderorder: self.orderdb_traderorder.clone(),
            orderdb_lendorder: self.orderdb_lendorder.clone(),
            lendpool_database: self.lendpool_database.clone(),
            liquidation_long_sortedset_db: self.liquidation_long_sortedset_db.clone(),
            liquidation_short_sortedset_db: self.liquidation_short_sortedset_db.clone(),
            open_long_sortedset_db: self.open_long_sortedset_db.clone(),
            open_short_sortedset_db: self.open_short_sortedset_db.clone(),
            close_long_sortedset_db: self.close_long_sortedset_db.clone(),
            close_short_sortedset_db: self.close_short_sortedset_db.clone(),
            sl_close_long_sortedset_db: sl_close_long,
            sl_close_short_sortedset_db: sl_close_short,
            tp_close_long_sortedset_db: tp_close_long,
            tp_close_short_sortedset_db: tp_close_short,
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.clone(),
            risk_params: self.risk_params.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
            group_name: self.group_name.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V10 (split SL/TP sorted sets, old QueueState without SL/TP queues)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDBOldV10 {
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
    queue_manager: QueueStateOldV10,
    pub group_name: String,
}

impl SnapshotDBOldV10 {
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
            sl_close_long_sortedset_db: self.sl_close_long_sortedset_db.clone(),
            sl_close_short_sortedset_db: self.sl_close_short_sortedset_db.clone(),
            tp_close_long_sortedset_db: self.tp_close_long_sortedset_db.clone(),
            tp_close_short_sortedset_db: self.tp_close_short_sortedset_db.clone(),
            position_size_log: self.position_size_log.clone(),
            risk_state: self.risk_state.clone(),
            risk_params: self.risk_params.clone(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone().migrate_to_new(),
            group_name: self.group_name.clone(),
        }
    }
}
