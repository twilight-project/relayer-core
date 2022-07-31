#![allow(dead_code)]
#![allow(unused_imports)]

// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
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

lazy_static! {
    pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    pub static ref KAFKA_EVENT_LOG_THREADPOOL: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        2,
        String::from("KAFKA_EVENT_LOG_THREADPOOL")
    ));
}
#[derive(Debug, Clone)]
pub struct OrderDB<T> {
    ordertable: HashMap<Uuid, Arc<RwLock<T>>>,
    sequence: usize,
    nonce: usize,
    cmd: Vec<RpcCommand>,
    event: Vec<Event<T>>,
    aggrigate_log_sequence: usize,
    last_snapshot_id: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event<T> {
    TraderOrder(T, RpcCommand, usize),
    LendOrder(T, RpcCommand, usize),
    RelayerUpdate(T, RelayerCommand, usize),
}

// impl<T> Event<T> {
//     fn clone(&self) -> Self {
//         (*self).clone()
//     }
// }

impl Event<TraderOrder> {
    pub fn new(event: Event<TraderOrder>, key: String) -> Self {
        match event {
            Event::TraderOrder(order, cmd, seq) => {
                let event = Event::TraderOrder(order, cmd, seq);
                let event_clone = event.clone();
                let pool = KAFKA_EVENT_LOG_THREADPOOL.lock().unwrap();
                pool.execute(move || {
                    Event::send_event_to_kafka_queue(
                        event_clone,
                        String::from("TraderOrderEventLog"),
                        key,
                    );
                });
                event
            }
            Event::LendOrder(order, cmd, seq) => Event::LendOrder(order, cmd, seq),
            Event::RelayerUpdate(order, cmd, seq) => Event::RelayerUpdate(order, cmd, seq),
        }
    }
    pub fn send_event_to_kafka_queue(event: Event<TraderOrder>, topic: String, key: String) {
        let mut kafka_producer = KAFKA_PRODUCER.lock().unwrap();
        let data = serde_json::to_vec(&event).unwrap();
        kafka_producer
            .send(&Record::from_key_value(&topic, key, data))
            .unwrap();
    }

    pub fn receive_event_from_kafka_queue(
        topic: String,
        group: String,
    ) -> Result<Arc<Mutex<mpsc::Receiver<EventLog>>>, KafkaError> {
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
                    // println!("No messages available right now.");
                    // return Ok(());
                } else {
                    for ms in mss.iter() {
                        for m in ms.messages() {
                            let message = EventLog {
                                offset: m.offset,
                                key: String::from_utf8_lossy(&m.key).to_string(),
                                value: serde_json::from_str(&String::from_utf8_lossy(&m.value))
                                    .unwrap(),
                            };
                            match sender_clone.send(message) {
                                Ok(_) => {
                                    // let _ = con.consume_message(&topic_clone, partition, m.offset);
                                    // println!("Im here");
                                }
                                Err(arg) => {
                                    println!("Closing Kafka Consumer Connection : {:#?}", arg);
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

pub trait LocalDB<T> {
    fn new() -> Self;
    fn add(&mut self, order: T, cmd: RpcCommand) -> T;
    fn get_nonce(&mut self) -> usize;
    fn update_nonce(&mut self) -> usize;
    fn get(&mut self, id: Uuid) -> Result<T, std::io::Error>;
    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<T>>, std::io::Error>;
    fn update(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
    fn remove(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
    fn aggrigate_log_sequence(&mut self) -> usize;
    fn load_data() -> bool;
    fn check_backup() -> Self;
    // fn get_from_snapshot(&self) {
    //     println!("{} says {}", self.name(), self.noise());
    // }
}

impl LocalDB<TraderOrder> for OrderDB<TraderOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            cmd: Vec::new(),
            event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }

    fn add(&mut self, mut order: TraderOrder, cmd: RpcCommand) -> TraderOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        order.entry_nonce = self.nonce;
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::new(
            Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            String::from("add_order"),
        ));
        order.clone()
    }

    fn get_nonce(&mut self) -> usize {
        get_nonce()
    }

    fn update_nonce(&mut self) -> usize {
        update_nonce()
    }

    fn get(&mut self, id: Uuid) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.get(&id) {
            Some(order) => {
                let orx = order.read().unwrap();
                Ok(orx.clone())
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<TraderOrder>>, std::io::Error> {
        match self.ordertable.get_mut(&id) {
            Some(order) => Ok(Arc::clone(order)),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn update(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::TraderOrder(
            order.clone(),
            cmd.clone(),
            self.aggrigate_log_sequence,
        ));
        Ok(order.clone())
    }

    fn remove(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.remove(&order.uuid) {
            Some(_) => {
                self.cmd.push(cmd.clone());
                self.aggrigate_log_sequence += 1;
                order.exit_nonce = get_nonce();
                self.event.push(Event::TraderOrder(
                    order.clone(),
                    cmd.clone(),
                    self.aggrigate_log_sequence,
                ));
                Ok(order)
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }

    fn load_data() -> bool {
        // fn load_data() -> (bool, Self) {

        false
    }

    fn check_backup() -> Self {
        let redis_data: bool = OrderDB::<TraderOrder>::load_data();
        if redis_data {
            LocalDB::<TraderOrder>::new()
        } else {
            LocalDB::<TraderOrder>::new()
        }
    }
}

pub fn get_nonce() -> usize {
    let nonce = GLOBAL_NONCE.read().unwrap();
    *nonce
}
pub fn update_nonce() -> usize {
    let mut nonce = GLOBAL_NONCE.write().unwrap();
    *nonce += 1;
    *nonce
}

#[derive(Debug)]
pub struct EventLog {
    /// The offset at which this message resides in the remote kafka
    /// broker topic partition.
    pub offset: i64,
    /// The "key" data of this message.  Empty if there is no such
    /// data for this message.
    pub key: String,
    /// The value data of this message.  Empty if there is no such
    /// data for this message.
    pub value: Event<TraderOrder>,
}
