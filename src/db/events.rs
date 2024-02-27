#![allow(dead_code)]
#![allow(unused_imports)]
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use uuid::Uuid;

lazy_static! {
    pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    pub static ref KAFKA_EVENT_LOG_THREADPOOL1: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        1,
        String::from("KAFKA_EVENT_LOG_THREADPOOL2")
    ));
    pub static ref KAFKA_EVENT_LOG_THREADPOOL2: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        1,
        String::from("KAFKA_EVENT_LOG_THREADPOOL2")
    ));
    pub static ref KAFKA_PRODUCER_EVENT: Mutex<Producer> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let broker = std::env::var("BROKER").expect("missing environment variable BROKER");
        let producer = Producer::from_hosts(vec![broker.to_owned()])
            .with_ack_timeout(Duration::from_secs(3))
            .with_required_acks(RequiredAcks::None)
            .create()
            .unwrap();
        Mutex::new(producer)
    };
}

#[derive(Debug)]
pub struct EventLog {
    pub offset: i64,
    pub key: String,
    pub value: Event,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    TraderOrder(TraderOrder, RpcCommand, usize),
    TraderOrderUpdate(TraderOrder, RelayerCommand, usize),
    TraderOrderFundingUpdate(TraderOrder, RelayerCommand),
    TraderOrderLiquidation(TraderOrder, RelayerCommand, usize),
    LendOrder(LendOrder, RpcCommand, usize),
    PoolUpdate(LendPoolCommand, LendPool, usize),
    FundingRateUpdate(f64, f64, String), //funding rate, btc price, time
    CurrentPriceUpdate(f64, String),
    SortedSetDBUpdate(SortedSetCommand),
    PositionSizeLogDBUpdate(PositionSizeLogCommand, PositionSizeLog),
    TxHash(
        Uuid,
        String,
        String,
        OrderType,
        OrderStatus,
        String,
        Option<String>,
    ), //orderid, account id, TxHash, OrderType, OrderStatus,DateTime, Output
    Stop(String),
}
use stopwatch::Stopwatch;
impl Event {
    pub fn new(event: Event, key: String, topic: String) -> Self {
        let event_clone = event.clone();
        let pool = KAFKA_EVENT_LOG_THREADPOOL1.lock().unwrap();
        pool.execute(move || {
            // match event_clone {
            //     Event::CurrentPriceUpdate(..) => {}
            //     _ => {
            //         println!("{:#?}", event_clone);
            //     }
            // }
            Event::send_event_to_kafka_queue(event_clone, topic, key);
        });
        drop(pool);
        event
    }
    pub fn send_event_to_kafka_queue(event: Event, topic: String, key: String) {
        // let sw = Stopwatch::start_new();
        let data = serde_json::to_vec(&event).unwrap();
        let pool = KAFKA_EVENT_LOG_THREADPOOL2.lock().unwrap();
        pool.execute(move || {
            let mut kafka_producer = KAFKA_PRODUCER_EVENT.lock().unwrap();
            kafka_producer
                .send(&Record::from_key_value(&topic, key, data))
                .unwrap();
            drop(kafka_producer);
        });
        drop(pool);
        // let time1 = sw.elapsed();
        // println!("kafka msg send time: {:#?}", time1);
    }

    pub fn receive_event_from_kafka_queue(
        topic: String,
        group: String,
    ) -> Result<Arc<Mutex<mpsc::Receiver<EventLog>>>, KafkaError> {
        let (sender, receiver) = mpsc::channel();
        // let _topic_clone = topic.clone();
        thread::spawn(move || {
            let broker = vec![std::env::var("BROKER")
                .expect("missing environment variable BROKER")
                .to_owned()];
            let mut con = Consumer::from_hosts(broker)
                // .with_topic(topic)
                .with_group(group)
                .with_topic_partitions(topic.clone(), &[0])
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
                                    let _ = con.consume_message(&topic, 0, m.offset);
                                }
                                Err(_arg) => {
                                    // println!("Closing Kafka Consumer Connection : {:#?}", arg);
                                    connection_status = false;
                                    break;
                                }
                            }
                        }
                        if connection_status == false {
                            break;
                        }
                        let _ = con.consume_messageset(ms);
                    }
                    con.commit_consumed().unwrap();
                }
            }
            // con.commit_consumed().unwrap();
            thread::park();
        });
        Ok(Arc::new(Mutex::new(receiver)))
    }
    pub fn receive_event_for_snapshot_from_kafka_queue(
        topic: String,
        group: String,
        fetchoffset: FetchOffset,
    ) -> Result<Arc<Mutex<mpsc::Receiver<EventLog>>>, KafkaError> {
        let (sender, receiver) = mpsc::channel();
        // let _topic_clone = topic.clone();
        thread::spawn(move || {
            let broker = vec![std::env::var("BROKER")
                .expect("missing environment variable BROKER")
                .to_owned()];
            let mut con = Consumer::from_hosts(broker)
                // .with_topic(topic)
                .with_group(group)
                .with_topic_partitions(topic.clone(), &[0])
                .with_fallback_offset(fetchoffset)
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
                                    let _ = con.consume_message(&topic, 0, m.offset);
                                }
                                Err(_arg) => {
                                    // println!("Closing Kafka Consumer Connection : {:#?}", arg);
                                    connection_status = false;
                                    break;
                                }
                            }
                        }
                        if connection_status == false {
                            break;
                        }
                        let _ = con.consume_messageset(ms);
                    }
                    con.commit_consumed().unwrap();
                }
            }
            // con.commit_consumed().unwrap();
            thread::park();
        });
        Ok(Arc::new(Mutex::new(receiver)))
    }
}
