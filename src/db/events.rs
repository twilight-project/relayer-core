#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::EVENTLOG_VERSION;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use jsonrpc::Request;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use std::time::{self, Duration};
use std::{fmt, thread};
use uuid::serde::compact::serialize;
use uuid::Uuid;
pub type OffsetCompletion = (i32, i64);
pub type Nonce = usize;
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
            .with_required_acks(RequiredAcks::One)
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
    pub partition: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EventKey {
    pub agg_id: String,
    pub event_type: String,
    pub event_version: String,
    pub metadata: Option<HashMap<String, String>>,
}
impl EventKey {
    pub fn default() -> Self {
        EventKey {
            agg_id: "".to_string(),
            event_type: "".to_string(),
            event_version: EVENTLOG_VERSION.to_string(),
            metadata: None,
        }
    }
    pub fn new(agg_id: String, event_type: String) -> Self {
        EventKey {
            agg_id,
            event_type,
            event_version: EVENTLOG_VERSION.to_string(),
            metadata: None,
        }
    }
    pub fn new_with_version(agg_id: String, event_type: String, event_version: String) -> Self {
        EventKey {
            agg_id,
            event_type,
            event_version,
            metadata: None,
        }
    }

    pub fn add_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
    pub fn add_in_metadata(&mut self, key: String, value: String) -> bool {
        match &mut self.metadata {
            Some(metadata) => metadata.insert(key, value).is_none(),
            None => {
                let mut metadata = HashMap::new();
                let bool = metadata.insert(key, value).is_none();
                self.metadata = Some(metadata);
                bool
            }
        }
    }

    pub fn to_string_or_default(&mut self) -> String {
        match serde_json::to_string(self) {
            Ok(value) => value,
            Err(_arg) => "".to_string(),
        }
    }
    pub fn from_string_or_deafault(serialize_event_key: String) -> Self {
        match serde_json::from_str(&serialize_event_key) {
            Ok(event_key) => event_key,
            Err(arg) => {
                let mut key = EventKey::default();
                key.event_type = serialize_event_key;
                key.event_version = "0.0.0".to_string();
                let _ = key.add_in_metadata("Error".to_string(), arg.to_string());
                key
            }
        }
    }
    pub fn is_upcast(&mut self) -> bool {
        if self.event_version == EVENTLOG_VERSION.to_string() {
            false
        } else {
            true
        }
    }
    // event type casting for load any order kafka logs for different version of event log
    pub fn event_log_upcast(&mut self, log: String) -> String {
        match &*self.event_version {
            // v0.1.0 is the current version of the event log code example to upgrade the event log version for any struct change in commands
            "v0.0.0" => match &*self.event_type {
                // "CurrentPriceUpdate" => {
                //     #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                //     pub enum Eventold {
                //         CurrentPriceUpdate(f64, String),
                //     }

                //     // check for enum replacment with vec of value
                //     let log_der: Eventold = serde_json::from_str(&log).unwrap();
                //     let Eventold::CurrentPriceUpdate(price, time) = log_der;
                //     let new_log = Event::CurrentPriceUpdate(price, time.clone(), time);
                //     // println!("log:{:?}", log);
                //     self.event_version = "v0.1.0".to_string();
                //     // println!("self:{:?}", self);
                //     return serde_json::to_string(&new_log).unwrap();
                // }
                // "TxHash" => {
                //     #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                //     pub enum Eventold {
                //         TxHash(
                //             Uuid,
                //             String,
                //             String,
                //             OrderType,
                //             OrderStatus,
                //             String,
                //             Option<String>,
                //         ),
                //     }
                //     // check for enum replacment with vec of value
                //     let log_der: Eventold = serde_json::from_str(&log).unwrap();
                //     let Eventold::TxHash(
                //         order_id,
                //         string1,
                //         string2,
                //         order_type,
                //         order_status,
                //         string3,
                //         option_string,
                //     ) = log_der;
                //     let new_log = Event::TxHash(
                //         order_id,
                //         string1.clone(),
                //         string2,
                //         order_type,
                //         order_status,
                //         string3,
                //         option_string,
                //         string1,
                //     );
                //     // println!("log:{:?}", log);
                //     self.event_version = "v0.1.0".to_string();
                //     // println!("self:{:?}", self);
                //     return serde_json::to_string(&new_log).unwrap();
                // }
                _ => {
                    self.event_version = "v0.1.0".to_string();
                }
            },
            _ => {}
        }
        log
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    TraderOrder(TraderOrder, RpcCommand, usize),
    TraderOrderUpdate(TraderOrder, RelayerCommand, usize),
    TraderOrderLimitUpdate(TraderOrder, RpcCommand, usize),
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
        RequestID,
    ), //orderid, account id, TxHash, OrderType, OrderStatus,DateTime, Output, RequestID
    TxHashUpdate(
        Uuid,
        String,
        String,
        OrderType,
        OrderStatus,
        String,
        Option<String>,
    ), //orderid, account id, TxHash, OrderType, OrderStatus,DateTime, Output
    Stop(String),
    AdvanceStateQueue(Nonce, twilight_relayer_sdk::zkvm::Output),
    FeeUpdate(RelayerCommand, String), //fee data and time
}

impl Event {
    pub fn new(event: Event, key: String, topic: String) -> Self {
        let event_clone = event.clone();
        let pool = KAFKA_EVENT_LOG_THREADPOOL1.lock().unwrap();
        pool.execute(move || {
            Event::send_event_to_kafka_queue(event_clone, topic, key);
        });
        drop(pool);
        event
    }
    pub fn send_event_to_kafka_queue(event: Event, topic: String, key: String) {
        // let sw = Stopwatch::start_new();
        let key = EventKey::new(key, event.get_event_type()).to_string_or_default();
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
    }

    pub fn receive_event_for_snapshot_from_kafka_queue(
        topic: String,
        group: String,
        fetchoffset: FetchOffset,
        thread_name: &str,
    ) -> Result<(Arc<Mutex<Receiver<EventLog>>>, Sender<OffsetCompletion>), KafkaError> {
        let (sender, receiver) = unbounded();
        let (tx_consumed, rx_consumed) = unbounded::<OffsetCompletion>();
        // let _topic_clone = topic.clone();
        let _handle = thread::Builder::new()
            .name(String::from(thread_name))
            .spawn(move || {
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
                                let mut eventkey = EventKey::from_string_or_deafault(
                                    String::from_utf8_lossy(&m.key).to_string(),
                                );
                                let mut value = String::from_utf8_lossy(&m.value).to_string();
                                while eventkey.is_upcast() {
                                    value = eventkey.event_log_upcast(value);
                                }
                                let value = match serde_json::from_str(&value) {
                                    Ok(ser_value) => ser_value,
                                    Err(arg) => {
                                        eprintln!("Error in event log snapshot : {:?}", arg);
                                        let _ = con.consume_message(&topic, 0, m.offset);
                                        continue;
                                    }
                                };
                                let message = EventLog {
                                    offset: m.offset,
                                    key: String::from_utf8_lossy(&m.key).to_string(),
                                    value: value,
                                    partition: ms.partition(),
                                };
                                match sender_clone.send(message) {
                                    Ok(_) => {
                                        // let _ = con.consume_message(&topic_clone, partition, m.offset);
                                        // println!("Im here");
                                        // let _ = con.consume_message(&topic, 0, m.offset);
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
                            // let _ = con.consume_messageset(ms);
                        }
                        // con.commit_consumed().unwrap();
                    }
                    while !rx_consumed.is_empty() || !connection_status {
                        match rx_consumed.recv() {
                            Ok((partition, offset)) => {
                                let e = con.consume_message(&topic, partition, offset);

                                if e.is_err() {
                                    println!("Kafka connection failed {:?}", e);
                                    connection_status = false;
                                    break;
                                }

                                let e = con.commit_consumed();
                                if e.is_err() {
                                    println!("Kafka connection failed {:?}", e);
                                    connection_status = false;
                                    break;
                                }
                                connection_status = false;
                                break;
                            }
                            Err(_e) => {
                                connection_status = false;
                                println!(
                                    "The consumed channel is closed: {:?}",
                                    thread::current().name()
                                );
                                break;
                            }
                        }
                    }
                }
                thread::sleep(time::Duration::from_millis(3000));
                // con.commit_consumed().unwrap();
                // thread::park();
            })
            .unwrap();
        Ok((Arc::new(Mutex::new(receiver)), tx_consumed))
    }

    pub fn get_event_type(&self) -> String {
        match self {
            Event::TraderOrder(..) => "TraderOrder".to_string(),
            Event::TraderOrderUpdate(..) => "TraderOrderUpdate".to_string(),
            Event::TraderOrderFundingUpdate(..) => "TraderOrderFundingUpdate".to_string(),
            Event::TraderOrderLiquidation(..) => "TraderOrderLiquidation".to_string(),
            Event::LendOrder(..) => "LendOrder".to_string(),
            Event::PoolUpdate(..) => "PoolUpdate".to_string(),
            Event::FundingRateUpdate(..) => "FundingRateUpdate".to_string(),
            Event::CurrentPriceUpdate(..) => "CurrentPriceUpdate".to_string(),
            Event::SortedSetDBUpdate(..) => "SortedSetDBUpdate".to_string(),
            Event::PositionSizeLogDBUpdate(..) => "PositionSizeLogDBUpdate".to_string(),
            Event::TxHash(..) => "TxHash".to_string(),
            Event::TxHashUpdate(..) => "TxHashUpdate".to_string(),
            Event::Stop(..) => "Stop".to_string(),
            Event::AdvanceStateQueue(..) => "AdvanceStateQueue".to_string(),
            Event::TraderOrderLimitUpdate(..) => "TraderOrderLimitUpdate".to_string(),
            Event::FeeUpdate(..) => "FeeUpdate".to_string(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::EventKey;

    #[test]
    fn test_eventlog_key() {
        let mut metadata = HashMap::new();
        metadata.insert("key1", "value1");
        let mut event_key = EventKey::new("agg_id".to_string(), "price_update".to_string());

        println!("event_key:{:?}", event_key);
        event_key.add_in_metadata("key".to_string(), "value".to_string());
        println!("event_key_meta:{:?}", event_key);
        event_key.add_in_metadata("key2".to_string(), "value2".to_string());
        println!("event_key_meta_new:{:?}", event_key);
    }
}
