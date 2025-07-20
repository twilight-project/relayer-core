//https://docs.rs/kafka/0.4.1/kafka/client/struct.KafkaClient.html
#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::BROKER;
use crate::db::OffsetCompletion;
use crate::kafkalib::offset_manager::OffsetManager;
use crate::relayer::*;
use crossbeam_channel::{ unbounded, Receiver, Sender };

use kafka::client::{ KafkaClient };
use kafka::consumer::{ Consumer, FetchOffset, GroupOffsetStorage };
use kafka::error::Error as KafkaError;
use kafka::producer::{ Producer, RequiredAcks };
use std::sync::{ Arc, Mutex };
use std::time::Duration;
use std::{ thread, time };

lazy_static! {
    pub static ref KAFKA_PRODUCER: Mutex<Producer> = {
        dotenv::dotenv().ok();
        let producer = Producer::from_hosts(vec![BROKER.to_owned()])
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(RequiredAcks::One)
            .create()
            .expect("Failed to create kafka producer");
        Mutex::new(producer)
    };
    pub static ref KAFKA_CLIENT: Mutex<KafkaClient> = {
        Mutex::new(KafkaClient::new(vec![BROKER.to_owned()]))
    };
}

pub fn check_kafka_topics() -> Result<Vec<String>, String> {
    // kafkalib::kafka_topic::kafka_new_topic("CLIENT-REQUEST");
    let mut result: Vec<String> = Vec::new();
    match KAFKA_CLIENT.lock() {
        Ok(mut kafka_client) => {
            match kafka_client.load_metadata_all() {
                Ok(_) => {}
                Err(e) => {
                    crate::log_heartbeat!(error, "Kafka client load metadata all failed {:?}", e);
                    return Err(e.to_string());
                }
            }
            crate::log_heartbeat!(info, "{:#?}", kafka_client);
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
        }
        Err(e) => {
            crate::log_heartbeat!(error, "Kafka client lock failed {:?}", e);
            return Err(e.to_string());
        }
    }

    Ok(result)
}

pub fn receive_from_kafka_queue(
    topic: String,
    group: String
) -> Result<
    (Arc<Mutex<Receiver<(RpcCommand, OffsetCompletion)>>>, Sender<OffsetCompletion>),
    KafkaError
> {
    let (sender, receiver) = unbounded();
    let (tx_consumed, rx_consumed) = unbounded::<OffsetCompletion>();
    let _topic_clone = topic.clone();
    let _handle = match
        thread::Builder
            ::new()
            .name(String::from("trader_order_handle"))
            .spawn(move || {
                // get the last offset from kafka
                let last_offset = get_offset_from_kafka(topic.clone(), group.clone());
                println!("last_offset: {:#?}", last_offset);

                // create an offset tracker
                let offset_tracker = Arc::new(OffsetManager::new(last_offset - 1));

                let worker_tracker = offset_tracker.clone();
                let commit_tracker = offset_tracker.clone();
                let mut pool_attempt = 0;
                match
                    Consumer::from_hosts(vec![BROKER.to_owned()])
                        // .with_topic(topic)
                        .with_group(group.clone())
                        .with_topic_partitions(topic.clone(), &[0])
                        .with_fallback_offset(FetchOffset::Earliest)
                        .with_offset_storage(GroupOffsetStorage::Kafka)
                        .create()
                {
                    Ok(mut con) => {
                        let mut connection_status = true;
                        let mut topic_partition: i32 = 0;
                        while connection_status {
                            if get_relayer_status() {
                                let sender_clone = sender.clone();
                                match con.poll() {
                                    Ok(mss) => {
                                        if mss.is_empty() {
                                        } else {
                                            for ms in mss.iter() {
                                                for m in ms.messages() {
                                                    match
                                                        serde_json::from_str(
                                                            &String::from_utf8_lossy(&m.value)
                                                        )
                                                    {
                                                        Ok(value) => {
                                                            let message = Message {
                                                                offset: m.offset,
                                                                key: String::from_utf8_lossy(
                                                                    &m.key
                                                                ).to_string(),
                                                                value: value,
                                                            };
                                                            match
                                                                sender_clone.send((
                                                                    Message::new(message),
                                                                    (ms.partition(), m.offset),
                                                                ))
                                                            {
                                                                Ok(_) => {}
                                                                Err(arg) => {
                                                                    eprintln!(
                                                                        "Closing Kafka Consumer Connection on kafka rpc client: {:#?}",
                                                                        arg
                                                                    );
                                                                    connection_status = false;
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            eprintln!(
                                                                "Error parsing message on kafka rpc client: {:?}",
                                                                e
                                                            );
                                                            continue;
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
                                    }
                                    Err(e) => {
                                        crate::log_heartbeat!(warn, "Kafka poll error: {:?}", e);
                                        std::thread::sleep(Duration::from_secs(1));
                                        pool_attempt += 1;
                                        if pool_attempt > 100 {
                                            break;
                                        }
                                    }
                                }
                            } else {
                                crate::log_heartbeat!(
                                    info,
                                    "Relayer turn off command received at kafka receiver"
                                );
                                break;
                            }

                            while !rx_consumed.is_empty() {
                                match rx_consumed.recv() {
                                    Ok((partition, offset)) => {
                                        worker_tracker.mark_done(offset);
                                        topic_partition = partition;
                                    }
                                    Err(_e) => {
                                        connection_status = false;
                                        crate::log_heartbeat!(
                                            error,
                                            "The consumed channel is closed: {:?}",
                                            thread::current().name()
                                        );
                                        break;
                                    }
                                }
                            }

                            if let Some(offset) = commit_tracker.next_commit_offset() {
                                let e = con.consume_message(&topic, topic_partition, offset);

                                if e.is_err() {
                                    eprintln!("Kafka connection failed {:?}", e);
                                    break;
                                }

                                let e = con.commit_consumed();
                                if e.is_err() {
                                    eprintln!("Kafka connection failed {:?}", e);
                                    break;
                                }
                            }
                            if connection_status == false {
                                break;
                            }
                        }
                        thread::sleep(time::Duration::from_millis(3000));
                        // thread::park();
                    }
                    Err(e) => {
                        crate::log_heartbeat!(error, "Kafka connection failed {:?}", e);
                        drop(sender);
                        return;
                    }
                }
            })
    {
        Ok(handle) => handle,
        Err(e) => {
            crate::log_heartbeat!(error, "thread builder failed {:?}", e);
            return Err(e.into());
        }
    };

    Ok((Arc::new(Mutex::new(receiver)), tx_consumed))
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
                    _request_id
                );
                return rcmd;
            }
            _ => {
                return value.value;
            }
        }
    }
}
use crate::relayer::Meta;

pub fn get_offset_from_kafka(topic: String, group: String) -> i64 {
    match
        Consumer::from_hosts(vec![BROKER.to_owned()])
            // .with_topic(topic)
            .with_group(group)
            .with_topic_partitions(topic.clone(), &[0])
            .with_fallback_offset(FetchOffset::Earliest)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .create()
    {
        Ok(mut con) => {
            let connection_status = true;
            while connection_status {
                let mss = match con.poll() {
                    Ok(mss) => mss,
                    Err(e) => {
                        crate::log_heartbeat!(error, "Error polling kafka consumer: {:?}", e);
                        return -1;
                    }
                };
                if mss.is_empty() {
                    // println!("No messages available right now.");
                    // return Ok(());
                    thread::sleep(time::Duration::from_millis(100));
                } else {
                    for ms in mss.iter() {
                        for m in ms.messages() {
                            return m.offset;
                        }
                    }
                }
            }
        }
        Err(e) => {
            crate::log_heartbeat!(error, "Error creating kafka consumer: {:?}", e);
            return -1;
        }
    }

    return -1;
}
