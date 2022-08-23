#![allow(dead_code)]
#![allow(unused_imports)]
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

// lazy_static! {
//     pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
// }

#[derive(Debug, Clone)]
pub struct LendPool {
    sequence: usize,
    nonce: usize,
    total_pool_share: f64,
    total_locked_value: f64,
    event_log: Vec<PoolEvent>,
    pending_orders: PoolBatchOrder,
    aggrigate_log_sequence: usize,
    last_snapshot_id: usize,
}
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
type Payment = f64;
type Deposit = f64;
type Withdraw = f64;

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

#[derive(Debug)]
pub struct PoolEventLog {
    pub offset: i64,
    pub key: String,
    pub value: PoolEvent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PoolEvent {
    PoolUpdate(LendPoolCommand, usize),
    Stop(String),
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PoolBatchOrder {
    pub nonce: usize,
    pub len: usize,
    pub amount: f64, //sats
    pub trader_order_data: Vec<LendPoolCommand>,
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
            timestamp: SystemTime::now(),
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
        let pool_event = PoolEvent::PoolUpdate(relayer_command.clone(), 0);
        let pool_event_execute = PoolEvent::new(
            pool_event,
            String::from("Initiate_Lend_Pool"),
            String::from("LendPoolEventLog1"),
        );
        LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: total_pool_share,
            total_locked_value: total_locked_value,
            event_log: {
                let mut event_log = Vec::new();
                event_log.push(pool_event_execute);
                event_log
            },
            pending_orders: PoolBatchOrder::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }

    pub fn load_data() -> (bool, LendPool) {
        // fn load_data() -> (bool, Self) {
        let mut database: LendPool = LendPool {
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
        let eventstop: PoolEvent = PoolEvent::Stop(time.clone());
        PoolEvent::send_event_to_kafka_queue(
            eventstop.clone(),
            String::from("LendPoolEventLog1"),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = PoolEvent::receive_event_from_kafka_queue(
            String::from("LendPoolEventLog1"),
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
                PoolEvent::PoolUpdate(cmd, seq) => match cmd.clone() {
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
                        database.event_log.push(data.value);
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                    LendPoolCommand::AddTraderOrderSettlement(..) => {}
                    LendPoolCommand::AddTraderLimitOrderSettlement(..) => {}
                    LendPoolCommand::AddTraderOrderLiquidation(..) => {}
                    LendPoolCommand::LendOrderCreateOrder(_rpc_request, lend_order, deposit) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value += deposit * 10000.0;
                        database.total_pool_share += lend_order.npoolshare;
                        database.event_log.push(data.value);
                    }
                    LendPoolCommand::LendOrderSettleOrder(_rpc_request, lend_order, withdraw) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        database.total_locked_value -= withdraw;
                        database.total_pool_share -= lend_order.npoolshare;
                        database.event_log.push(data.value);
                    }
                    LendPoolCommand::BatchExecuteTraderOrder(cmd) => {
                        database.nonce += 1;
                        database.aggrigate_log_sequence += 1;
                        match cmd {
                            RelayerCommand::FundingCycle(batch, _metadata) => {
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
                },
                PoolEvent::Stop(timex) => {
                    if timex == time {
                        // database.aggrigate_log_sequence = database.cmd.len();
                        stop_signal = false;
                    }
                }
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
                self.pending_orders.amount += payment;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                self.event_log.push(PoolEvent::new(
                    PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    String::from("AddTraderOrderSettlement"),
                    String::from("LendPoolEventLog1"),
                ));
                // check pending order length
            }
            LendPoolCommand::AddTraderLimitOrderSettlement(
                _relayer_request,
                _trader_order,
                payment,
            ) => {
                self.pending_orders.len += 1;
                self.aggrigate_log_sequence += 1;
                self.pending_orders.amount += payment;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                self.event_log.push(PoolEvent::new(
                    PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    String::from("AddTraderLimitOrderSettlement"),
                    String::from("LendPoolEventLog1"),
                ));
                // check pending order length
            }
            LendPoolCommand::AddTraderOrderLiquidation(
                _relayer_command,
                _trader_order,
                payment,
            ) => {
                self.pending_orders.len += 1;
                self.pending_orders.amount -= payment;
                self.aggrigate_log_sequence += 1;
                self.pending_orders
                    .trader_order_data
                    .push(command_clone.clone());
                self.event_log.push(PoolEvent::new(
                    PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    String::from("AddTraderOrderLiquidation"),
                    String::from("LendPoolEventLog1"),
                ));
                // check pending order length
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
                self.event_log.push(PoolEvent::new(
                    PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                    String::from("LendOrderCreateOrder"),
                    String::from("LendPoolEventLog1"),
                ));
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                lendorder_db.add(lend_order, rpc_request);
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
                self.event_log.push(PoolEvent::new(
                    PoolEvent::PoolUpdate(
                        LendPoolCommand::LendOrderSettleOrder(
                            rpc_request.clone(),
                            lend_order.clone(),
                            withdraw.clone(),
                        ),
                        self.aggrigate_log_sequence,
                    ),
                    String::from("LendOrderSettleOrder"),
                    String::from("LendPoolEventLog1"),
                ));
                let mut lendorder_db = LEND_ORDER_DB.lock().unwrap();
                let _ = lendorder_db.remove(lend_order, rpc_request);
                drop(lendorder_db);
            }
            LendPoolCommand::BatchExecuteTraderOrder(relayer_command) => {
                self.nonce += 1;
                self.aggrigate_log_sequence += 1;
                match relayer_command {
                    RelayerCommand::FundingCycle(pool_batch_order, _metadata) => {
                        self.total_locked_value -= pool_batch_order.amount * 10000.0;
                        self.event_log.push(PoolEvent::new(
                            PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                            String::from("FundingCycleDataUpdate"),
                            String::from("LendPoolEventLog1"),
                        ));
                    }
                    RelayerCommand::RpcCommandPoolupdate() => {
                        let batch = self.pending_orders.clone();
                        self.total_locked_value -= batch.amount * 10000.0;
                        self.event_log.push(PoolEvent::new(
                            PoolEvent::PoolUpdate(command_clone, self.aggrigate_log_sequence),
                            String::from("FundingCycleDataUpdate"),
                            String::from("LendPoolEventLog1"),
                        ));
                        let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
                        for cmd in batch.trader_order_data {
                            let cmd_clone = cmd.clone();
                            match cmd {
                                LendPoolCommand::AddTraderOrderSettlement(
                                    rpc_cmd,
                                    order,
                                    payment,
                                ) => {
                                    trader_order_db.remove(order, rpc_cmd);
                                }
                                LendPoolCommand::AddTraderLimitOrderSettlement(
                                    relayer_cmd,
                                    order,
                                    payment,
                                ) => match relayer_cmd {
                                    RelayerCommand::PriceTickerOrderSettle(
                                        _,
                                        metadata,
                                        current_price,
                                    ) => {
                                        let dummy_rpccommand =
                                            RpcCommand::RelayerCommandTraderOrderOnLimit(
                                                order.clone(),
                                                metadata,
                                                current_price,
                                            );
                                        trader_order_db.remove(order.clone(), dummy_rpccommand);
                                    }
                                    _ => {}
                                },
                                LendPoolCommand::AddTraderOrderLiquidation(
                                    relayer_cmd,
                                    order,
                                    payment,
                                ) => {
                                    trader_order_db.liquidate(order, relayer_cmd);
                                }
                                _ => {}
                            }
                        }
                        drop(trader_order_db);
                        self.pending_orders = PoolBatchOrder::new();
                    }
                    _ => {}
                }
            }
            LendPoolCommand::InitiateNewPool(lend_order, metadata) => {
                // self.aggrigate_log_sequence += 1;
            }
            LendPoolCommand::AddFundingData(..) => {}
        }
    }

    pub fn get_lendpool(&mut self) -> (f64, f64) {
        (self.total_locked_value, self.total_pool_share)
    }
}

impl PoolEvent {
    pub fn new(event: PoolEvent, key: String, topic: String) -> Self {
        match event {
            PoolEvent::PoolUpdate(cmd, seq) => {
                let event = PoolEvent::PoolUpdate(cmd, seq);
                let event_clone = event.clone();
                let pool = KAFKA_EVENT_LOG_THREADPOOL.lock().unwrap();
                pool.execute(move || {
                    PoolEvent::send_event_to_kafka_queue(event_clone, topic, key);
                });
                event
            }
            PoolEvent::Stop(seq) => PoolEvent::Stop(seq.to_string()),
        }
    }
    pub fn send_event_to_kafka_queue(event: PoolEvent, topic: String, key: String) {
        let mut kafka_producer = KAFKA_PRODUCER.lock().unwrap();
        let data = serde_json::to_vec(&event).unwrap();
        kafka_producer
            .send(&Record::from_key_value(&topic, key, data))
            .unwrap();
    }

    pub fn receive_event_from_kafka_queue(
        topic: String,
        group: String,
    ) -> Result<Arc<Mutex<mpsc::Receiver<PoolEventLog>>>, KafkaError> {
        let (sender, receiver) = mpsc::channel();
        let _topic_clone = topic.clone();
        thread::spawn(move || {
            let broker = vec![std::env::var("BROKER")
                .expect("missing environment variable BROKER")
                .to_owned()];
            let mut con = Consumer::from_hosts(broker)
                // .with_topic(topic)
                .with_group(group)
                .with_topic_partitions(topic, &[0])
                .with_fallback_offset(FetchOffset::Earliest)
                .with_offset_storage(GroupOffsetStorage::Kafka)
                .create()
                .unwrap();
            let mut connection_status = true;
            let _partition: i32 = 0;
            while connection_status {
                let sender_clone = sender.clone();
                let mss = con.poll().unwrap();
                if mss.is_empty() {
                } else {
                    for ms in mss.iter() {
                        for m in ms.messages() {
                            let message = PoolEventLog {
                                offset: m.offset,
                                key: String::from_utf8_lossy(&m.key).to_string(),
                                value: serde_json::from_str(&String::from_utf8_lossy(&m.value))
                                    .unwrap(),
                            };
                            match sender_clone.send(message) {
                                Ok(_) => {}
                                Err(_arg) => {
                                    // println!("Closing Kafka Consumer Connection : {:#?}", arg);
                                    connection_status = false;
                                    break;
                                }
                            }
                        }
                        let _ = con.consume_messageset(ms);
                    }
                    con.commit_consumed().unwrap();
                }
            }
            con.commit_consumed().unwrap();
            thread::park();
        });
        Ok(Arc::new(Mutex::new(receiver)))
    }
}
