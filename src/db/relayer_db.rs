#![allow(dead_code)]
#![allow(unused_imports)]
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

lazy_static! {
    pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    pub static ref KAFKA_EVENT_LOG_THREADPOOL: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        2,
        String::from("KAFKA_EVENT_LOG_THREADPOOL")
    ));
}

#[derive(Debug)]
pub struct EventLog<T> {
    pub offset: i64,
    pub key: String,
    pub value: Event<T>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(bound(serialize = "T: SerializeAs", deserialize = "T: DeserializeAs<'de>",))]
pub enum Event<T> {
    TraderOrder(T, RpcCommand, usize),
    LendOrder(T, RpcCommand, usize),
    RelayerUpdate(T, RelayerCommand, usize),
    Stop(String),
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
    fn load_data() -> (bool, OrderDB<T>);
    fn check_backup() -> Self;
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
        self.event.push(Event::<TraderOrder>::new(
            Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            String::from("add_order"),
            String::from("TraderOrderEventLog1"),
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
                    std::io::ErrorKind::NotFound,
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
                    std::io::ErrorKind::NotFound,
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
                    std::io::ErrorKind::NotFound,
                    "Order not found",
                ))
            }
        }
    }

    fn aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }

    fn load_data() -> (bool, OrderDB<TraderOrder>) {
        // fn load_data() -> (bool, Self) {
        let mut database: OrderDB<TraderOrder> = LocalDB::<TraderOrder>::new();
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let eventstop: Event<TraderOrder> = Event::Stop(time.clone());
        Event::<TraderOrder>::send_event_to_kafka_queue(
            eventstop.clone(),
            String::from("TraderOrderEventLog1"),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = Event::<TraderOrder>::receive_event_from_kafka_queue(
            String::from("TraderOrderEventLog1"),
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
            // pub struct EventLog {
            //     pub offset: i64,
            //     pub key: String,
            //     pub value: Event<TraderOrder>,
            // }
            // println!("kafka msg:{:#?}", data.value.clone());
            // if data.key == String::from("StopLoadMSG") {
            match data.value.clone() {
                Event::TraderOrder(order, cmd, seq) => {
                    database
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                    database.cmd.push(cmd);
                    database.event.push(data.value);
                    if database.sequence < seq {
                        database.sequence = seq;
                    }
                }
                Event::Stop(timex) => {
                    if timex == time {
                        database.aggrigate_log_sequence = database.cmd.len();
                        stop_signal = false;
                    }
                }
                Event::LendOrder { .. } => {
                    println!("LendOrder Im here");
                }
                Event::RelayerUpdate { .. } => {
                    println!("RelayerUpdate Im here");
                }
            }
        }
        if database.sequence > 0 {
            (true, database.clone())
        } else {
            (false, database)
        }
    }

    fn check_backup() -> Self {
        println!("Loading TraderOrder Database ....");
        let (redis_data, database): (bool, OrderDB<TraderOrder>) =
            OrderDB::<TraderOrder>::load_data();
        if redis_data {
            // println!("uploading db:{:?}", database);
            println!("TraderOrder Database Loaded ....");
            database
        } else {
            println!("No old TraderOrder Database found ....\nCreating new database");
            LocalDB::<TraderOrder>::new()
        }
    }
}

impl Event<TraderOrder> {
    pub fn new(event: Event<TraderOrder>, key: String, topic: String) -> Self {
        match event {
            Event::TraderOrder(order, cmd, seq) => {
                let event = Event::TraderOrder(order, cmd, seq);
                let event_clone = event.clone();
                let pool = KAFKA_EVENT_LOG_THREADPOOL.lock().unwrap();
                pool.execute(move || {
                    Event::<TraderOrder>::send_event_to_kafka_queue(event_clone, topic, key);
                });
                event
            }
            Event::LendOrder(order, cmd, seq) => Event::LendOrder(order, cmd, seq),
            Event::RelayerUpdate(order, cmd, seq) => Event::RelayerUpdate(order, cmd, seq),
            Event::Stop(seq) => Event::Stop(seq.to_string()),
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
    ) -> Result<Arc<Mutex<mpsc::Receiver<EventLog<TraderOrder>>>>, KafkaError> {
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

impl LocalDB<LendOrder> for OrderDB<LendOrder> {
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

    fn add(&mut self, mut order: LendOrder, cmd: RpcCommand) -> LendOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        order.entry_nonce = self.nonce;
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::<LendOrder>::new(
            Event::LendOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
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

    fn get(&mut self, id: Uuid) -> Result<LendOrder, std::io::Error> {
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

    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<LendOrder>>, std::io::Error> {
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
        mut order: LendOrder,
        cmd: RpcCommand,
    ) -> Result<LendOrder, std::io::Error> {
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
        mut order: LendOrder,
        cmd: RpcCommand,
    ) -> Result<LendOrder, std::io::Error> {
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

    fn load_data() -> (bool, OrderDB<LendOrder>) {
        // fn load_data() -> (bool, Self) {
        let mut database: OrderDB<LendOrder> = LocalDB::<LendOrder>::new();
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let eventstop: Event<LendOrder> = Event::Stop(time.clone());
        Event::<LendOrder>::send_event_to_kafka_queue(
            eventstop.clone(),
            String::from("LendOrderEventLog1"),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = Event::<LendOrder>::receive_event_from_kafka_queue(
            String::from("LendOrderEventLog1"),
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
                Event::LendOrder(order, cmd, seq) => {
                    database
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                    database.cmd.push(cmd);
                    database.event.push(data.value);
                    if database.sequence < seq {
                        database.sequence = seq;
                    }
                }
                Event::Stop(timex) => {
                    if timex == time {
                        database.aggrigate_log_sequence = database.cmd.len();
                        stop_signal = false;
                    }
                }
                Event::TraderOrder { .. } => {
                    println!("TraderOrder Im here");
                }
                Event::RelayerUpdate { .. } => {
                    println!("RelayerUpdate Im here");
                }
            }
        }
        if database.sequence > 0 {
            (true, database.clone())
        } else {
            (false, database)
        }
    }

    fn check_backup() -> Self {
        println!("Loading LendOrder Database ....");
        let (redis_data, database): (bool, OrderDB<LendOrder>) = OrderDB::<LendOrder>::load_data();
        if redis_data {
            // println!("uploading db:{:?}", database);
            println!("LendOrder Database Loaded ....");
            database
        } else {
            println!("No old LendOrder Database found ....\nCreating new database");
            LocalDB::<LendOrder>::new()
        }
    }
}

impl Event<LendOrder> {
    pub fn new(event: Event<LendOrder>, key: String) -> Self {
        match event {
            Event::TraderOrder(order, cmd, seq) => Event::TraderOrder(order, cmd, seq),
            Event::LendOrder(order, cmd, seq) => {
                let event = Event::LendOrder(order, cmd, seq);
                let event_clone = event.clone();
                let pool = KAFKA_EVENT_LOG_THREADPOOL.lock().unwrap();
                pool.execute(move || {
                    Event::<LendOrder>::send_event_to_kafka_queue(
                        event_clone,
                        String::from("LendOrderEventLog1"),
                        key,
                    );
                });
                event
            }
            Event::RelayerUpdate(order, cmd, seq) => Event::RelayerUpdate(order, cmd, seq),
            Event::Stop(seq) => Event::Stop(seq.to_string()),
        }
    }
    pub fn send_event_to_kafka_queue(event: Event<LendOrder>, topic: String, key: String) {
        let mut kafka_producer = KAFKA_PRODUCER.lock().unwrap();
        let data = serde_json::to_vec(&event).unwrap();
        kafka_producer
            .send(&Record::from_key_value(&topic, key, data))
            .unwrap();
    }

    pub fn receive_event_from_kafka_queue(
        topic: String,
        group: String,
    ) -> Result<Arc<Mutex<mpsc::Receiver<EventLog<LendOrder>>>>, KafkaError> {
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

pub fn get_nonce() -> usize {
    let nonce = GLOBAL_NONCE.read().unwrap();
    *nonce
}
pub fn update_nonce() -> usize {
    let mut nonce = GLOBAL_NONCE.write().unwrap();
    *nonce += 1;
    *nonce
}
