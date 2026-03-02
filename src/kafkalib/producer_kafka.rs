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

use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use kafka::producer::Record;

/// Produce a message to Kafka using the shared resilient producer.
pub fn produce_main(payload: &String, topic: &str) {
    let data = payload.as_bytes();
    if let Err(e) = KAFKA_PRODUCER.send(&Record::from_value(topic, data)) {
        crate::log_heartbeat!(error, "Failed producing messages: {}", e);
    }
}
