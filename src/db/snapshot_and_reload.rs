#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
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

fn load_data() -> (bool, OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) {
    // fn load_data() -> (bool, Self) {
    let mut orderdb_traderorder: OrderDB<TraderOrder> = LocalDB::<TraderOrder>::new();
    let mut orderdb_lendrorder: OrderDB<LendOrder> = LocalDB::<LendOrder>::new();
    let mut lendpool_database: LendPool = LendPool {
        sequence: 0,
        nonce: 0,
        total_pool_share: 0.0,
        total_locked_value: 0.0,
        event_log: Vec::new(),
        pending_orders: PoolBatchOrder::new(),
        aggrigate_log_sequence: 0,
        last_snapshot_id: 0,
    };
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
        match data.value.clone() {
            Event::TraderOrder(order, cmd, seq) => match cmd {
                RpcCommand::ExecuteTraderOrder(_rpc_request, _metadata) => {
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
                RpcCommand::RelayerCommandTraderOrderOnLimit(_rpc_request, _metadata, _payment) => {
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
                _ => {
                    let order_clone = order.clone();
                    orderdb_traderorder
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                    // orderdb_traderorder.event.push(data.value);
                    if orderdb_traderorder.sequence < order_clone.entry_sequence {
                        orderdb_traderorder.sequence = order_clone.entry_sequence;
                    }
                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                        orderdb_traderorder.aggrigate_log_sequence = seq;
                    }
                }
            },
            Event::TraderOrderUpdate(order, _cmd, seq) => {
                let order_clone = order.clone();
                orderdb_traderorder
                    .ordertable
                    .insert(order.uuid, Arc::new(RwLock::new(order)));
                // orderdb_traderorder.event.push(data.value);
                if orderdb_traderorder.sequence < order_clone.entry_sequence {
                    orderdb_traderorder.sequence = order_clone.entry_sequence;
                }
                if orderdb_traderorder.aggrigate_log_sequence < seq {
                    orderdb_traderorder.aggrigate_log_sequence = seq;
                }
            }
            Event::TraderOrderFundingUpdate(order, _cmd) => {
                orderdb_traderorder
                    .ordertable
                    .insert(order.uuid, Arc::new(RwLock::new(order)));
            }
            Event::TraderOrderLiquidation(order, _cmd, seq) => {
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
        }
    }
    if orderdb_traderorder.sequence > 0 {
        println!("TraderOrder Database Loaded ....");
    } else {
        println!("No old TraderOrder Database found ....\nCreating new lendpool_database");
    }
    if orderdb_lendrorder.sequence > 0 {
        println!("LendOrder Database Loaded ....");
    } else {
        println!("No old LendOrder Database found ....\nCreating new lendpool_database");
    }
    if lendpool_database.total_locked_value > 0.0 {
        println!("LendPool Database Loaded ....");
    } else {
        println!("No old LendPool Database found ....\nCreating new database");
    }
    (
        true,
        orderdb_traderorder.clone(),
        orderdb_lendrorder.clone(),
        lendpool_database.clone(),
    )
}
