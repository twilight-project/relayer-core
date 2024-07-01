//https://docs.rs/kafka/0.4.1/kafka/client/struct.KafkaClient.html
#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::IS_RELAYER_ACTIVE;
use crate::relayer::*;
use kafka::client::{metadata::Broker, FetchPartition, KafkaClient};
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{thread, time};

lazy_static! {
    pub static ref KAFKA_PRODUCER: Mutex<Producer> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let broker = std::env::var("BROKER").expect("missing environment variable BROKER");
        let producer = Producer::from_hosts(vec![broker.to_owned()])
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(RequiredAcks::One)
            .create()
            .unwrap();
        Mutex::new(producer)
    };
    pub static ref KAFKA_CLIENT: Mutex<KafkaClient> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let broker = std::env::var("BROKER").expect("missing environment variable BROKER");
        Mutex::new(KafkaClient::new(vec![broker.to_owned()]))
    };
}

pub fn check_kafka_topics() -> Vec<String> {
    dotenv::dotenv().expect("Failed loading dotenv");
    // kafkalib::kafka_topic::kafka_new_topic("CLIENT-REQUEST");
    let mut kafka_client = KAFKA_CLIENT.lock().unwrap();
    kafka_client.load_metadata_all().unwrap();
    let mut result: Vec<String> = Vec::new();
    println!("{:#?}", kafka_client);
    for topic in kafka_client.topics() {
        for _partition in topic.partitions() {
            // println!(
            //     "{} #{} => {}",
            //     topic.name(),
            //     partition.id(),
            //     partition.leader().map(Broker::host).unwrap_or("no-leader!")
            // );
            result.push(topic.name().to_string());
        }
    }
    result
}
// use bstr::ext_slice::ByteSlice;
pub fn send_to_kafka_queue(cmd: RpcCommand, topic: String, key: &str) {
    let mut kafka_producer = KAFKA_PRODUCER.lock().unwrap();
    let data = serde_json::to_vec(&cmd).unwrap();
    // let data = serde_json::to_string(&cmd).unwrap();
    // let data = data.as_bytes();
    // kafka_producer
    //     .send(&Record::from_value(&topic, data))
    //     .unwrap();
    kafka_producer
        .send(&Record::from_key_value(&topic, key, data))
        .unwrap();
}

pub fn receive_from_kafka_queue(
    topic: String,
    group: String,
) -> Result<Arc<Mutex<mpsc::Receiver<RpcCommand>>>, KafkaError> {
    let (sender, receiver) = mpsc::channel();
    let _topic_clone = topic.clone();
    let _handle = thread::Builder::new()
        .name(String::from("trader_order_handle"))
        .spawn(move || {
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
                if get_relayer_status() {
                    let sender_clone = sender.clone();
                    let mss = con.poll().unwrap();
                    if mss.is_empty() {
                        // println!("No messages available right now.");
                        // return Ok(());
                    } else {
                        for ms in mss.iter() {
                            for m in ms.messages() {
                                let message = Message {
                                    offset: m.offset,
                                    key: String::from_utf8_lossy(&m.key).to_string(),
                                    value: serde_json::from_str(&String::from_utf8_lossy(&m.value))
                                        .unwrap(),
                                };
                                match sender_clone.send(Message::new(message)) {
                                    Ok(_) => {
                                        // let _ = con.consume_message(&topic_clone, partition, m.offset);
                                        // println!("Im here");
                                        let _ = con.consume_message(&topic, 0, m.offset);
                                    }
                                    Err(arg) => {
                                        println!("Closing Kafka Consumer Connection : {:#?}", arg);
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
                } else {
                    println!("Relayer turn off command received at kafka receiver");
                    break;
                }
            }
            thread::sleep(time::Duration::from_millis(3000));
            // thread::park();
        })
        .unwrap();

    Ok(Arc::new(Mutex::new(receiver)))
}

#[derive(Debug)]
pub struct Message {
    /// The offset at which this message resides in the remote kafka
    /// broker topic partition.
    pub offset: i64,
    /// The "key" data of this message.  Empty if there is no such
    /// data for this message.
    pub key: String,
    /// The value data of this message.  Empty if there is no such
    /// data for this message.
    pub value: RpcCommand,
}

impl Message {
    pub fn new(value: Message) -> RpcCommand {
        match value.value {
            RpcCommand::CreateTraderOrder(
                createtraderorder,
                Meta { mut metadata },
                _zkos_hex_string,
                _request_id,
            ) => {
                metadata.insert(String::from("offset"), Some(value.offset.to_string()));
                metadata.insert(String::from("kafka_key"), Some(value.key));
                let rcmd = RpcCommand::CreateTraderOrder(
                    createtraderorder,
                    Meta { metadata: metadata },
                    _zkos_hex_string,
                    _request_id,
                );
                return rcmd;
            }
            _ => return value.value,
        }
    }
}
use crate::relayer::CreateTraderOrder;
use crate::relayer::Meta;
