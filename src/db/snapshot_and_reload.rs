#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
// use bincode::{config, Decode, Encode};
use bincode;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;
lazy_static! {
    pub static ref SNAPSHOT_DATA: Arc<Mutex<SnapshotDB>> = Arc::new(Mutex::new(SnapshotDB::new()));
}
pub fn load_backup_data() -> (OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) {
    // fn load_data() -> (bool, Self) {
    let mut orderdb_traderorder: OrderDB<TraderOrder> = LocalDB::<TraderOrder>::new();
    let mut orderdb_lendorder: OrderDB<LendOrder> = LocalDB::<LendOrder>::new();
    let mut lendpool_database: LendPool = LendPool::default();

    let mut liquidation_long_sortedset_db = TRADER_LP_LONG.lock().unwrap();
    let mut liquidation_short_sortedset_db = TRADER_LP_SHORT.lock().unwrap();
    let mut open_long_sortedset_db = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
    let mut open_short_sortedset_db = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
    let mut close_long_sortedset_db = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
    let mut close_short_sortedset_db = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
    let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
    let mut output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    let eventstop: Event = Event::Stop(time.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut stop_signal: bool = true;

    let recever = Event::receive_event_for_snapshot_from_kafka_queue(
        CORE_EVENT_LOG.clone().to_string(),
        ServerTime::now().epoch,
        FetchOffset::Earliest,
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
                RpcCommand::CreateTraderOrder(
                    _rpc_request,
                    _metadata,
                    zkos_hex_string,
                    _request_id,
                ) => {
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
                    orderdb_traderorder
                        .zkos_msg
                        .insert(order.uuid, zkos_hex_string);
                    let _ = orderdb_traderorder.set_order_check(order.account_id);
                }
                RpcCommand::CancelTraderOrder(
                    _rpc_request,
                    _metadata,
                    _zkos_hex_string,
                    _request_id,
                ) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
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
                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                    let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                    let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                RpcCommand::CreateLendOrder(
                    _rpc_request,
                    _metadata,
                    zkos_hex_string,
                    _request_id,
                ) => {
                    let order_clone = order.clone();
                    orderdb_lendorder
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
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
                    let _ = orderdb_lendorder.set_order_check(order_clone.account_id);
                }
                RpcCommand::ExecuteLendOrder(..) => {
                    // orderdb_lendorder.event.push(data.value);

                    if orderdb_lendorder.ordertable.contains_key(&order.uuid) {
                        orderdb_lendorder.ordertable.remove(&order.uuid);
                        let _removed_zkos_msg = orderdb_lendorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_lendorder.remove_order_check(order.account_id);
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
                // match cmd.clone() {
                //     LendPoolCommand::InitiateNewPool(lend_order, _metadata, _payemnt) => {
                //         let total_pool_share = lend_order.deposit;
                //         let total_locked_value = lend_order.deposit * 100000000.0;
                //         if lendpool_database.sequence < lend_order.entry_sequence {
                //             lendpool_database.sequence = lend_order.entry_sequence;
                //         }
                //         if lendpool_database.nonce < lend_order.entry_nonce {
                //             lendpool_database.nonce = lend_order.entry_nonce;
                //         }
                //         lendpool_database.total_pool_share = total_pool_share;
                //         lendpool_database.total_locked_value = total_locked_value;
                //         // lendpool_database.event_log.push(data.value);
                //         if lendpool_database.aggrigate_log_sequence < seq {
                //             lendpool_database.aggrigate_log_sequence = seq;
                //         }
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //     }
                //     LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         lendpool_database.total_locked_value += deposit * 10000.0;
                //         lendpool_database.total_pool_share += lend_order.npoolshare;
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //         // lendpool_database.event_log.push(data.value);
                //     }
                //     LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         lendpool_database.total_locked_value -= withdraw;
                //         lendpool_database.total_pool_share -= lend_order.npoolshare;
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //         // lendpool_database.event_log.push(data.value);
                //     }
                //     LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         match cmd {
                //             RelayerCommand::FundingCycle(batch, _metadata, _fundingrate) => {
                //                 lendpool_database.total_locked_value -= batch.amount * 10000.0;
                //             }
                //             RelayerCommand::RpcCommandPoolupdate() => {
                //                 let batch = lendpool_database.pending_orders.clone();
                //                 lendpool_database.total_locked_value -= batch.amount * 10000.0;
                //                 lendpool_database.pending_orders = PoolBatchOrder::new();
                //             }
                //             _ => {}
                //         }
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //     }
                //     LendPoolCommand::AddFundingData(..) => {}
                //     LendPoolCommand::AddTraderOrderSettlement(..) => {}
                //     LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                //     LendPoolCommand::AddTraderOrderLiquidation(..) => {}
                // }
                if lend_pool.nonce > lendpool_database.nonce {
                    lendpool_database = lend_pool;
                }
            }
            Event::FundingRateUpdate(funding_rate, _current_price, _time) => {
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
            Event::TxHash(
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
                            Some(output_hex_string) => match hex::decode(output_hex_string) {
                                Ok(output_byte) => match bincode::deserialize(&output_byte) {
                                    Ok(output_memo) => Some(output_memo),
                                    Err(_) => None,
                                },
                                Err(_) => None,
                            },
                            None => None,
                        };

                        match output_memo_option {
                            Some(output_memo) => {
                                let _ = output_hex_storage.add(uuid_to_byte, Some(output_memo), 0);
                            }
                            None => {}
                        }
                    }
                    OrderStatus::SETTLED | OrderStatus::CANCELLED | OrderStatus::LIQUIDATE => {
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
        }
    }
    drop(liquidation_long_sortedset_db);
    drop(liquidation_short_sortedset_db);
    drop(open_long_sortedset_db);
    drop(open_short_sortedset_db);
    drop(close_long_sortedset_db);
    drop(close_short_sortedset_db);
    drop(position_size_log);
    let _ = output_hex_storage.take_snapshot();
    drop(output_hex_storage);
    if orderdb_traderorder.sequence > 0 {
        println!("TraderOrder Database Loaded ....");
    } else {
        println!("No old TraderOrder Database found ....\nCreating new TraderOrder_database");
    }
    if orderdb_lendorder.sequence > 0 {
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
        orderdb_lendorder.clone(),
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
    pub position_size_log: PositionSizeLog,
    pub localdb_hashmap: HashMap<String, f64>,
    // pub output_memo_hashmap: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub event_offset: i64,
    pub event_timestamp: String,
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
            position_size_log: PositionSizeLog::new(),
            localdb_hashmap: HashMap::new(),
            // output_memo_hashmap:
            //     utxo_in_memory::db::LocalStorage::<Option<zkvm::zkos_types::Output>>::new(1),
            event_offset: 0,
            event_timestamp: ServerTime::now().epoch,
        }
    }
    fn print_status(&self) {
        if self.orderdb_traderorder.sequence > 0 {
            println!("TraderOrder Database Loaded ....");
        } else {
            println!("No old TraderOrder Database found ....\nCreating new TraderOrder_database");
        }
        if self.orderdb_lendorder.sequence > 0 {
            println!("LendOrder Database Loaded ....");
        } else {
            println!("No old LendOrder Database found ....\nCreating new LendOrder_database");
        }
        if self.lendpool_database.aggrigate_log_sequence > 0 {
            println!("LendPool Database Loaded ....");
        } else {
            println!("No old LendPool Database found ....\nCreating new LendPool_database");
        }
    }
}

pub fn snapshot() -> Result<(), std::io::Error> {
    // let read_snapshot = fs::read("snapshot").expect("Could not read file");
    // snapshot renaming on success
    // encryption on snapshot data
    // snapshot version
    // delete old snapshot data deleted by cron job
    let read_snapshot = fs::read(format!(
        "{}-{}",
        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
    ));
    let decoded_snapshot: SnapshotDB;
    let mut is_file_exist = false;
    let last_snapshot_time: String;
    let fetchoffset: FetchOffset;
    match read_snapshot {
        Ok(snapshot_data_from_file) => {
            decoded_snapshot =
                bincode::deserialize(&snapshot_data_from_file).expect("Could not decode vector");
            is_file_exist = true;
            last_snapshot_time = decoded_snapshot.event_timestamp.clone();
            fetchoffset = FetchOffset::Earliest;
        }
        Err(arg) => {
            println!(
                "No previous Snapshot Found- Error:{:#?} \n path: {:?}",
                arg,
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
            );
            decoded_snapshot = SnapshotDB::new();
            last_snapshot_time = decoded_snapshot.event_timestamp.clone();
            fetchoffset = FetchOffset::Earliest;
        }
    }
    let mut snapshot_data = SNAPSHOT_DATA.lock().unwrap();
    *snapshot_data = decoded_snapshot.clone();
    drop(snapshot_data);
    let mut output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
    let _ = output_hex_storage.load_from_snapshot();
    drop(output_hex_storage);
    let snapshot_db_updated = create_snapshot_data(fetchoffset);

    let mut snapshot_data = SNAPSHOT_DATA.lock().unwrap();
    *snapshot_data = snapshot_db_updated.clone();

    let encoded_v = bincode::serialize(&snapshot_db_updated).expect("Could not encode vector");
    match fs::write(
        format!(
            "{}-{}-new",
            *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
        ),
        encoded_v,
    ) {
        Ok(_) => {
            if is_file_exist {
                fs::rename(
                    format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
                    format!(
                        "{}-{}-{}",
                        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION, last_snapshot_time
                    ),
                )
                .unwrap();
            }
            fs::rename(
                format!(
                    "{}-{}-new",
                    *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                ),
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
            )
            .unwrap();
        }
        Err(arg) => {
            println!("Could not write snapshot file - Error:{:#?}", arg);
        }
    }
    // println!("Snapshot:{:#?}", snapshot_db_updated);

    Ok(())
}

pub fn create_snapshot_data(fetchoffset: FetchOffset) -> SnapshotDB {
    let snapshot_db = SNAPSHOT_DATA.lock().unwrap().clone();
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
        mut position_size_log,
        mut localdb_hashmap,
        mut event_offset,
        event_timestamp: _,
    } = snapshot_db;

    let mut output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
    // let mut event_offset: i64 = 0;
    let time = ServerTime::now().epoch;
    let event_timestamp = time.clone();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut stop_signal: bool = true;

    let recever = Event::receive_event_for_snapshot_from_kafka_queue(
        CORE_EVENT_LOG.clone().to_string(),
        format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
        fetchoffset,
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
                    orderdb_traderorder
                        .zkos_msg
                        .insert(order.uuid, zkos_hex_string);
                    let _ = orderdb_traderorder.set_order_check(order.account_id);
                }
                RpcCommand::CancelTraderOrder(
                    _rpc_request,
                    _metadata,
                    _zkos_hex_string,
                    _request_id,
                ) => {
                    let order_clone = order.clone();
                    if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                        orderdb_traderorder.ordertable.remove(&order.uuid);
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
                    }
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
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
                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                        let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                    let _removed_zkos_msg = orderdb_traderorder.zkos_msg.remove(&order.uuid);
                    let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                    event_offset = data.offset;
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
                    let _ = orderdb_traderorder.set_order_check(order_clone.account_id);
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
                        let _removed_zkos_msg = orderdb_lendorder.zkos_msg.remove(&order.uuid);
                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
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
                // match cmd.clone() {
                //     LendPoolCommand::InitiateNewPool(lend_order, _metadata, _payment) => {
                //         let total_pool_share = lend_order.deposit;
                //         let total_locked_value = lend_order.deposit * 100000000.0;
                //         if lendpool_database.sequence < lend_order.entry_sequence {
                //             lendpool_database.sequence = lend_order.entry_sequence;
                //         }
                //         if lendpool_database.nonce < lend_order.entry_nonce {
                //             lendpool_database.nonce = lend_order.entry_nonce;
                //         }
                //         lendpool_database.total_pool_share = total_pool_share;
                //         lendpool_database.total_locked_value = total_locked_value;
                //         // lendpool_database.event_log.push(data.value);
                //         if lendpool_database.aggrigate_log_sequence < seq {
                //             lendpool_database.aggrigate_log_sequence = seq;
                //         }
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //     }
                //     LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         lendpool_database.total_locked_value += deposit * 10000.0;
                //         lendpool_database.total_pool_share += lend_order.npoolshare;
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //         // lendpool_database.event_log.push(data.value);
                //     }
                //     LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         lendpool_database.total_locked_value -= withdraw;
                //         lendpool_database.total_pool_share -= lend_order.npoolshare;
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //         // lendpool_database.event_log.push(data.value);
                //     }
                //     LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                //         lendpool_database.nonce += 1;
                //         lendpool_database.aggrigate_log_sequence += 1;
                //         match cmd {
                //             RelayerCommand::FundingCycle(batch, _metadata, _fundingrate) => {
                //                 lendpool_database.total_locked_value -= batch.amount * 10000.0;
                //             }
                //             RelayerCommand::RpcCommandPoolupdate() => {
                //                 let batch = lendpool_database.pending_orders.clone();
                //                 lendpool_database.total_locked_value -= batch.amount * 10000.0;
                //                 lendpool_database.pending_orders = PoolBatchOrder::new();
                //             }
                //             _ => {}
                //         }
                //         lendpool_database.last_output_state = lend_pool.clone().last_output_state;
                //     }
                //     LendPoolCommand::AddFundingData(..) => {}
                //     LendPoolCommand::AddTraderOrderSettlement(..) => {}
                //     LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                //     LendPoolCommand::AddTraderOrderLiquidation(..) => {}
                // }

                if lend_pool.nonce > lendpool_database.nonce {
                    lendpool_database = lend_pool;
                }
            }
            Event::FundingRateUpdate(funding_rate, _current_price, _time) => {
                // set_localdb("FundingRate", funding_rate);
                localdb_hashmap.insert("FundingRate".to_string(), funding_rate);
            }
            Event::CurrentPriceUpdate(current_price, _time) => {
                // set_localdb("CurrentPrice", current_price);
                localdb_hashmap.insert("CurrentPrice".to_string(), current_price);
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
            Event::TxHash(
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
                            Some(output_hex_string) => match hex::decode(output_hex_string) {
                                Ok(output_byte) => match bincode::deserialize(&output_byte) {
                                    Ok(output_memo) => Some(output_memo),
                                    Err(_) => None,
                                },
                                Err(_) => None,
                            },
                            None => None,
                        };
                        match output_memo_option {
                            Some(output_memo) => {
                                let _ = output_hex_storage.add(uuid_to_byte, Some(output_memo), 0);
                            }
                            None => {}
                        }
                    }
                    OrderStatus::SETTLED | OrderStatus::CANCELLED | OrderStatus::LIQUIDATE => {
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
        }
    }
    let _ = output_hex_storage.take_snapshot();
    drop(output_hex_storage);

    if lendpool_database.aggrigate_log_sequence > 0 {
    } else {
        lendpool_database = LendPool::new();
    }

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
        position_size_log: position_size_log.clone(),
        localdb_hashmap: localdb_hashmap,
        event_offset: event_offset,
        event_timestamp: event_timestamp,
    }
}

pub fn load_from_snapshot() {
    match snapshot() {
        Ok(_) => {
            let snapshot_data = SNAPSHOT_DATA.lock().unwrap();
            let mut liquidation_long_sortedset_db = TRADER_LP_LONG.lock().unwrap();
            let mut liquidation_short_sortedset_db = TRADER_LP_SHORT.lock().unwrap();
            let mut open_long_sortedset_db = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
            let mut open_short_sortedset_db = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
            let mut close_long_sortedset_db = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
            let mut close_short_sortedset_db = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
            let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
            let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
            let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
            let mut load_pool_data = LEND_POOL_DB.lock().unwrap();
            snapshot_data.print_status();

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

            let trader_order_handle = thread::Builder::new()
                .name(String::from("trader_order_handle"))
                .spawn(move || {
                    let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
                    for (key, val) in traderorder_hashmap.iter() {
                        load_trader_data
                            .ordertable
                            .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                    }
                })
                .unwrap();
            let lend_order_handle = thread::Builder::new()
                .name(String::from("lend_order_handle"))
                .spawn(move || {
                    let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
                    for (key, val) in lendorder_hashmap.iter() {
                        load_lend_data
                            .ordertable
                            .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                    }
                })
                .unwrap();

            *liquidation_long_sortedset_db = snapshot_data.liquidation_long_sortedset_db.clone();
            *liquidation_short_sortedset_db = snapshot_data.liquidation_short_sortedset_db.clone();
            *open_long_sortedset_db = snapshot_data.open_long_sortedset_db.clone();
            *open_short_sortedset_db = snapshot_data.open_short_sortedset_db.clone();
            *close_long_sortedset_db = snapshot_data.close_long_sortedset_db.clone();
            *close_short_sortedset_db = snapshot_data.close_short_sortedset_db.clone();
            *position_size_log = snapshot_data.position_size_log.clone();
            *load_pool_data = snapshot_data.lendpool_database.clone();
            let current_price = snapshot_data.localdb_hashmap.get("CurrentPrice").clone();
            set_localdb(
                "CurrentPrice",
                match current_price {
                    Some(value) => value.clone(),
                    None => 18000.0,
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
            let fee = snapshot_data.localdb_hashmap.get("Fee").clone();
            set_localdb(
                "Fee",
                match fee {
                    Some(value) => value.clone(),
                    None => 0.0,
                },
            );

            trader_order_handle.join().unwrap();
            lend_order_handle.join().unwrap();
        }

        Err(arg) => {
            println!("unable to load data from snapshot \n error: {:?}", arg);
            let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
            let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
            let mut load_pool_data = LEND_POOL_DB.lock().unwrap();
            let (data1, data2, data3): (OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) =
                load_backup_data();
            *load_trader_data = data1;
            *load_lend_data = data2;
            *load_pool_data = data3;
            drop(load_trader_data);
            drop(load_lend_data);
            drop(load_pool_data);
        }
    }
}
