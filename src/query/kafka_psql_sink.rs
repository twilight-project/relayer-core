use crate::config::*;
// use crate::db::*;
use crate::relayer::*;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

extern crate postgres_types;

pub fn upload_rpc_command_to_psql() {
    let recever = receive_event_from_kafka_queue(
        // RPC_CLIENT_REQUEST.clone().to_string(),
        String::from("CLIENT-REQUEST"),
        String::from("Query_Saver_Group"),
        0,
    )
    .unwrap();
    let recever1 = recever.lock().unwrap();
    loop {
        let data = recever1.recv().unwrap();
        // let event = data.value.clone();
        psql_rpc_command(data.clone());
    }
}
pub fn upload_event_log_to_psql() {
    let recever = receive_event_from_kafka_queue(
        // CORE_EVENT_LOG.clone().to_string(),
        String::from("CoreEventLogTopic"),
        String::from("Query_Saver_Group-1"),
        0,
    )
    .unwrap();
    let recever1 = recever.lock().unwrap();
    loop {
        let data = recever1.recv().unwrap();
        // let event = data.value.clone();
        psql_event_logs(data.clone());
        // println!("{:#?}", data);
    }
}

pub fn receive_event_from_kafka_queue(
    topic: String,
    group: String,
    partition: i32,
) -> Result<Arc<Mutex<mpsc::Receiver<EventLogRPCQuery>>>, KafkaError> {
    let (sender, receiver) = mpsc::channel();
    let _topic_clone = topic.clone();
    thread::spawn(move || {
        let broker = vec![std::env::var("BROKER")
            .expect("missing environment variable BROKER")
            .to_owned()];
        let mut con = Consumer::from_hosts(broker)
            // .with_topic(topic)
            .with_group(group)
            .with_topic_partitions(topic, &[partition])
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
                        let message = EventLogRPCQuery {
                            offset: m.offset,
                            key: String::from_utf8_lossy(&m.key).to_string(),
                            value: serde_json::to_value(&String::from_utf8_lossy(&m.value))
                                .unwrap(),
                            // value: serde_json::from_str(&String::from_utf8_lossy(&m.value))
                            //     .unwrap(),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EventLogRPCQuery {
    pub offset: i64,
    pub key: String,
    pub value: Value,
}

pub fn psql_rpc_command(data: EventLogRPCQuery) {
    //creating static connection
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    let query = format!(
        "INSERT INTO public.rpc_query(\"offset\", key, payload) VALUES ({},'{}',$1);",
        data.offset, data.key
    );
    client.execute(&query, &[&data.value]).unwrap();
}
pub fn psql_event_logs(data: EventLogRPCQuery) {
    //creating static connection
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    let query = format!(
        "INSERT INTO public.event_logs(\"offset\", key, payload) VALUES ({},'{}',$1);",
        data.offset, data.key
    );
    client.execute(&query, &[&data.value]).unwrap();
}
