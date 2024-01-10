#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;
type Payment = f64;
type Deposit = f64;
type Withdraw = f64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LendPool {
    pub sequence: usize,
    pub nonce: usize,
    pub total_pool_share: f64,
    pub total_locked_value: f64,
    pub pending_orders: PoolBatchOrder,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PoolBatchOrder {
    pub nonce: usize,
    pub len: usize,
    pub amount: f64, //sats
    pub trader_order_data: Vec<LendPoolCommand>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LendPoolCommand {
    AddTraderOrderSettlement(RpcCommand, TraderOrder, Payment),
    AddTraderLimitOrderSettlement(RelayerCommand, TraderOrder, Payment),
    AddFundingData(TraderOrder, Payment),
    AddTraderOrderLiquidation(RelayerCommand, TraderOrder, Payment),
    LendOrderCreateOrder(RpcCommand, LendOrder, Deposit),
    LendOrderSettleOrder(RpcCommand, LendOrder, Withdraw),
    BatchExecuteTraderOrder(RelayerCommand),
    InitiateNewPool(LendOrder, Meta),
}

impl PoolBatchOrder {
    pub fn new() -> Self {
        PoolBatchOrder {
            nonce: 0,
            len: 0,
            amount: 0.0, //sats
            trader_order_data: Vec::new(),
        }
    }
    pub fn add(&mut self, payment: f64, order: TraderOrder) {
        self.amount += payment;
        self.trader_order_data
            .push(LendPoolCommand::AddFundingData(order, payment));
        self.len += 1;
    }
}

impl LendPool {
    pub fn default() -> Self {
        LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: 0.0,
            total_locked_value: 0.0,
            // event_log: Vec::new(),
            pending_orders: PoolBatchOrder::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }
    pub fn new() -> Self {
        let relayer_initial_lend_order = LendOrder {
            uuid: Uuid::new_v4(),
            account_id: String::from("Relayer Initial Transaction, with public key"),
            balance: 10.0,
            order_status: OrderStatus::SETTLED,
            order_type: OrderType::LEND,
            entry_nonce: 0,
            exit_nonce: 0,
            deposit: 10.0,
            new_lend_state_amount: 10.0 * 10000.0,
            timestamp: systemtime_to_utc(),
            npoolshare: 10.0,
            nwithdraw: 0.0,
            payment: 0.0,
            tlv0: 0.0,
            tps0: 0.0,
            tlv1: 100000.0,
            tps1: 10.0,
            tlv2: 100000.0,
            tps2: 10.0,
            tlv3: 100000.0,
            tps3: 10.0,
            entry_sequence: 0,
        };
        let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
        let rpc_request = RpcCommand::CreateLendOrder(
            CreateLendOrder {
                account_id: relayer_initial_lend_order.account_id.clone(),
                balance: relayer_initial_lend_order.balance.clone(),
                order_type: relayer_initial_lend_order.order_type.clone(),
                order_status: relayer_initial_lend_order.order_status.clone(),
                deposit: relayer_initial_lend_order.deposit.clone(),
            },
            Meta {
                metadata: {
                    let mut hashmap = HashMap::new();
                    hashmap.insert(
                        String::from("Relayer_default_pool"),
                        Some(String::from("Relayer_default_pool")),
                    );
                    hashmap.insert(
                        String::from("request_server_time"),
                        Some(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                .to_string(),
                        ),
                    );
                    hashmap
                },
            },
            "zkos_hex_string".to_string(),
        );
        lendorder_db.add(relayer_initial_lend_order.clone(), rpc_request);
        drop(lendorder_db);
        let total_pool_share = relayer_initial_lend_order.npoolshare;
        let total_locked_value = relayer_initial_lend_order.deposit * 10000.0;
        let mut metadata = HashMap::new();
        metadata.insert(
            String::from("Relayer_Public_Key"),
            Some(String::from("Relayer Initial Transaction, with public key")),
        );
        metadata.insert(
            String::from("Pool_Initiate_time"),
            Some(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    .to_string(),
            ),
        );

        let relayer_command =
            LendPoolCommand::InitiateNewPool(relayer_initial_lend_order, Meta { metadata });

        let lendpool = LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: total_pool_share,
            total_locked_value: total_locked_value,
            // event_log: {
            //     let mut event_log = Vec::new();
            //     event_log.push(pool_event_execute);
            //     event_log
            // },
            pending_orders: PoolBatchOrder::new(),
            aggrigate_log_sequence: 1,
            last_snapshot_id: 0,
        };
        let pool_event = Event::PoolUpdate(relayer_command.clone(), lendpool.clone(), 1);
        let _pool_event_execute = Event::new(
            pool_event,
            String::from("Initiate_Lend_Pool"),
            LENDPOOL_EVENT_LOG.clone().to_string(),
        );
        lendpool
    }

    pub fn load_data() -> (bool, LendPool) {
        // fn load_data() -> (bool, Self) {
        let mut database: LendPool = LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: 0.0,
            total_locked_value: 0.0,
            // event_log: Vec::new(),
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
            LENDPOOL_EVENT_LOG.clone().to_string(),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = Event::receive_event_from_kafka_queue(
            LENDPOOL_EVENT_LOG.clone().to_string(),
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
                Event::PoolUpdate(cmd, lendpool, seq) => match cmd.clone() {
                    LendPoolCommand::InitiateNewPool(lend_order, _metadata) => {
                        let total_pool_share = lend_order.deposit;
                        let total_locked_value = lend_order.deposit * 10000.0;
                        if database.sequence < lend_order.entry_sequence {
                            database.sequence = lend_order.entry_sequence;
                        }
                        if database.nonce < lend_order.entry_nonce {
                            database.nonce = lend_order.entry_nonce;
                        }
                        database.total_pool_share += total_pool_share;
                        database.total_locked_value += total_locked_value;
                        // database.event_log.push(data.value);
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                    LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value += deposit * 10000.0;
                        database.total_pool_share += lend_order.npoolshare;
                        // database.event_log.push(data.value);
                    }
                    LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value -= withdraw;
                        database.total_pool_share -= lend_order.npoolshare;
                        // database.event_log.push(data.value);
                    }
                    LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        match cmd {
                            RelayerCommand::FundingCycle(batch, _metadata, _fundingrate) => {
                                database.total_locked_value -= batch.amount * 10000.0;
                            }
                            RelayerCommand::RpcCommandPoolupdate() => {
                                let batch = database.pending_orders.clone();
                                database.total_locked_value -= batch.amount * 10000.0;
                                database.pending_orders = PoolBatchOrder::new();
                            }
                            _ => {}
                        }
                    }
                    LendPoolCommand::AddFundingData(..) => {}
                    LendPoolCommand::AddTraderOrderSettlement(..) => {}
                    LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                    LendPoolCommand::AddTraderOrderLiquidation(..) => {}
                },
                Event::Stop(timex) => {
                    if timex == time {
                        // database.aggrigate_log_sequence = database.cmd.len();
                        stop_signal = false;
                    }
                }
                _ => {}
            }
        }
        if database.total_locked_value > 0.0 {
            (true, database.clone())
        } else {
            (false, database)
        }
    }

    pub fn check_backup() -> Self {
        println!("Loading LendPool Database ....");
        let (old_data, database): (bool, LendPool) = LendPool::load_data();
        if old_data {
            println!("LendPool Database Loaded ....");
            database
        } else {
            println!("No old LendPool Database found ....\nCreating new database");
            LendPool::new()
        }
    }

    pub fn make_pool_transaction(&mut self) {}

    pub fn get_nonce(&mut self) -> usize {
        self.nonce.clone()
    }
    pub fn next_nonce(&mut self) -> usize {
        self.nonce += 1;
        self.nonce.clone()
    }

    pub fn add_transaction(&mut self, command: LendPoolCommand) {
        let command_clone = command.clone();
        match command {
            LendPoolCommand::AddTraderOrderSettlement(_rpc_request, _trader_order, payment) => {
                self.pending_orders.len += 1;
                self.aggrigate_log_sequence += 1;
                self.pending_orders.amount += payment * 10000.0;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                Event::new(
                    Event::PoolUpdate(command_clone, self.clone(), self.aggrigate_log_sequence),
                    String::from("AddTraderOrderSettlement"),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                // check pending order length
                //skip batch process code
                self.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(
                    RelayerCommand::RpcCommandPoolupdate(),
                ));
            }
            LendPoolCommand::AddTraderLimitOrderSettlement(
                _relayer_request,
                _trader_order,
                payment,
            ) => {
                self.pending_orders.len += 1;
                self.aggrigate_log_sequence += 1;
                self.pending_orders.amount += payment * 10000.0;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                Event::new(
                    Event::PoolUpdate(command_clone, self.clone(), self.aggrigate_log_sequence),
                    String::from("AddTraderLimitOrderSettlement"),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                // check pending order length
                //skip batch process code
                self.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(
                    RelayerCommand::RpcCommandPoolupdate(),
                ));
            }
            LendPoolCommand::AddTraderOrderLiquidation(_relayer_command, trader_order, payment) => {
                self.pending_orders.len += 1;
                self.pending_orders.amount -= payment * 10000.0;
                self.aggrigate_log_sequence += 1;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                Event::new(
                    Event::PoolUpdate(command_clone, self.clone(), self.aggrigate_log_sequence),
                    String::from(format!("AddTraderOrderLiquidation-{}", trader_order.uuid)),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                // check pending order length
                //skip batch process code
                self.add_transaction(LendPoolCommand::BatchExecuteTraderOrder(
                    RelayerCommand::RpcCommandPoolupdate(),
                ));
            }
            LendPoolCommand::LendOrderCreateOrder(rpc_request, mut lend_order, deposit) => {
                self.nonce += 1;
                self.aggrigate_log_sequence += 1;
                self.total_locked_value += deposit * 10000.0;
                self.total_pool_share += lend_order.npoolshare;
                lend_order.tps1 = self.total_pool_share.clone();
                lend_order.tlv1 = self.total_locked_value.clone();
                lend_order.order_status = OrderStatus::FILLED;
                lend_order.entry_nonce = self.nonce;
                let mut lendpool_clone_with_empty_trade_order = self.clone();
                lendpool_clone_with_empty_trade_order
                    .pending_orders
                    .trader_order_data = Vec::new();
                Event::new(
                    Event::PoolUpdate(
                        command_clone,
                        lendpool_clone_with_empty_trade_order.clone(),
                        self.aggrigate_log_sequence,
                    ),
                    String::from("LendOrderCreateOrder"),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                lendorder_db.add(lend_order.clone(), rpc_request.clone());
                zkos_order_handler(ZkosTxCommand::CreateLendOrderTX(
                    lend_order.clone(),
                    rpc_request,
                ));
                drop(lendorder_db);
            }
            LendPoolCommand::LendOrderSettleOrder(rpc_request, mut lend_order, withdraw) => {
                self.nonce += 1;
                self.aggrigate_log_sequence += 1;
                // self.total_locked_value -= withdraw * 10000.0;
                self.total_locked_value -= withdraw;
                self.total_pool_share -= lend_order.npoolshare;
                lend_order.tps3 = self.total_pool_share;
                lend_order.tlv3 = self.total_locked_value;
                lend_order.order_status = OrderStatus::SETTLED;
                lend_order.exit_nonce = self.nonce;
                Event::new(
                    Event::PoolUpdate(
                        LendPoolCommand::LendOrderSettleOrder(
                            rpc_request.clone(),
                            lend_order.clone(),
                            withdraw.clone(),
                        ),
                        self.clone(),
                        self.aggrigate_log_sequence,
                    ),
                    String::from("LendOrderSettleOrder"),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                let _ = lendorder_db.remove(lend_order.clone(), rpc_request.clone());
                drop(lendorder_db);
                zkos_order_handler(ZkosTxCommand::ExecuteLendOrderTX(
                    lend_order.clone(),
                    rpc_request,
                ));
            }
            LendPoolCommand::BatchExecuteTraderOrder(relayer_command) => match relayer_command {
                RelayerCommand::FundingCycle(pool_batch_order, _metadata, _fundingrate) => {
                    self.nonce += 1;
                    self.aggrigate_log_sequence += 1;
                    self.total_locked_value -= pool_batch_order.amount * 10000.0;
                    // self.event_log.push(Event::new(
                    //     Event::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    //     String::from("FundingCycleDataUpdate"),
                    //     LENDPOOL_EVENT_LOG.clone().to_string(),
                    // ));
                }
                RelayerCommand::RpcCommandPoolupdate() => {
                    let batch = self.pending_orders.clone();
                    if batch.trader_order_data.len() > 0 {
                        self.nonce += 1;
                        self.aggrigate_log_sequence += 1;

                        self.total_locked_value -= batch.amount;

                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                        for cmd in batch.trader_order_data {
                            // let cmd_clone = cmd.clone();
                            match cmd {
                                LendPoolCommand::AddTraderOrderSettlement(
                                    rpc_cmd,
                                    mut order,
                                    _payment,
                                ) => {
                                    // println!("hey, im here");
                                    order.exit_nonce = self.nonce;
                                    let _ = trader_order_db.remove(order, rpc_cmd);
                                }
                                LendPoolCommand::AddTraderLimitOrderSettlement(
                                    relayer_cmd,
                                    mut order,
                                    _payment,
                                ) => match relayer_cmd {
                                    RelayerCommand::PriceTickerOrderSettle(
                                        _,
                                        metadata,
                                        current_price,
                                    ) => {
                                        order.exit_nonce = self.nonce;
                                        let dummy_rpccommand =
                                            RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                                                order.clone(),
                                                metadata,
                                                current_price,
                                            );
                                        let _ =
                                            trader_order_db.remove(order.clone(), dummy_rpccommand);
                                    }
                                    _ => {}
                                },
                                LendPoolCommand::AddTraderOrderLiquidation(
                                    relayer_cmd,
                                    mut order,
                                    _payment,
                                ) => {
                                    order.exit_nonce = self.nonce;
                                    let _ = trader_order_db.liquidate(order, relayer_cmd);
                                }
                                _ => {}
                            }
                        }
                        drop(trader_order_db);
                        self.pending_orders.trader_order_data = Vec::new();
                        Event::new(
                            Event::PoolUpdate(
                                command_clone,
                                self.clone(),
                                self.aggrigate_log_sequence,
                            ),
                            String::from(format!("RpcCommandPoolupdate-nonce-{}", self.nonce)),
                            LENDPOOL_EVENT_LOG.clone().to_string(),
                        );
                        self.pending_orders = PoolBatchOrder::new();
                    }
                }
                _ => {}
            },
            LendPoolCommand::InitiateNewPool(_lend_order, _smetadata) => {
                // self.aggrigate_log_sequence += 1;
            }
            LendPoolCommand::AddFundingData(..) => {}
        }
    }

    pub fn get_lendpool(&mut self) -> (f64, f64) {
        (self.total_locked_value, self.total_pool_share)
    }
}
