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
    PoolUpdate(LendPoolCommand, usize),
    FundingRateUpdate(f64, SystemTime),
    CurrentPriceUpdate(f64, SystemTime),
    SortedSetDBUpdate(SortedSetCommand),
    PositionSizeLogDBUpdate(PositionSizeLogCommand, PositionSizeLog),
    Stop(String),
}

impl Event {
    pub fn new(event: Event, key: String, topic: String) -> Self {
        let event_clone = event.clone();
        let pool = KAFKA_EVENT_LOG_THREADPOOL.lock().unwrap();
        pool.execute(move || {
            match event_clone {
                Event::CurrentPriceUpdate(..) => {}
                _ => {
                    println!("{:#?}", event_clone);
                }
            }
            Event::send_event_to_kafka_queue(event_clone, topic, key);
        });
        event
    }
    pub fn send_event_to_kafka_queue(event: Event, topic: String, key: String) {
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
