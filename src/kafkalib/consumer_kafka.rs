//! ## consumer
//!
//!
#![allow(dead_code)]
use crate::config::BinanceMiniTickerPayload;
use crate::postgresqllib::psql_sink::kafka_sink;
use crate::relayer::CreateOrder;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;

///
///
pub fn consume_main() {
    dotenv::dotenv().expect("Failed loading dotenv");

    let broker = std::env::var("BROKER")
        .expect("missing environment variable BROKER")
        .to_owned();
    // let broker = "localhost:9092".to_owned();
    let topic = "BinanceMiniTickerPayload".to_owned();
    let group = "BTC_payload_postgreSQL".to_owned();

    if let Err(e) = consume_messages(group, topic, vec![broker]) {
        println!("Failed consuming messages: {}", e);
    }
}

fn consume_messages(group: String, topic: String, brokers: Vec<String>) -> Result<(), KafkaError> {
    let mut con = Consumer::from_hosts(brokers)
        // .with_topic(topic)
        .with_group(group)
        .with_topic_partitions(topic, &[0])
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(GroupOffsetStorage::Kafka)
        .create()?;
    loop {
        let mss = con.poll()?;
        if mss.is_empty() {
            // println!("No messages available right now.");
            // return Ok(());
        } else {
            for ms in mss.iter() {
                for m in ms.messages() {
                    let binance_payload: BinanceMiniTickerPayload =
                        serde_json::from_str(&String::from_utf8_lossy(m.value)).unwrap();
                    kafka_sink(&ms.topic(), &ms.partition(), &m.offset, binance_payload);
                }
                let _ = con.consume_messageset(ms);
            }
            con.commit_consumed()?;
        }
    }
}

pub fn consume_kafka(topic: String, group: String) {
    dotenv::dotenv().expect("Failed loading dotenv");

    let broker = std::env::var("BROKER")
        .expect("missing environment variable BROKER")
        .to_owned();
    // let broker = "localhost:9092".to_owned();
    // let topic = "BinanceMiniTickerPayload".to_owned();
    // let group = "BTC_payload_postgreSQL".to_owned();

    if let Err(e) = consume_messages_kafka(group, topic, vec![broker]) {
        println!("Failed consuming messages: {}", e);
    }
}

fn consume_messages_kafka(
    group: String,
    topic: String,
    brokers: Vec<String>,
) -> Result<(), KafkaError> {
    let mut con = Consumer::from_hosts(brokers)
        // .with_topic(topic)
        .with_group(group)
        .with_topic_partitions(topic, &[0])
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(GroupOffsetStorage::Kafka)
        .create()?;
    loop {
        let mss = con.poll()?;
        if mss.is_empty() {
            // println!("No messages available right now.");
            // return Ok(());
        } else {
            for ms in mss.iter() {
                for m in ms.messages() {
                    let ordertx: CreateOrder =
                        serde_json::from_str(&String::from_utf8_lossy(m.value)).unwrap();
                    // kafka_sink(&ms.topic(), &ms.partition(), &m.offset, binance_payload);
                    println!("order msg: {:#?}, Offset:{}", ordertx, &m.offset);
                }
                let _ = con.consume_messageset(ms);
            }
            con.commit_consumed()?;
        }
    }
}
