//! ## producer
//! **issue creating producer each time while calling function. this is a relatively costly operation, so you'll do this typically once in your application and re-use the instance many times.
//!
//! **Need to change line: 71:77 and call this function anly one time
//!
//!  let mut producer = Producer::from_hosts(brokers)
//!        .with_ack_timeout(Duration::from_secs(1))
//!        .with_required_acks(RequiredAcks::One)
//!        .create()?;

#![allow(dead_code)]
use std::time::Duration;
// mod lib;
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};

/// ## producer
///This is a higher-level producer API for Kafka and is provided by the module `producer::produce_main`. It provides convenient automatic partition assignment capabilities through partitioners. This is the API a client application of this library wants to use for sending messsages to Kafka.
///
/// ### Examples
///   
/// Basic usage:
///
/// ```rust,no_run
///  use crate::relayer_core::kafkalib;
///
/// fn main() {
/// 	let message_data: String = String::from("{\"name\": \"Some name\", \"age\": \"30\", }");
/// 	let topic: &str = "Some_Topic"; //topic must already exist in kafka topic list
/// 	kafkalib::producer_kafka::produce_main(&message_data, topic);
/// }
///
/// ```
///
pub fn produce_main(payload: &String, topic: &str) {
    // env_logger::init();

    // let broker = "localhost:9092";
    dotenv::dotenv().expect("Failed loading dotenv");

    let broker = std::env::var("BROKER").expect("missing environment variable BROKER");

    let data = &payload.as_bytes();

    if let Err(e) = produce_message(data, topic, vec![broker.to_owned()]) {
        println!("Failed producing messages: {}", e);
    }
}

fn produce_message<'a, 'b>(
    data: &'a [u8],
    topic: &'b str,
    brokers: Vec<String>,
) -> Result<(), KafkaError> {
    // ## issue
    // ~ create a producer. this is a relatively costly operation, so
    // you'll do this typically once in your application and re-use
    // the instance many times.
    let mut producer = Producer::from_hosts(brokers)
        // ~ give the brokers one second time to ack the message
        .with_ack_timeout(Duration::from_secs(1))
        // ~ require only one broker to ack the message
        .with_required_acks(RequiredAcks::One)
        // ~ build the producer with the above settings
        .create()?;
    // ~ now send a single message.  this is a synchronous/blocking
    // operation.

    // ~ we're sending 'data' as a 'value'. there will be no key
    // associated with the sent message.

    // ~ we leave the partition "unspecified" - this is a negative
    // partition - which causes the producer to find out one on its
    // own using its underlying partitioner.
    // producer.send(&Record {
    //     topic,
    //     partition: -1,
    //     key: (),
    //     value: data,
    // })?;

    // ~ we can achieve exactly the same as above in a shorter way with
    // the following call
    producer.send(&Record::from_value(topic, data))?;

    Ok(())
}
