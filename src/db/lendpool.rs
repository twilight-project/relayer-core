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
    cmd_log: Vec<RelayerCommand>,
    event_log: Vec<PoolEvent>,
    pending_orders: HashMap<String, PoolOrder>,
    aggrigate_log_sequence: usize,
    last_snapshot_id: usize,
}

#[derive(Debug)]
pub struct PoolEventLog {
    pub offset: i64,
    pub key: String,
    pub value: PoolEvent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PoolEvent {
    PoolUpdate(RelayerCommand, usize),
    Stop(String),
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PoolOrder {
    nonce: usize,
    sequence: usize,
    price: f64,
    amount: f64,
    trader_order_data: Vec<Uuid>,
    lend_order_data: Vec<Uuid>,
}
// pub trait LocalDB<T> {
//     fn new() -> Self;
//     fn add(&mut self, order: T, cmd: RpcCommand) -> T;
//     fn get_nonce(&mut self) -> usize;
//     fn update_nonce(&mut self) -> usize;
//     fn get(&mut self, id: Uuid) -> Result<T, std::io::Error>;
//     fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<T>>, std::io::Error>;
//     fn update(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
//     fn remove(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
//     fn aggrigate_log_sequence(&mut self) -> usize;
//     fn load_data() -> (bool, OrderDB<T>);
//     fn check_backup() -> Self;
// }
impl LendPool {
    pub fn new() -> Self {
        LendPool {
            sequence: 0,
            nonce: 0,
            total_pool_share: 0.0,
            total_locked_value: 0.0,
            cmd_log: Vec::new(),
            event_log: Vec::new(),
            pending_orders: HashMap::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }

    pub fn load_data() -> (bool, LendPool) {
        // fn load_data() -> (bool, Self) {
        let mut database: LendPool = LendPool::new();
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
                PoolEvent::PoolUpdate(cmd, seq) => {
                    // database
                    //     .ordertable
                    //     .insert(order.uuid, Arc::new(RwLock::new(order)));
                    // database.cmd.push(cmd);
                    // database.event.push(data.value);
                    // if database.sequence < seq {
                    //     database.sequence = seq;
                    // }
                }
                PoolEvent::Stop(timex) => {
                    if timex == time {
                        // database.aggrigate_log_sequence = database.cmd.len();
                        stop_signal = false;
                    }
                }
            }
        }
        if database.sequence > 0 {
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
