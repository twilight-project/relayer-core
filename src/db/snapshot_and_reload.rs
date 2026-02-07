#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use bincode;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShotTO {
    pub ordertable: HashMap<Uuid, TraderOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}
impl OrderDBSnapShotTO {
    fn new() -> Self {
        OrderDBSnapShotTO {
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
pub struct OrderDBSnapShotLO {
    pub ordertable: HashMap<Uuid, LendOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}
impl OrderDBSnapShotLO {
    fn new() -> Self {
        OrderDBSnapShotLO {
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
    pub fn migrate_to_new(&self) -> SnapshotDBOld {
        let snapshot_new = SnapshotDBOld {
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
    pub localdb_hashmap: HashMap<String, f64>,
    // pub output_memo_hashmap: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

impl SnapshotDBOld {
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
            risk_state: RiskState::new(),
            localdb_hashmap: self.localdb_hashmap.clone(),
            event_offset_partition: self.event_offset_partition,
            event_timestamp: self.event_timestamp.clone(),
            output_hex_storage: self.output_hex_storage.clone(),
            queue_manager: self.queue_manager.clone(),
        }
    }
    fn new_old() -> Self {
        SnapshotDBOld {
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
}

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
    pub localdb_hashmap: HashMap<String, f64>,
    pub event_offset_partition: (i32, i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager: QueueState,
}

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
}

pub fn snapshot() -> Result<SnapshotDB, String> {
    // let read_snapshot = fs::read("snapshot").expect("Could not read file");
    // snapshot renaming on success
    // encryption on snapshot data
    // snapshot version
    // delete old snapshot data deleted by cron job
    crate::log_heartbeat!(info, "started taking snapshot");
    let read_snapshot = fs::read(format!(
        "{}-{}",
        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
    ));
    let decoded_snapshot: SnapshotDB;
    let fetchoffset: FetchOffset;
    match read_snapshot {
        Ok(snapshot_data_from_file) => {
            decoded_snapshot = match bincode::deserialize::<SnapshotDB>(&snapshot_data_from_file) {
                Ok(snap_data) => {
                    fetchoffset = FetchOffset::Earliest;
                    snap_data
                }
                Err(arg) => {
                    crate::log_heartbeat!(error, "Could not decode V3 snapshot - Error:{:#?}", arg);
                    crate::log_heartbeat!(error, "trying V2 (SnapshotDBOld) format:");
                    match bincode::deserialize::<SnapshotDBOld>(&snapshot_data_from_file) {
                        Ok(snap_data) => {
                            fetchoffset = FetchOffset::Earliest;
                            snap_data.migrate_to_new()
                        }
                        Err(arg2) => {
                            crate::log_heartbeat!(error, "Could not decode V2 snapshot - Error:{:#?}", arg2);
                            crate::log_heartbeat!(error, "trying V1 (SnapshotDBOldV1) format:");
                            match bincode::deserialize::<SnapshotDBOldV1>(&snapshot_data_from_file) {
                                Ok(snap_data) => {
                                    fetchoffset = FetchOffset::Earliest;
                                    snap_data.migrate_to_new().migrate_to_new()
                                }
                                Err(arg3) => {
                                    crate::log_heartbeat!(
                                        info,
                                        "No previous Snapshot Found- Error:{:#?} \n path: {:?} \n Creating new Snapshot",
                                        arg3,
                                        format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
                                    );
                                    fetchoffset = FetchOffset::Earliest;
                                    SnapshotDB::new()
                                }
                            }
                        }
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

    let encoded_v = match bincode::serialize(&snapshot_db_updated) {
        Ok(v) => v,
        Err(arg) => {
            crate::log_heartbeat!(error, "Could not encode snapshot data - Error:{:#?}", arg);
            return Err(arg.to_string());
        }
    };

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

pub fn create_snapshot_data(
    fetchoffset: FetchOffset,
    snapshot_db: SnapshotDB,
) -> Result<(SnapshotDB, crossbeam_channel::Sender<(i32, i64)>), String> {
    let SnapshotDB {
        mut orderdb_traderorder,
        mut orderdb_lendorder,
        mut lendpool_database,
        mut liquidation_long_sortedset_db,
        mut liquidation_short_sortedset_db,
        mut open_long_sortedset_db,
        mut open_short_sortedset_db,
        mut close_long_sortedset_db,
        mut close_short_sortedset_db,
        mut sltp_close_long_sortedset_db,
        mut sltp_close_short_sortedset_db,
        mut position_size_log,
        mut risk_state,
        mut localdb_hashmap,
        mut event_offset_partition,
        event_timestamp: _,
        mut output_hex_storage,
        mut queue_manager,
    } = snapshot_db;

    let time = ServerTime::now().epoch;
    let event_timestamp = time.clone();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    let _ = Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
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
                            match data.value.clone() {
                                Event::TraderOrder(order, cmd, seq) => match cmd {
                                    RpcCommand::CreateTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // let order_clone = order.clone();
                                        orderdb_traderorder
                                            .ordertable
                                            .insert(order.uuid, order.clone());
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence
                                            < order.entry_sequence.clone()
                                        {
                                            orderdb_traderorder.sequence =
                                                order.entry_sequence.clone();
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }

                                        match order.order_status {
                                            OrderStatus::FILLED => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ = liquidation_long_sortedset_db.add(
                                                        order.uuid,
                                                        (order.liquidation_price * 10000.0) as i64,
                                                    );
                                                }
                                                PositionType::SHORT => {
                                                    let _ = liquidation_short_sortedset_db.add(
                                                        order.uuid,
                                                        (order.liquidation_price * 10000.0) as i64,
                                                    );
                                                }
                                            },
                                            OrderStatus::PENDING => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ = open_long_sortedset_db.add(
                                                        order.uuid,
                                                        (order.entryprice * 10000.0) as i64,
                                                    );
                                                }
                                                PositionType::SHORT => {
                                                    let _ = open_short_sortedset_db.add(
                                                        order.uuid,
                                                        (order.entryprice * 10000.0) as i64,
                                                    );
                                                }
                                            },
                                            _ => {}
                                        }
                                        orderdb_traderorder
                                            .zkos_msg
                                            .insert(order.uuid, zkos_hex_string);
                                        let _ =
                                            orderdb_traderorder.set_order_check(order.account_id);
                                    }
                                    RpcCommand::CancelTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        let order_clone = order.clone();
                                        if orderdb_traderorder.ordertable.contains_key(&order.uuid)
                                        {
                                            orderdb_traderorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ = orderdb_traderorder
                                                .remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order_clone.entry_sequence
                                        {
                                            orderdb_traderorder.sequence =
                                                order_clone.entry_sequence;
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        queue_manager.remove_fill(&order.uuid);
                                    }
                                    RpcCommand::ExecuteTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // let order_clone = order.clone();
                                        if orderdb_traderorder
                                            .ordertable
                                            .contains_key(&order.uuid.clone())
                                        {
                                            orderdb_traderorder
                                                .ordertable
                                                .remove(&order.uuid.clone());
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ = orderdb_traderorder
                                                .remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence
                                            < order.entry_sequence.clone()
                                        {
                                            orderdb_traderorder.sequence =
                                                order.entry_sequence.clone();
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        match order.order_status {
                                            OrderStatus::SETTLED => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ = liquidation_long_sortedset_db
                                                        .remove(order.uuid);
                                                }
                                                PositionType::SHORT => {
                                                    let _ = liquidation_short_sortedset_db
                                                        .remove(order.uuid);
                                                }
                                            },
                                            _ => {}
                                        }
                                        queue_manager.remove_settle(&order.uuid);
                                    }
                                    RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                                        _rpc_request,
                                        _metadata,
                                        _payment,
                                    ) => {
                                        let order_clone = order.clone();
                                        if orderdb_traderorder.ordertable.contains_key(&order.uuid)
                                        {
                                            orderdb_traderorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ = orderdb_traderorder
                                                .remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order_clone.entry_sequence
                                        {
                                            orderdb_traderorder.sequence =
                                                order_clone.entry_sequence;
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        queue_manager.remove_fill(&order.uuid);
                                        queue_manager.remove_settle(&order.uuid);
                                    }
                                    RpcCommand::CreateTraderOrderSlTp(_, _, _, _, _, _) => {}
                                    RpcCommand::CancelTraderOrderSlTp(_, _, _, _, _) => {}
                                    RpcCommand::ExecuteLendOrder(_, _, _, _) => {}
                                    RpcCommand::CreateLendOrder(_, _, _, _) => {}
                                    RpcCommand::ExecuteTraderOrderSlTp(_, _, _, _, _) => {}
                                },
                                Event::TraderOrderLimitUpdate(order, cmd, seq) => match cmd {
                                    RpcCommand::ExecuteTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        orderdb_traderorder
                                            .zkos_msg
                                            .insert(order.uuid, zkos_hex_string);
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                    }
                                    RpcCommand::ExecuteTraderOrderSlTp(
                                        _rpc_request,
                                        _option_sltp,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        orderdb_traderorder
                                            .zkos_msg
                                            .insert(order.uuid, zkos_hex_string);
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                    }
                                    _ => {}
                                },
                                Event::TraderOrderUpdate(order, _cmd, seq) => {
                                    // let order_clone = order.clone();
                                    orderdb_traderorder
                                        .ordertable
                                        .insert(order.uuid, order.clone());
                                    // orderdb_traderorder.event.push(data.value);
                                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                                    }
                                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                                        orderdb_traderorder.aggrigate_log_sequence = seq;
                                    }

                                    match order.order_status {
                                        OrderStatus::FILLED => match order.position_type {
                                            PositionType::LONG => {
                                                let _ = liquidation_long_sortedset_db.add(
                                                    order.uuid,
                                                    (order.liquidation_price * 10000.0) as i64,
                                                );
                                            }
                                            PositionType::SHORT => {
                                                let _ = liquidation_short_sortedset_db.add(
                                                    order.uuid,
                                                    (order.liquidation_price * 10000.0) as i64,
                                                );
                                            }
                                        },
                                        _ => {}
                                    }
                                    queue_manager.remove_fill(&order.uuid);
                                }
                                Event::TraderOrderFundingUpdate(order, _cmd) => {
                                    orderdb_traderorder
                                        .ordertable
                                        .insert(order.uuid, order.clone());

                                    match order.position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db.update(
                                                order.uuid,
                                                (order.liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db.update(
                                                order.uuid,
                                                (order.liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                    }
                                }
                                Event::TraderOrderLiquidation(order, _cmd, seq) => {
                                    // let order_clone = order.clone();
                                    if orderdb_traderorder
                                        .ordertable
                                        .contains_key(&order.uuid.clone())
                                    {
                                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                                        let _removed_zkos_msg =
                                            orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                        let _ = orderdb_traderorder
                                            .remove_order_check(order.account_id);
                                    }
                                    // orderdb_traderorder.event.push(data.value);
                                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                                    }
                                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                                        orderdb_traderorder.aggrigate_log_sequence = seq;
                                    }
                                    queue_manager.remove_liquidate(&order.uuid);
                                }
                                Event::Stop(timex) => {
                                    if timex == event_stoper_string {
                                        stop_signal = false;
                                        event_offset_partition = (data.partition, data.offset);
                                    }
                                }
                                Event::LendOrder(order, cmd, seq) => match cmd {
                                    RpcCommand::CreateLendOrder(
                                        _rpc_request,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        let order_clone = order.clone();
                                        orderdb_lendorder.ordertable.insert(order.uuid, order);
                                        // orderdb_lendorder.event.push(data.value);
                                        if orderdb_lendorder.sequence < order_clone.entry_sequence {
                                            orderdb_lendorder.sequence = order_clone.entry_sequence;
                                        }
                                        if orderdb_lendorder.aggrigate_log_sequence < seq {
                                            orderdb_lendorder.aggrigate_log_sequence = seq;
                                        }
                                        orderdb_lendorder
                                            .zkos_msg
                                            .insert(order_clone.uuid, zkos_hex_string);
                                        let _ = orderdb_traderorder
                                            .set_order_check(order_clone.account_id);
                                    }
                                    RpcCommand::ExecuteLendOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // orderdb_lendorder.event.push(data.value);

                                        if orderdb_lendorder.ordertable.contains_key(&order.uuid) {
                                            orderdb_lendorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_lendorder.zkos_msg.remove(&order.uuid);
                                            let _ = orderdb_traderorder
                                                .remove_order_check(order.account_id);
                                        }
                                        if orderdb_lendorder.aggrigate_log_sequence < seq {
                                            orderdb_lendorder.aggrigate_log_sequence = seq;
                                        }
                                        if orderdb_lendorder.sequence < order.entry_sequence {
                                            orderdb_lendorder.sequence = order.entry_sequence;
                                        }
                                    }
                                    _ => {}
                                },
                                Event::PoolUpdate(_cmd, lend_pool, _seq) => {
                                    if data.offset > last_offset {
                                        lendpool_database = lend_pool;
                                        last_offset = data.offset;
                                    }
                                }
                                Event::FundingRateUpdate(funding_rate, _current_price, _time) => {
                                    // set_localdb("FundingRate", funding_rate);
                                    localdb_hashmap.insert("FundingRate".to_string(), funding_rate);
                                }
                                Event::FeeUpdate(cmd, _time) => match cmd {
                                    RelayerCommand::UpdateFees(
                                        order_filled_on_market,
                                        order_filled_on_limit,
                                        order_settled_on_market,
                                        order_settled_on_limit,
                                    ) => {
                                        localdb_hashmap.insert(
                                            FeeType::FilledOnMarket.into(),
                                            order_filled_on_market,
                                        );
                                        localdb_hashmap.insert(
                                            FeeType::FilledOnLimit.into(),
                                            order_filled_on_limit,
                                        );
                                        localdb_hashmap.insert(
                                            FeeType::SettledOnMarket.into(),
                                            order_settled_on_market,
                                        );
                                        localdb_hashmap.insert(
                                            FeeType::SettledOnLimit.into(),
                                            order_settled_on_limit,
                                        );
                                    }
                                    _ => {}
                                },
                                Event::CurrentPriceUpdate(current_price, _time) => {
                                    // set_localdb("CurrentPrice", current_price);
                                    localdb_hashmap
                                        .insert("CurrentPrice".to_string(), current_price);

                                    queue_manager.bulk_insert_to_fill(
                                        &mut open_short_sortedset_db,
                                        &mut open_long_sortedset_db,
                                        current_price,
                                    );
                                    queue_manager.bulk_insert_to_liquidate(
                                        &mut liquidation_short_sortedset_db,
                                        &mut liquidation_long_sortedset_db,
                                        current_price,
                                    );
                                    queue_manager.bulk_insert_to_settle(
                                        &mut close_short_sortedset_db,
                                        &mut close_long_sortedset_db,
                                        current_price,
                                    );
                                }
                                Event::SortedSetDBUpdate(cmd) => match cmd {
                                    SortedSetCommand::AddOpenLimitPrice(
                                        order_id,
                                        entry_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db
                                                .add(order_id, (entry_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db
                                                .add(order_id, (entry_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::AddLiquidationPrice(
                                        order_id,
                                        liquidation_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _sortedset_ = liquidation_long_sortedset_db.add(
                                                order_id,
                                                (liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db.add(
                                                order_id,
                                                (liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    SortedSetCommand::AddCloseLimitPrice(
                                        order_id,
                                        execution_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db
                                                .add(order_id, (execution_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db
                                                .add(order_id, (execution_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::RemoveOpenLimitPrice(
                                        order_id,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db.remove(order_id);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db.remove(order_id);
                                        }
                                    },
                                    SortedSetCommand::RemoveLiquidationPrice(
                                        order_id,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db.remove(order_id);
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db.remove(order_id);
                                        }
                                    },
                                    SortedSetCommand::RemoveCloseLimitPrice(
                                        order_id,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db.remove(order_id);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db.remove(order_id);
                                        }
                                    },
                                    SortedSetCommand::UpdateOpenLimitPrice(
                                        order_id,
                                        entry_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db
                                                .update(order_id, (entry_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db
                                                .update(order_id, (entry_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::UpdateLiquidationPrice(
                                        order_id,
                                        liquidation_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db.update(
                                                order_id,
                                                (liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db.update(
                                                order_id,
                                                (liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    SortedSetCommand::UpdateCloseLimitPrice(
                                        order_id,
                                        execution_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db.update(
                                                order_id,
                                                (execution_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db.update(
                                                order_id,
                                                (execution_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveOpenLimitPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db
                                                .search_lt((price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveCloseLimitPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db
                                                .search_lt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveLiquidationPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db
                                                .search_lt((price * 10000.0) as i64);
                                        }
                                    },
                                    // Stop Loss and Take Profit Close LIMIT Price
                                    SortedSetCommand::AddStopLossCloseLIMITPrice(
                                        order_id,
                                        stop_loss_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::SHORT => {
                                            let _ = sltp_close_long_sortedset_db
                                                .add(order_id, (stop_loss_price * 10000.0) as i64);
                                        }
                                        PositionType::LONG => {
                                            let _ = sltp_close_short_sortedset_db
                                                .add(order_id, (stop_loss_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::AddTakeProfitCloseLIMITPrice(
                                        order_id,
                                        take_profit_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = sltp_close_long_sortedset_db.add(
                                                order_id,
                                                (take_profit_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = sltp_close_short_sortedset_db.add(
                                                order_id,
                                                (take_profit_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    SortedSetCommand::RemoveStopLossCloseLIMITPrice(
                                        order_id,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::SHORT => {
                                            let _ = sltp_close_long_sortedset_db.remove(order_id);
                                        }
                                        PositionType::LONG => {
                                            let _ = sltp_close_short_sortedset_db.remove(order_id);
                                        }
                                    },
                                    SortedSetCommand::RemoveTakeProfitCloseLIMITPrice(
                                        order_id,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = sltp_close_long_sortedset_db.remove(order_id);
                                        }
                                        PositionType::SHORT => {
                                            let _ = sltp_close_short_sortedset_db.remove(order_id);
                                        }
                                    },
                                    SortedSetCommand::UpdateStopLossCloseLIMITPrice(
                                        order_id,
                                        stop_loss_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::SHORT => {
                                            let _ = sltp_close_long_sortedset_db.update(
                                                order_id,
                                                (stop_loss_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::LONG => {
                                            let _ = sltp_close_short_sortedset_db.update(
                                                order_id,
                                                (stop_loss_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    SortedSetCommand::UpdateTakeProfitCloseLIMITPrice(
                                        order_id,
                                        take_profit_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = sltp_close_long_sortedset_db.update(
                                                order_id,
                                                (take_profit_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = sltp_close_short_sortedset_db.update(
                                                order_id,
                                                (take_profit_price * 10000.0) as i64,
                                            );
                                        }
                                    },
                                    // SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(
                                    //     price,
                                    //     position_type,
                                    // ) => match position_type {
                                    //     PositionType::SHORT => {
                                    //         let _ = sltp_close_long_sortedset_db
                                    //             .search_gt((price * 10000.0) as i64);
                                    //     }
                                    //     PositionType::LONG => {
                                    //         let _ = sltp_close_short_sortedset_db
                                    //             .search_lt((price * 10000.0) as i64);
                                    //     }
                                    // },
                                    SortedSetCommand::BulkSearchRemoveSLTPCloseLIMITPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = sltp_close_long_sortedset_db
                                                .search_lt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = sltp_close_short_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                    },
                                },
                                Event::PositionSizeLogDBUpdate(_cmd, event) => {
                                    position_size_log = event;
                                }
                                Event::RiskEngineUpdate(_cmd, event) => {
                                    risk_state = event;
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
                                ) => match order_type {
                                    OrderType::LIMIT | OrderType::MARKET => match order_status {
                                        OrderStatus::FILLED => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let output_memo_option = match option_output {
                                                Some(output_hex_string) => {
                                                    match hex::decode(output_hex_string) {
                                                        Ok(output_byte) => {
                                                            match bincode::deserialize(&output_byte)
                                                            {
                                                                Ok(output_memo) => {
                                                                    Some(output_memo)
                                                                }
                                                                Err(_) => None,
                                                            }
                                                        }
                                                        Err(_) => None,
                                                    }
                                                }
                                                None => None,
                                            };
                                            match output_memo_option {
                                                Some(output_memo) => {
                                                    let _ = output_hex_storage.add(
                                                        uuid_to_byte,
                                                        Some(output_memo),
                                                        0,
                                                    );
                                                }
                                                None => {}
                                            }
                                        }
                                        OrderStatus::SETTLED
                                        | OrderStatus::CANCELLED
                                        | OrderStatus::LIQUIDATE => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let _ = output_hex_storage.remove(uuid_to_byte, 0);
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                },
                                Event::TxHashUpdate(
                                    orderid,
                                    _account_id,
                                    _tx_hash,
                                    order_type,
                                    order_status,
                                    _timestamp,
                                    option_output,
                                ) => match order_type {
                                    OrderType::LIMIT | OrderType::MARKET => match order_status {
                                        OrderStatus::FILLED => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let output_memo_option = match option_output {
                                                Some(output_hex_string) => {
                                                    match hex::decode(output_hex_string) {
                                                        Ok(output_byte) => {
                                                            match bincode::deserialize(&output_byte)
                                                            {
                                                                Ok(output_memo) => {
                                                                    Some(output_memo)
                                                                }
                                                                Err(_) => None,
                                                            }
                                                        }
                                                        Err(_) => None,
                                                    }
                                                }
                                                None => None,
                                            };
                                            match output_memo_option {
                                                Some(output_memo) => {
                                                    let _ = output_hex_storage.add(
                                                        uuid_to_byte,
                                                        Some(output_memo),
                                                        0,
                                                    );
                                                }
                                                None => {}
                                            }
                                        }
                                        OrderStatus::SETTLED
                                        | OrderStatus::CANCELLED
                                        | OrderStatus::LIQUIDATE => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let _ = output_hex_storage.remove(uuid_to_byte, 0);
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                },
                                Event::AdvanceStateQueue(_, _) => {}
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(error, "Error at kafka log receiver : {:?}", arg);
                            kafka_reconnect_attempt += 1;
                            if kafka_reconnect_attempt > 20 {
                                return Err(arg.to_string());
                            }
                            thread::sleep(std::time::Duration::from_millis(1000));
                        }
                    }
                }

                if lendpool_database.aggrigate_log_sequence > 0 {
                } else {
                    lendpool_database = LendPool::new();
                }

                queue_manager.bulk_remove_queue(&orderdb_traderorder);

                return Ok((
                    SnapshotDB {
                        orderdb_traderorder: orderdb_traderorder.clone(),
                        orderdb_lendorder: orderdb_lendorder.clone(),
                        lendpool_database: lendpool_database.clone(),
                        liquidation_long_sortedset_db: liquidation_long_sortedset_db.clone(),
                        liquidation_short_sortedset_db: liquidation_short_sortedset_db.clone(),
                        open_long_sortedset_db: open_long_sortedset_db.clone(),
                        open_short_sortedset_db: open_short_sortedset_db.clone(),
                        close_long_sortedset_db: close_long_sortedset_db.clone(),
                        close_short_sortedset_db: close_short_sortedset_db.clone(),
                        sltp_close_long_sortedset_db: sltp_close_long_sortedset_db.clone(),
                        sltp_close_short_sortedset_db: sltp_close_short_sortedset_db.clone(),
                        position_size_log: position_size_log.clone(),
                        risk_state: risk_state.clone(),
                        localdb_hashmap: localdb_hashmap,
                        event_offset_partition: event_offset_partition,
                        event_timestamp: event_timestamp,
                        output_hex_storage: output_hex_storage,
                        queue_manager,
                    },
                    tx_consumed,
                ));
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

pub fn load_from_snapshot() -> Result<QueueState, String> {
    match snapshot() {
        Ok(snapshot_data) => {
            let mut liquidation_long_sortedset_db = match TRADER_LP_LONG.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking TRADER_LP_LONG : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut liquidation_short_sortedset_db = match TRADER_LP_SHORT.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking TRADER_LP_SHORT : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut open_long_sortedset_db = match TRADER_LIMIT_OPEN_LONG.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_LIMIT_OPEN_LONG : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };
            let mut open_short_sortedset_db = match TRADER_LIMIT_OPEN_SHORT.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_LIMIT_OPEN_SHORT : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };
            let mut close_long_sortedset_db = match TRADER_LIMIT_CLOSE_LONG.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_LIMIT_CLOSE_LONG : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };
            let mut close_short_sortedset_db = match TRADER_LIMIT_CLOSE_SHORT.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_LIMIT_CLOSE_SHORT : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };
            let mut sltp_close_long_sortedset_db = match TRADER_SLTP_CLOSE_LONG.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_SLTP_CLOSE_LONG : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };
            let mut sltp_close_short_sortedset_db = match TRADER_SLTP_CLOSE_SHORT.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(
                        error,
                        "Error locking TRADER_SLTP_CLOSE_SHORT : {:?}",
                        arg
                    );
                    return Err(arg.to_string());
                }
            };

            let mut position_size_log = match POSITION_SIZE_LOG.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking POSITION_SIZE_LOG : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut load_trader_data = match TRADER_ORDER_DB.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking TRADER_ORDER_DB : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut load_lend_data = match LEND_ORDER_DB.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking LEND_ORDER_DB : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut load_pool_data = match LEND_POOL_DB.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking LEND_POOL_DB : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let mut output_hex_storage = match OUTPUT_STORAGE.lock() {
                Ok(lock) => lock,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error locking OUTPUT_STORAGE : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            snapshot_data.print_status();
            *output_hex_storage = snapshot_data.output_hex_storage;
            drop(output_hex_storage);
            // add field of Trader order db
            load_trader_data.sequence = snapshot_data.orderdb_traderorder.sequence.clone();
            load_trader_data.nonce = snapshot_data.orderdb_traderorder.nonce.clone();
            load_trader_data.aggrigate_log_sequence = snapshot_data
                .orderdb_traderorder
                .aggrigate_log_sequence
                .clone();
            load_trader_data.last_snapshot_id =
                snapshot_data.orderdb_traderorder.last_snapshot_id.clone();
            load_trader_data.zkos_msg = snapshot_data.orderdb_traderorder.zkos_msg.clone();
            load_trader_data.hash = snapshot_data.orderdb_traderorder.hash.clone();
            //end

            // add field of Lend order db
            load_lend_data.sequence = snapshot_data.orderdb_lendorder.sequence.clone();
            load_lend_data.nonce = snapshot_data.orderdb_lendorder.nonce.clone();
            load_lend_data.aggrigate_log_sequence = snapshot_data
                .orderdb_lendorder
                .aggrigate_log_sequence
                .clone();
            load_lend_data.last_snapshot_id =
                snapshot_data.orderdb_lendorder.last_snapshot_id.clone();
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
            *load_pool_data = snapshot_data.lendpool_database.clone();
            let current_price = snapshot_data.localdb_hashmap.get("CurrentPrice").clone();
            set_localdb(
                "CurrentPrice",
                match current_price {
                    Some(value) => value.clone(),
                    None => 60000.0,
                },
            );
            let funding_rate = snapshot_data.localdb_hashmap.get("FundingRate").clone();
            set_localdb(
                "FundingRate",
                match funding_rate {
                    Some(value) => value.clone(),
                    None => 0.0,
                },
            );
            let order_filled_on_market = snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnMarket.into())
                .unwrap_or(&0.0)
                .clone();
            let order_filled_on_limit = snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnLimit.into())
                .unwrap_or(&0.0)
                .clone();
            let order_settled_on_market = snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnMarket.into())
                .unwrap_or(&0.0)
                .clone();
            let order_settled_on_limit = snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnLimit.into())
                .unwrap_or(&0.0)
                .clone();
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

    // 2. Remove old file if it exists
    if final_path.exists() {
        fs::remove_file(final_path)?; // safest: delete before rename
    }

    // 3. Atomically move temp → final
    fs::rename(&tmp_path, final_path)?;

    // 4. fsync the directory entry so the rename itself is durable
    if let Some(parent) = final_path.parent() {
        let dir = File::open(parent)?;
        dir.sync_all()?;
    }

    // 5. Push committed offset downstream; ignore channel‑full errors
    let _ = tx_consumed.try_send(offset);

    Ok(())
}
