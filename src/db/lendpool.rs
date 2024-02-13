#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use curve25519_dalek::scalar::Scalar;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;
use zkvm::Output;
type Payment = f64;
type Deposit = f64;
type Withdraw = f64;
type PoolLockError = i128;
type Nonce = usize;
use relayerwalletlib::zkoswalletlib::util::{
    create_output_state_for_trade_lend_order, get_state_info_from_output_hex,
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LendPool {
    pub sequence: usize,
    pub nonce: usize,
    pub total_pool_share: f64,
    pub total_locked_value: f64,
    pub pending_orders: PoolBatchOrder,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub last_output_state: Output,
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
    AddTraderOrderSettlement(RpcCommand, TraderOrder, Payment, Output),
    AddTraderLimitOrderSettlement(RelayerCommand, TraderOrder, Payment, Output),
    AddFundingData(TraderOrder, Payment),
    AddTraderOrderLiquidation(RelayerCommand, TraderOrder, Payment),
    LendOrderCreateOrder(RpcCommand, LendOrder, Deposit, Output),
    LendOrderSettleOrder(RpcCommand, LendOrder, Withdraw, Output),
    BatchExecuteTraderOrder(RelayerCommand),
    InitiateNewPool(LendOrder, Meta, Payment),
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
        let last_output_state = last_state_output_fixed();

        LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: 0.0,
            total_locked_value: 0.0,
            pending_orders: PoolBatchOrder::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            last_output_state,
        }
    }
    pub fn new() -> Self {
        let mut tlv_init: f64 = 20048621560.0 / 100000000.0;
        let mut tps_init: f64 = 2000000.0;
        let mut nonce_init = 7;
        let (nonce, tlv_witness, _, tps_witness, _) =
            match get_state_info_from_output_hex(last_state_output_string()) {
                Ok((nonce, tlv_witness, _tlv_blinding, tps_witness, _tps_blinding)) => (
                    nonce,
                    tlv_witness,
                    _tlv_blinding,
                    tps_witness,
                    _tps_blinding,
                ),
                Err(_arg) => (
                    nonce_init,
                    tlv_init.round() as u64,
                    Scalar::random(&mut rand::thread_rng()),
                    tps_init.round() as u64,
                    Scalar::random(&mut rand::thread_rng()),
                ),
            };
        nonce_init = nonce;
        tps_init = tps_witness as f64;
        tlv_init = tlv_witness as f64;
        let last_output_state = last_state_output_fixed();
        let aggrigate_log_sequence_init = 8;
        let relayer_initial_lend_order = LendOrder {
            uuid: Uuid::new_v4(),
            account_id: last_output_state
                .clone()
                .as_output_data()
                .get_owner_address()
                .clone()
                .unwrap()
                .clone(),
            balance: tlv_init,
            order_status: OrderStatus::SETTLED,
            order_type: OrderType::LEND,
            entry_nonce: 0,
            exit_nonce: 1 as usize,
            deposit: tlv_init,
            new_lend_state_amount: tlv_init * 100000000.0,
            timestamp: systemtime_to_utc(),
            npoolshare: tps_init * 10000.0,
            nwithdraw: 0.0,
            payment: 0.0,
            tlv0: 0.0,
            tps0: 0.0,
            tlv1: tlv_init * 100000000.0,
            tps1: tps_init,
            tlv2: tlv_init * 100000000.0,
            tps2: tps_init,
            tlv3: tlv_init * 100000000.0,
            tps3: tps_init,
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
            last_state_output_string(),
        );
        lendorder_db.add(relayer_initial_lend_order.clone(), rpc_request);
        drop(lendorder_db);
        let total_pool_share = relayer_initial_lend_order.npoolshare / 10000.0;
        let total_locked_value = relayer_initial_lend_order.deposit * 100000000.0;
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

        let relayer_command = LendPoolCommand::InitiateNewPool(
            relayer_initial_lend_order,
            Meta { metadata },
            total_locked_value,
        );

        //need to pick from env variable later

        let lendpool = LendPool {
            sequence: 0,
            nonce: nonce_init as usize,
            total_pool_share: total_pool_share.round(),
            total_locked_value: (tlv_init * 100000000.0).round(),
            pending_orders: PoolBatchOrder::new(),
            aggrigate_log_sequence: aggrigate_log_sequence_init,
            last_snapshot_id: 0,
            last_output_state,
        };
        let pool_event = Event::PoolUpdate(
            relayer_command.clone(),
            lendpool.clone(),
            nonce_init as usize,
        );
        let _pool_event_execute = Event::new(
            pool_event,
            String::from("Initiate_Lend_Pool"),
            LENDPOOL_EVENT_LOG.clone().to_string(),
        );
        lendpool
    }

    pub fn load_data() -> (bool, LendPool) {
        // fn load_data() -> (bool, Self) {
        let mut database: LendPool = LendPool::default();

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
                    LendPoolCommand::InitiateNewPool(lend_order, _metadata, _payment) => {
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
                    LendPoolCommand::LendOrderCreateOrder(
                        _rpc_request,
                        lend_order,
                        deposit,
                        next_output_state,
                    ) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value += deposit * 10000.0;
                        database.total_pool_share += lend_order.npoolshare;
                        database.last_output_state = next_output_state;
                        // database.event_log.push(data.value);
                    }
                    LendPoolCommand::LendOrderSettleOrder(
                        _rpc_request,
                        lend_order,
                        withdraw,
                        next_output_state,
                    ) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value -= withdraw;
                        database.total_pool_share -= lend_order.npoolshare;
                        database.last_output_state = next_output_state;
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

                                database = lendpool;
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
            LendPoolCommand::AddTraderOrderSettlement(
                _rpc_request,
                _trader_order,
                payment,
                _next_output_state,
            ) => {
                self.pending_orders.len += 1;
                self.aggrigate_log_sequence += 1;
                // self.pending_orders.amount += payment * 10000.0;
                self.pending_orders.amount += payment;
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
                _rel_cmd,
                _trader_order,
                payment,
                _next_output_state,
            ) => {
                self.pending_orders.len += 1;
                self.aggrigate_log_sequence += 1;
                self.pending_orders.amount += payment;
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
                self.pending_orders.amount -= payment;
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

            LendPoolCommand::LendOrderCreateOrder(
                rpc_request,
                mut lend_order,
                deposit,
                next_output_state,
            ) => {
                self.nonce += 1;
                self.aggrigate_log_sequence += 1;
                self.total_locked_value += deposit.round();
                self.total_pool_share += (lend_order.npoolshare / 10000.0).round();
                lend_order.tps1 = self.total_pool_share.clone();
                lend_order.tlv1 = self.total_locked_value.clone();
                lend_order.order_status = OrderStatus::FILLED;
                lend_order.entry_nonce = self.nonce;
                let mut lendpool_clone_with_empty_trade_order = self.clone();
                lendpool_clone_with_empty_trade_order
                    .pending_orders
                    .trader_order_data = Vec::new();

                // zkos_order_handler(ZkosTxCommand::CreateLendOrderTX(
                //     lend_order.clone(),
                //     rpc_request.clone(),
                //     self.last_output_state.clone(),
                //     next_output_state.clone(),
                // ));
                self.last_output_state = next_output_state;
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
                drop(lendorder_db);
            }

            LendPoolCommand::LendOrderSettleOrder(
                rpc_request,
                mut lend_order,
                nwithdraw,
                next_output_state,
            ) => {
                self.nonce += 1;
                self.aggrigate_log_sequence += 1;
                // self.total_locked_value -= withdraw * 10000.0;
                self.total_locked_value -= (nwithdraw / 10000.0).round();
                self.total_pool_share -= (lend_order.npoolshare / 10000.0).round();
                lend_order.tps3 = self.total_pool_share;
                lend_order.tlv3 = self.total_locked_value;
                lend_order.order_status = OrderStatus::SETTLED;
                lend_order.exit_nonce = self.nonce;

                self.last_output_state = next_output_state.clone();

                let mut lendpool_clone_with_empty_trade_order = self.clone();
                lendpool_clone_with_empty_trade_order
                    .pending_orders
                    .trader_order_data = Vec::new();
                Event::new(
                    Event::PoolUpdate(
                        LendPoolCommand::LendOrderSettleOrder(
                            rpc_request.clone(),
                            lend_order.clone(),
                            nwithdraw.clone(),
                            next_output_state,
                        ),
                        lendpool_clone_with_empty_trade_order.clone(),
                        self.aggrigate_log_sequence,
                    ),
                    String::from("LendOrderSettleOrder"),
                    LENDPOOL_EVENT_LOG.clone().to_string(),
                );
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                let _ = lendorder_db.remove(lend_order.clone(), rpc_request.clone());
                drop(lendorder_db);
            }

            LendPoolCommand::BatchExecuteTraderOrder(relayer_command) => match relayer_command {
                RelayerCommand::FundingCycle(_pool_batch_order, _metadata, _fundingrate) => {
                    // // "funding fn on chain is pending in zkos module"
                    // self.nonce += 1;
                    // self.aggrigate_log_sequence += 1;
                    // self.total_locked_value -= (pool_batch_order.amount * 10000.0).round();
                    // // do not send the event, it is too big for msg queue
                    // // self.event_log.push(Event::new(
                    // //     Event::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    // //     String::from("FundingCycleDataUpdate"),
                    // //     LENDPOOL_EVENT_LOG.clone().to_string(),
                    // // ));
                }
                RelayerCommand::RpcCommandPoolupdate() => {
                    let batch = self.pending_orders.clone();
                    if batch.trader_order_data.len() > 0 {
                        self.nonce += 1;
                        self.aggrigate_log_sequence += 1;

                        self.total_locked_value -= (batch.amount).round();

                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                        for cmd in batch.trader_order_data {
                            // let cmd_clone = cmd.clone();
                            match cmd {
                                LendPoolCommand::AddTraderOrderSettlement(
                                    rpc_cmd,
                                    mut order,
                                    _payment,
                                    next_output_state,
                                ) => {
                                    // println!("hey, im here");
                                    order.exit_nonce = self.nonce;
                                    let _ = trader_order_db.remove(order.clone(), rpc_cmd.clone());

                                    println!("I am at lendpool line 568");
                                    println!("self.total_locked_value :{:?}, \n self.total_locked_value.round() : {:?} \n self.total_pool_share : {:?} \n self.total_pool_share.round() : {:?}",self.total_locked_value,self.total_locked_value.round() as u64,self.total_pool_share,self.total_pool_share.round() as u64);

                                    // zkos_order_handler(ZkosTxCommand::ExecuteTraderOrderTX(
                                    //     order,
                                    //     rpc_cmd,
                                    //     self.last_output_state.clone(),
                                    //     next_output_state.clone(),
                                    // ));
                                    self.last_output_state = next_output_state.clone();
                                }
                                LendPoolCommand::AddTraderLimitOrderSettlement(
                                    relayer_cmd,
                                    mut order,
                                    _payment,
                                    next_output_state,
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

                                        println!("I am at lendpool line 628");
                                        println!("self.total_locked_value :{:?}, \n self.total_locked_value.round() : {:?} \n self.total_pool_share : {:?} \n self.total_pool_share.round() : {:?}",self.total_locked_value,self.total_locked_value.round() as u64,self.total_pool_share,self.total_pool_share.round() as u64);

                                        self.last_output_state = next_output_state.clone();
                                        let _ = trader_order_db
                                            .remove(order.clone(), dummy_rpccommand.clone());
                                    }
                                    _ => {}
                                },
                                LendPoolCommand::AddTraderOrderLiquidation(
                                    relayer_cmd,
                                    mut order,
                                    _payment,
                                ) => {
                                    order.exit_nonce = self.nonce;

                                    let next_output_state =
                                        create_output_state_for_trade_lend_order(
                                            self.nonce as u32,
                                            self.last_output_state
                                                .clone()
                                                .as_output_data()
                                                .get_script_address()
                                                .unwrap()
                                                .clone(),
                                            self.last_output_state
                                                .clone()
                                                .as_output_data()
                                                .get_owner_address()
                                                .clone()
                                                .unwrap()
                                                .clone(),
                                            self.total_locked_value.round() as u64,
                                            self.total_pool_share.round() as u64,
                                            0,
                                        );
                                    println!("I am at lendpool line 671");
                                    println!("self.total_locked_value :{:?}, \n self.total_locked_value.round() : {:?} \n self.total_pool_share : {:?} \n self.total_pool_share.round() : {:?}",self.total_locked_value,self.total_locked_value.round() as u64,self.total_pool_share,self.total_pool_share.round() as u64);

                                    self.last_output_state = next_output_state.clone();
                                    let _ = trader_order_db.liquidate(order.clone(), relayer_cmd);
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
            LendPoolCommand::InitiateNewPool(_lend_order, _metadata, _payment) => {
                // self.aggrigate_log_sequence += 1;
            }
            LendPoolCommand::AddFundingData(..) => {}
        }
    }

    pub fn get_lendpool(&mut self) -> (f64, f64) {
        (self.total_locked_value, self.total_pool_share)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OutputStateCommand {
    TraderSettle(Uuid, Nonce, PoolLockError),
    LendCreate(Uuid, Nonce, PoolLockError),
    LendSettle(Uuid, Nonce, PoolLockError),
    FundingInterest(Payment),
    InitiateNewPool(Payment, Nonce),
}

impl OutputStateCommand {
    pub fn get_order_id(&self) -> Option<Uuid> {
        match self.clone() {
            OutputStateCommand::TraderSettle(uuid, _nonce, _lock_error) => Some(uuid),
            OutputStateCommand::LendCreate(uuid, _nonce, _lock_error) => Some(uuid),
            OutputStateCommand::LendSettle(uuid, _nonce, _lock_error) => Some(uuid),
            OutputStateCommand::FundingInterest(_payment) => None,
            OutputStateCommand::InitiateNewPool(_payment, _nonce) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolStateHistory {
    nonce: Nonce,
    state_outputs_hex: String,
    previous_lendpool: LendPool,
    cmd: OutputStateCommand,
    aggrigate_log_sequence: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolStateHistoryDB {
    state_outputs: HashMap<Nonce, PoolStateHistory>,
    orderid_to_nonce_link: HashMap<Uuid, Nonce>,
    last_nonce: Nonce,
}

impl PoolStateHistory {
    pub fn default() -> Self {
        PoolStateHistory {
            nonce: 0,
            state_outputs_hex: "".to_string(),
            previous_lendpool: LendPool::default(),
            cmd: OutputStateCommand::InitiateNewPool(0.0, 0),
            aggrigate_log_sequence: 0,
        }
    }

    pub fn new(
        nonce: Nonce,
        state_outputs_hex: String,
        previous_lendpool: LendPool,
        cmd: OutputStateCommand,
        aggrigate_log_sequence: usize,
    ) -> Self {
        PoolStateHistory {
            nonce,
            state_outputs_hex,
            previous_lendpool,
            cmd,
            aggrigate_log_sequence,
        }
    }
    pub fn get_nonce(&mut self) -> Nonce {
        self.nonce
    }
    pub fn get_state_outputs_hex(&mut self) -> String {
        self.state_outputs_hex.clone()
    }
    pub fn get_previous_lendpool(&mut self) -> LendPool {
        self.previous_lendpool.clone()
    }
    pub fn get_cmd(&mut self) -> OutputStateCommand {
        self.cmd.clone()
    }
    pub fn get_aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }
}
impl PoolStateHistoryDB {
    pub fn new() -> Self {
        PoolStateHistoryDB {
            state_outputs: HashMap::new(),
            orderid_to_nonce_link: HashMap::new(),
            last_nonce: 0,
        }
    }
    pub fn insert_pool_history(
        &mut self,
        mut pool_state_history: PoolStateHistory,
    ) -> Option<PoolStateHistory> {
        let nonce = pool_state_history.get_nonce();
        let order_id = pool_state_history.get_cmd().get_order_id();
        let _uuid_store_result = match order_id {
            Some(order_id_uuid) => self.orderid_to_nonce_link.insert(order_id_uuid, nonce),
            None => None,
        };
        self.last_nonce = nonce;
        self.state_outputs.insert(nonce, pool_state_history)
    }

    pub fn update_pool_history(
        &mut self,
        mut pool_state_history: PoolStateHistory,
    ) -> Option<PoolStateHistory> {
        let nonce = pool_state_history.get_nonce();
        let order_id = pool_state_history.get_cmd().get_order_id();
        let _uuid_store_result = match order_id {
            Some(order_id_uuid) => self.orderid_to_nonce_link.insert(order_id_uuid, nonce),
            None => None,
        };
        self.state_outputs.insert(nonce, pool_state_history)
    }

    pub fn delete_last_state(&mut self) -> Option<PoolStateHistory> {
        let nonce = self.last_nonce;
        let last_state = self.state_outputs.remove(&nonce);
        if self.last_nonce > 0 {
            self.last_nonce -= 1;
        }

        match last_state.clone() {
            Some(mut pool_state) => {
                let order_id = pool_state.get_cmd().get_order_id();
                match order_id {
                    Some(uuid) => {
                        self.orderid_to_nonce_link.remove(&uuid);
                    }
                    None => {}
                }
            }
            None => {}
        }

        last_state
    }
    pub fn delete_state_by_nonce(&mut self, nonce: Nonce) -> Option<PoolStateHistory> {
        let last_state = self.state_outputs.remove(&nonce);

        match last_state.clone() {
            Some(mut pool_state) => {
                let order_id = pool_state.get_cmd().get_order_id();
                match order_id {
                    Some(uuid) => {
                        self.orderid_to_nonce_link.remove(&uuid);
                    }
                    None => {}
                }
            }
            None => {}
        }

        last_state
    }
    pub fn delete_bulk_state_history(
        &mut self,
        from_nonce: Option<Nonce>,
        to_nonce: Option<Nonce>,
    ) {
        let from = match from_nonce {
            Some(from) => from,
            None => 0,
        };
        let to = match to_nonce {
            Some(to) => to,
            None => self.last_nonce,
        };
        for nonce in from..to {
            let _ = self.delete_state_by_nonce(nonce);
        }
    }

    pub fn get_pool_history_by_nonce(&mut self, nonce: Nonce) -> Result<PoolStateHistory, String> {
        match self.state_outputs.get(&nonce) {
            Some(pool_history) => Ok(pool_history.clone()),
            None => Err("Pool History Not Availble for given Nonce".to_string()),
        }
    }

    pub fn get_nonce_by_uuid(&mut self, order_id: Uuid) -> Option<&Nonce> {
        self.orderid_to_nonce_link.get(&order_id)
    }
    pub fn get_last_nonce(&mut self) -> Nonce {
        self.last_nonce
    }

    pub fn get_pool_history_by_uuid(&mut self, order_id: Uuid) -> Result<PoolStateHistory, String> {
        match self.orderid_to_nonce_link.get(&order_id) {
            Some(nonce) => match self.state_outputs.get(&nonce) {
                Some(pool_history) => Ok(pool_history.clone()),
                None => Err("Pool History Not Availble for given uuid".to_string()),
            },
            None => Err("Pool History Not Availble for given uuid".to_string()),
        }
    }
}
