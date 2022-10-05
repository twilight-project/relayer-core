#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
// use bincode::{config, Decode, Encode};
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;
lazy_static! {
    pub static ref SNAPSHOT_DATA: Arc<Mutex<SnapshotDB>> = Arc::new(Mutex::new(SnapshotDB::new()));
}
pub fn load_backup_data() -> (OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) {
    // fn load_data() -> (bool, Self) {
    let mut orderdb_traderorder: OrderDB<TraderOrder> = LocalDB::<TraderOrder>::new();
    let mut orderdb_lendrorder: OrderDB<LendOrder> = LocalDB::<LendOrder>::new();
    let mut lendpool_database: LendPool = LendPool::default();

    let mut liquidation_long_sortedset_db = TRADER_LP_LONG.lock().unwrap();
    let mut liquidation_short_sortedset_db = TRADER_LP_SHORT.lock().unwrap();
    let mut open_long_sortedset_db = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
    let mut open_short_sortedset_db = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
    let mut close_long_sortedset_db = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
    let mut close_short_sortedset_db = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
    let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros()
        .to_string();
    let eventstop: Event = Event::Stop(time.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut stop_signal: bool = true;

    let recever = Event::receive_event_from_kafka_queue(
        CORE_EVENT_LOG.clone().to_string(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string(),
    )
    .unwrap();
    let recever1 = recever.lock().unwrap();
    while stop_signal {
        let data = recever1.recv().unwrap();
        // match data.value {
        //     Event::CurrentPriceUpdate(..) => {}
        //     _ => {
        //         println!("Envent log: {:#?}", data);
        //     }
        // }
        match data.value.clone() {
            Event::TraderOrder(order, cmd, seq) => match cmd {
                RpcCommand::CreateTraderOrder(_rpc_request, _metadata) => {
                    // let order_clone = order.clone();
                    orderdb_traderorder
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
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
                                let _ = liquidation_long_sortedset_db
                                    .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                            }
                            PositionType::SHORT => {
                                let _ = liquidation_short_sortedset_db
                                    .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                            }
                        },
                        OrderStatus::PENDING => match order.position_type {
                            PositionType::LONG => {
                                let _ = open_long_sortedset_db
                                    .add(order.uuid, (order.entryprice * 10000.0) as i64);
                            }
                            PositionType::SHORT => {
                                let _ = open_short_sortedset_db
                                    .add(order.uuid, (order.entryprice * 10000.0) as i64);
                            }
                        },
                        _ => {}
                    }
                }
                RpcCommand::CancelTraderOrder(_rpc_request, _metadata) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
                }
                RpcCommand::ExecuteTraderOrder(_rpc_request, _metadata) => {
                    // let order_clone = order.clone();
                    if orderdb_traderorder
                        .ordertable
                        .contains_key(&order.uuid.clone())
                    {
                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
                    match order.order_status {
                        OrderStatus::SETTLED => match order.position_type {
                            PositionType::LONG => {
                                let _ = liquidation_long_sortedset_db.remove(order.uuid);
                            }
                            PositionType::SHORT => {
                                let _ = liquidation_short_sortedset_db.remove(order.uuid);
                            }
                        },
                        _ => {}
                    }
                }
                RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                    _rpc_request,
                    _metadata,
                    _payment,
                ) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
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
                    .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
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
                            let _ = liquidation_long_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = liquidation_short_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                    },
                    _ => {}
                }
            }
            Event::TraderOrderFundingUpdate(order, _cmd) => {
                orderdb_traderorder
                    .ordertable
                    .insert(order.uuid, Arc::new(RwLock::new(order.clone())));

                match order.position_type {
                    PositionType::LONG => {
                        let _ = liquidation_long_sortedset_db
                            .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
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
                }
                // orderdb_traderorder.event.push(data.value);
                if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                    orderdb_traderorder.sequence = order.entry_sequence.clone();
                }
                if orderdb_traderorder.aggrigate_log_sequence < seq {
                    orderdb_traderorder.aggrigate_log_sequence = seq;
                }
            }
            Event::Stop(timex) => {
                if timex == time {
                    stop_signal = false;
                }
            }
            Event::LendOrder(order, cmd, seq) => match cmd {
                RpcCommand::CreateLendOrder(..) => {
                    let order_clone = order.clone();
                    orderdb_lendrorder
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                    // orderdb_lendrorder.event.push(data.value);
                    if orderdb_lendrorder.sequence < order_clone.entry_sequence {
                        orderdb_lendrorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_lendrorder.aggrigate_log_sequence < seq {
                        orderdb_lendrorder.aggrigate_log_sequence = seq;
                    }
                }
                RpcCommand::ExecuteLendOrder(..) => {
                    // orderdb_lendrorder.event.push(data.value);

                    if orderdb_lendrorder.ordertable.contains_key(&order.uuid) {
                        orderdb_lendrorder.ordertable.remove(&order.uuid);
                    }
                    if orderdb_lendrorder.aggrigate_log_sequence < seq {
                        orderdb_lendrorder.aggrigate_log_sequence = seq;
                    }
                    if orderdb_lendrorder.sequence < order.entry_sequence {
                        orderdb_lendrorder.sequence = order.entry_sequence;
                    }
                }
                _ => {}
            },
            Event::PoolUpdate(cmd, seq) => match cmd.clone() {
                LendPoolCommand::InitiateNewPool(lend_order, _metadata) => {
                    let total_pool_share = lend_order.deposit;
                    let total_locked_value = lend_order.deposit * 10000.0;
                    if lendpool_database.sequence < lend_order.entry_sequence {
                        lendpool_database.sequence = lend_order.entry_sequence;
                    }
                    if lendpool_database.nonce < lend_order.entry_nonce {
                        lendpool_database.nonce = lend_order.entry_nonce;
                    }
                    lendpool_database.total_pool_share += total_pool_share;
                    lendpool_database.total_locked_value += total_locked_value;
                    // lendpool_database.event_log.push(data.value);
                    if lendpool_database.aggrigate_log_sequence < seq {
                        lendpool_database.aggrigate_log_sequence = seq;
                    }
                }
                LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    lendpool_database.total_locked_value += deposit * 10000.0;
                    lendpool_database.total_pool_share += lend_order.npoolshare;
                    // lendpool_database.event_log.push(data.value);
                }
                LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    lendpool_database.total_locked_value -= withdraw;
                    lendpool_database.total_pool_share -= lend_order.npoolshare;
                    // lendpool_database.event_log.push(data.value);
                }
                LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    match cmd {
                        RelayerCommand::FundingCycle(batch, _metadata, _fundingrate) => {
                            lendpool_database.total_locked_value -= batch.amount * 10000.0;
                        }
                        RelayerCommand::RpcCommandPoolupdate() => {
                            let batch = lendpool_database.pending_orders.clone();
                            lendpool_database.total_locked_value -= batch.amount * 10000.0;
                            lendpool_database.pending_orders = PoolBatchOrder::new();
                        }
                        _ => {}
                    }
                }
                LendPoolCommand::AddFundingData(..) => {}
                LendPoolCommand::AddTraderOrderSettlement(..) => {}
                LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                LendPoolCommand::AddTraderOrderLiquidation(..) => {}
            },
            Event::FundingRateUpdate(funding_rate, _time) => {
                set_localdb("FundingRate", funding_rate);
            }
            Event::CurrentPriceUpdate(current_price, _time) => {
                set_localdb("CurrentPrice", current_price);
            }
            Event::SortedSetDBUpdate(cmd) => match cmd {
                SortedSetCommand::AddOpenLimitPrice(order_id, entry_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db
                                .add(order_id, (entry_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db
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
                        let _sortedset_ = liquidation_long_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::AddCloseLimitPrice(order_id, execution_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db
                                .add(order_id, (execution_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db
                                .add(order_id, (execution_price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::RemoveOpenLimitPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::RemoveLiquidationPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = liquidation_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = liquidation_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::RemoveCloseLimitPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::UpdateOpenLimitPrice(order_id, entry_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db
                                .update(order_id, (entry_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db
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
                        let _ = liquidation_long_sortedset_db
                            .update(order_id, (liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .update(order_id, (liquidation_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::UpdateCloseLimitPrice(
                    order_id,
                    execution_price,
                    position_type,
                ) => match position_type {
                    PositionType::LONG => {
                        let _ = close_long_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = close_short_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::BulkSearchRemoveOpenLimitPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::BulkSearchRemoveCloseLimitPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::BulkSearchRemoveLiquidationPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ =
                                liquidation_long_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ =
                                liquidation_short_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                    }
                }
            },
            Event::PositionSizeLogDBUpdate(_cmd, event) => {
                // position_size_log.total_long_positionsize = event.total_long_positionsize;
                // position_size_log.totalpositionsize = event.totalpositionsize;
                // position_size_log.total_short_positionsize = event.total_short_positionsize;
                *position_size_log = event;
            }
        }
    }
    drop(liquidation_long_sortedset_db);
    drop(liquidation_short_sortedset_db);
    drop(open_long_sortedset_db);
    drop(open_short_sortedset_db);
    drop(close_long_sortedset_db);
    drop(close_short_sortedset_db);
    drop(position_size_log);
    if orderdb_traderorder.sequence > 0 {
        println!("TraderOrder Database Loaded ....");
    } else {
        println!("No old TraderOrder Database found ....\nCreating new TraderOrder_database");
    }
    if orderdb_lendrorder.sequence > 0 {
        println!("LendOrder Database Loaded ....");
    } else {
        println!("No old LendOrder Database found ....\nCreating new LendOrder_database");
    }
    if lendpool_database.aggrigate_log_sequence > 0 {
        println!("LendPool Database Loaded ....");
    } else {
        lendpool_database = LendPool::new();
        println!("No old LendPool Database found ....\nCreating new LendPool_database");
    }
    (
        orderdb_traderorder.clone(),
        orderdb_lendrorder.clone(),
        lendpool_database.clone(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShotTO {
    pub ordertable: HashMap<Uuid, TraderOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
}
impl OrderDBSnapShotTO {
    fn new() -> Self {
        OrderDBSnapShotTO {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShotLO {
    pub ordertable: HashMap<Uuid, LendOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
}
impl OrderDBSnapShotLO {
    fn new() -> Self {
        OrderDBSnapShotLO {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDB {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendrorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
}
impl SnapshotDB {
    fn new() -> Self {
        SnapshotDB {
            orderdb_traderorder: OrderDBSnapShotTO::new(),
            orderdb_lendrorder: OrderDBSnapShotLO::new(),
            lendpool_database: LendPool::default(),
            liquidation_long_sortedset_db: SortedSet::new(),
            liquidation_short_sortedset_db: SortedSet::new(),
            open_long_sortedset_db: SortedSet::new(),
            open_short_sortedset_db: SortedSet::new(),
            close_long_sortedset_db: SortedSet::new(),
            close_short_sortedset_db: SortedSet::new(),
            position_size_log: PositionSizeLog::new(),
        }
    }
}
use bincode;
use std::fs;
pub fn snapshot() {
    let read_snapshot = fs::read("snapshot").expect("Could not read file");
    let decoded_snapshot: SnapshotDB =
        bincode::deserialize(&read_snapshot).expect("Could not decode vector");
    let mut snapshot_data = SNAPSHOT_DATA.lock().unwrap();
    *snapshot_data = decoded_snapshot.clone();
    drop(snapshot_data);
    let mut snapshot_db_updated = create_snapshot_data();

    let mut snapshot_data = SNAPSHOT_DATA.lock().unwrap();
    *snapshot_data = snapshot_db_updated.clone();

    let encoded_v = bincode::serialize(&snapshot_db_updated).expect("Could not encode vector");
    fs::write("snapshot", encoded_v).expect("Could not write file");
}

pub fn create_snapshot_data() -> SnapshotDB {
    let snapshot_db = SNAPSHOT_DATA.lock().unwrap().clone();
    let mut orderdb_traderorder: OrderDBSnapShotTO = snapshot_db.orderdb_traderorder;
    let mut orderdb_lendrorder: OrderDBSnapShotLO = snapshot_db.orderdb_lendrorder;
    let mut lendpool_database: LendPool = snapshot_db.lendpool_database;
    let mut liquidation_long_sortedset_db = snapshot_db.liquidation_long_sortedset_db;
    let mut liquidation_short_sortedset_db = snapshot_db.liquidation_short_sortedset_db;
    let mut open_long_sortedset_db = snapshot_db.open_long_sortedset_db;
    let mut open_short_sortedset_db = snapshot_db.open_short_sortedset_db;
    let mut close_long_sortedset_db = snapshot_db.close_long_sortedset_db;
    let mut close_short_sortedset_db = snapshot_db.close_short_sortedset_db;
    let mut position_size_log = snapshot_db.position_size_log;

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros()
        .to_string();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut stop_signal: bool = true;

    let recever = Event::receive_event_from_kafka_queue(
        CORE_EVENT_LOG.clone().to_string(),
        "snapshot-group".to_string(),
    )
    .unwrap();
    let recever1 = recever.lock().unwrap();
    while stop_signal {
        let data = recever1.recv().unwrap();
        // match data.value {
        //     Event::CurrentPriceUpdate(..) => {}
        //     _ => {
        //         println!("Envent log: {:#?}", data);
        //     }
        // }
        match data.value.clone() {
            Event::TraderOrder(order, cmd, seq) => match cmd {
                RpcCommand::CreateTraderOrder(_rpc_request, _metadata) => {
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
                                let _ = liquidation_long_sortedset_db
                                    .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                            }
                            PositionType::SHORT => {
                                let _ = liquidation_short_sortedset_db
                                    .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                            }
                        },
                        OrderStatus::PENDING => match order.position_type {
                            PositionType::LONG => {
                                let _ = open_long_sortedset_db
                                    .add(order.uuid, (order.entryprice * 10000.0) as i64);
                            }
                            PositionType::SHORT => {
                                let _ = open_short_sortedset_db
                                    .add(order.uuid, (order.entryprice * 10000.0) as i64);
                            }
                        },
                        _ => {}
                    }
                }
                RpcCommand::CancelTraderOrder(_rpc_request, _metadata) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
                }
                RpcCommand::ExecuteTraderOrder(_rpc_request, _metadata) => {
                    // let order_clone = order.clone();
                    if orderdb_traderorder
                        .ordertable
                        .contains_key(&order.uuid.clone())
                    {
                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
                    match order.order_status {
                        OrderStatus::SETTLED => match order.position_type {
                            PositionType::LONG => {
                                let _ = liquidation_long_sortedset_db.remove(order.uuid);
                            }
                            PositionType::SHORT => {
                                let _ = liquidation_short_sortedset_db.remove(order.uuid);
                            }
                        },
                        _ => {}
                    }
                }
                RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                    _rpc_request,
                    _metadata,
                    _payment,
                ) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
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
                            let _ = liquidation_long_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = liquidation_short_sortedset_db
                                .add(order.uuid, (order.liquidation_price * 10000.0) as i64);
                        }
                    },
                    _ => {}
                }
            }
            Event::TraderOrderFundingUpdate(order, _cmd) => {
                orderdb_traderorder
                    .ordertable
                    .insert(order.uuid, order.clone());

                match order.position_type {
                    PositionType::LONG => {
                        let _ = liquidation_long_sortedset_db
                            .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .update(order.uuid, (order.liquidation_price * 10000.0) as i64);
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
                }
                // orderdb_traderorder.event.push(data.value);
                if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                    orderdb_traderorder.sequence = order.entry_sequence.clone();
                }
                if orderdb_traderorder.aggrigate_log_sequence < seq {
                    orderdb_traderorder.aggrigate_log_sequence = seq;
                }
            }
            Event::Stop(timex) => {
                if timex == event_stoper_string {
                    stop_signal = false;
                }
            }
            Event::LendOrder(order, cmd, seq) => match cmd {
                RpcCommand::CreateLendOrder(..) => {
                    let order_clone = order.clone();
                    orderdb_lendrorder.ordertable.insert(order.uuid, order);
                    // orderdb_lendrorder.event.push(data.value);
                    if orderdb_lendrorder.sequence < order_clone.entry_sequence {
                        orderdb_lendrorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_lendrorder.aggrigate_log_sequence < seq {
                        orderdb_lendrorder.aggrigate_log_sequence = seq;
                    }
                }
                RpcCommand::ExecuteLendOrder(..) => {
                    // orderdb_lendrorder.event.push(data.value);

                    if orderdb_lendrorder.ordertable.contains_key(&order.uuid) {
                        orderdb_lendrorder.ordertable.remove(&order.uuid);
                    }
                    if orderdb_lendrorder.aggrigate_log_sequence < seq {
                        orderdb_lendrorder.aggrigate_log_sequence = seq;
                    }
                    if orderdb_lendrorder.sequence < order.entry_sequence {
                        orderdb_lendrorder.sequence = order.entry_sequence;
                    }
                }
                _ => {}
            },
            Event::PoolUpdate(cmd, seq) => match cmd.clone() {
                LendPoolCommand::InitiateNewPool(lend_order, _metadata) => {
                    let total_pool_share = lend_order.deposit;
                    let total_locked_value = lend_order.deposit * 10000.0;
                    if lendpool_database.sequence < lend_order.entry_sequence {
                        lendpool_database.sequence = lend_order.entry_sequence;
                    }
                    if lendpool_database.nonce < lend_order.entry_nonce {
                        lendpool_database.nonce = lend_order.entry_nonce;
                    }
                    lendpool_database.total_pool_share += total_pool_share;
                    lendpool_database.total_locked_value += total_locked_value;
                    // lendpool_database.event_log.push(data.value);
                    if lendpool_database.aggrigate_log_sequence < seq {
                        lendpool_database.aggrigate_log_sequence = seq;
                    }
                }
                LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    lendpool_database.total_locked_value += deposit * 10000.0;
                    lendpool_database.total_pool_share += lend_order.npoolshare;
                    // lendpool_database.event_log.push(data.value);
                }
                LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    lendpool_database.total_locked_value -= withdraw;
                    lendpool_database.total_pool_share -= lend_order.npoolshare;
                    // lendpool_database.event_log.push(data.value);
                }
                LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                    lendpool_database.nonce += 1;
                    lendpool_database.aggrigate_log_sequence += 1;
                    match cmd {
                        RelayerCommand::FundingCycle(batch, _metadata, _fundingrate) => {
                            lendpool_database.total_locked_value -= batch.amount * 10000.0;
                        }
                        RelayerCommand::RpcCommandPoolupdate() => {
                            let batch = lendpool_database.pending_orders.clone();
                            lendpool_database.total_locked_value -= batch.amount * 10000.0;
                            lendpool_database.pending_orders = PoolBatchOrder::new();
                        }
                        _ => {}
                    }
                }
                LendPoolCommand::AddFundingData(..) => {}
                LendPoolCommand::AddTraderOrderSettlement(..) => {}
                LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                LendPoolCommand::AddTraderOrderLiquidation(..) => {}
            },
            Event::FundingRateUpdate(funding_rate, _time) => {
                set_localdb("FundingRate", funding_rate);
            }
            Event::CurrentPriceUpdate(current_price, _time) => {
                set_localdb("CurrentPrice", current_price);
            }
            Event::SortedSetDBUpdate(cmd) => match cmd {
                SortedSetCommand::AddOpenLimitPrice(order_id, entry_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db
                                .add(order_id, (entry_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db
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
                        let _sortedset_ = liquidation_long_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .add(order_id, (liquidation_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::AddCloseLimitPrice(order_id, execution_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db
                                .add(order_id, (execution_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db
                                .add(order_id, (execution_price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::RemoveOpenLimitPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::RemoveLiquidationPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = liquidation_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = liquidation_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::RemoveCloseLimitPrice(order_id, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db.remove(order_id);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db.remove(order_id);
                        }
                    }
                }
                SortedSetCommand::UpdateOpenLimitPrice(order_id, entry_price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db
                                .update(order_id, (entry_price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db
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
                        let _ = liquidation_long_sortedset_db
                            .update(order_id, (liquidation_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = liquidation_short_sortedset_db
                            .update(order_id, (liquidation_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::UpdateCloseLimitPrice(
                    order_id,
                    execution_price,
                    position_type,
                ) => match position_type {
                    PositionType::LONG => {
                        let _ = close_long_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                    PositionType::SHORT => {
                        let _ = close_short_sortedset_db
                            .update(order_id, (execution_price * 10000.0) as i64);
                    }
                },
                SortedSetCommand::BulkSearchRemoveOpenLimitPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = open_long_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = open_short_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::BulkSearchRemoveCloseLimitPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ = close_long_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ = close_short_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                    }
                }
                SortedSetCommand::BulkSearchRemoveLiquidationPrice(price, position_type) => {
                    match position_type {
                        PositionType::LONG => {
                            let _ =
                                liquidation_long_sortedset_db.search_gt((price * 10000.0) as i64);
                        }
                        PositionType::SHORT => {
                            let _ =
                                liquidation_short_sortedset_db.search_lt((price * 10000.0) as i64);
                        }
                    }
                }
            },
            Event::PositionSizeLogDBUpdate(_cmd, event) => {
                // position_size_log.total_long_positionsize = event.total_long_positionsize;
                // position_size_log.totalpositionsize = event.totalpositionsize;
                // position_size_log.total_short_positionsize = event.total_short_positionsize;
                position_size_log = event;
            }
        }
    }
    if orderdb_traderorder.sequence > 0 {
        println!("TraderOrder Database Loaded ....");
    } else {
        println!("No old TraderOrder Database found ....\nCreating new TraderOrder_database");
    }
    if orderdb_lendrorder.sequence > 0 {
        println!("LendOrder Database Loaded ....");
    } else {
        println!("No old LendOrder Database found ....\nCreating new LendOrder_database");
    }
    if lendpool_database.aggrigate_log_sequence > 0 {
        println!("LendPool Database Loaded ....");
    } else {
        lendpool_database = LendPool::new();
        println!("No old LendPool Database found ....\nCreating new LendPool_database");
    }

    SnapshotDB {
        orderdb_traderorder: orderdb_traderorder.clone(),
        orderdb_lendrorder: orderdb_lendrorder.clone(),
        lendpool_database: lendpool_database.clone(),
        liquidation_long_sortedset_db: liquidation_long_sortedset_db.clone(),
        liquidation_short_sortedset_db: liquidation_short_sortedset_db.clone(),
        open_long_sortedset_db: open_long_sortedset_db.clone(),
        open_short_sortedset_db: open_short_sortedset_db.clone(),
        close_long_sortedset_db: close_long_sortedset_db.clone(),
        close_short_sortedset_db: close_short_sortedset_db.clone(),
        position_size_log: position_size_log.clone(),
    }
    // (
    //     orderdb_traderorder.clone(),
    //     orderdb_lendrorder.clone(),
    //     lendpool_database.clone(),
    //     liquidation_long_sortedset_db.clone(),
    //     liquidation_short_sortedset_db.clone(),
    //     open_long_sortedset_db.clone(),
    //     open_short_sortedset_db.clone(),
    //     close_long_sortedset_db.clone(),
    //     close_short_sortedset_db.clone(),
    //     position_size_log.clone(),
    // )
}
