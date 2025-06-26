// #![allow(dead_code)]
// #![allow(unused_imports)]
// use crate::config::*;
// use crate::db::*;
// use crate::kafkalib::kafkacmd::receive_from_kafka_queue;
// use crate::kafkalib::kafkacmd::Message;
// use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
// use crate::relayer::*;
// // use bincode::{config, Decode, Encode};
// use bincode;
// use crossbeam_channel::{unbounded, Receiver, Sender};
// use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
// use kafka::error::Error as KafkaError;
// use kafka::producer::Record;
// use serde::Deserialize as DeserializeAs;
// use serde::Serialize as SerializeAs;
// use serde_derive::{Deserialize, Serialize};
// use std::collections::VecDeque;
// use std::collections::{HashMap, HashSet};
// use std::fs;
// use std::sync::{mpsc, Arc, Mutex, RwLock};
// use std::thread;
// use std::time;
// use std::time::SystemTime;
// use utxo_in_memory::db::LocalDBtrait;
// use uuid::Uuid;

// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// pub enum RequestStatus {
//     PENDING,
//     INITIATED,
//     COMPLETED,
//     CANCELLED,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct RequestDetail {
//     offset: i64,
//     status: RequestStatus,
//     request_time: u128,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct ClientRequestDB {
//     pub map: HashMap<RequestID, RequestDetail>,
//     pub vec_deque: VecDeque<i64>,
//     pub max_offset: i64,
// }
// impl ClientRequestDB {
//     pub fn new() -> Self {
//         ClientRequestDB {
//             map: HashMap::new(),
//             vec_deque: VecDeque::new(),
//             max_offset: 0,
//         }
//     }
//     pub fn insert(&mut self, request_id: RequestID, request_detail: RequestDetail) {}
// }

// // pub fn client_request_snapshot() {
// //     match receive_from_kafka_queue(
// //         RPC_CLIENT_REQUEST.clone().to_string(),
// //         String::from("client_cmd_receiver3"),
// //     ) {
// //         Ok(rpc_cmd_receiver) => {
// //             let rpc_cmd_receiver1 = Arc::clone(&rpc_cmd_receiver);
// //             loop {
// //                 let rpc_client_cmd_request = rpc_cmd_receiver1.lock().unwrap();
// //                 match rpc_client_cmd_request.recv() {
// //                     Ok(rpc_command) => {
// //                         rpc_event_handler(rpc_command);
// //                     }
// //                     Err(arg) => {
// //                         println!("Relayer turn off command receiver from client-request");
// //                         break;
// //                     }
// //                 }
// //             }
// //         }
// //         Err(arg) => {
// //             println!("error in client_cmd_receiver: {:#?}", arg);
// //         }
// //     }
// // }

// // pub fn receive_request_from_kafka_queue(
// //     topic: String,
// //     group: String,
// // ) -> Result<(Arc<Mutex<Receiver<RpcCommand>>>, Sender<OffsetCompletion>), KafkaError> {
// //     let (sender, receiver) = unbounded();
// //     let (tx_consumed, rx_consumed) = unbounded::<OffsetCompletion>();
// //     let _topic_clone = topic.clone();
// //     let _handle = thread::Builder::new()
// //         .name(String::from("trader_order_handle"))
// //         .spawn(move || {
// //             let broker = vec![std::env::var("BROKER")
// //                 .expect("missing environment variable BROKER")
// //                 .to_owned()];
// //             let mut con = Consumer::from_hosts(broker)
// //                 // .with_topic(topic)
// //                 .with_group(group)
// //                 .with_topic_partitions(topic.clone(), &[0])
// //                 .with_fallback_offset(FetchOffset::Earliest)
// //                 .with_offset_storage(GroupOffsetStorage::Kafka)
// //                 .create()
// //                 .unwrap();
// //             let mut connection_status = true;
// //             let _partition: i32 = 0;
// //             while connection_status {
// //                 if get_relayer_status() {
// //                     let sender_clone = sender.clone();
// //                     let mss = con.poll().unwrap();
// //                     if mss.is_empty() {
// //                         // println!("No messages available right now.");
// //                         // return Ok(());
// //                         connection_status = false;
// //                     } else {
// //                         for ms in mss.iter() {
// //                             for m in ms.messages() {
// //                                 let message = Message {
// //                                     offset: m.offset,
// //                                     key: String::from_utf8_lossy(&m.key).to_string(),
// //                                     value: serde_json::from_str(&String::from_utf8_lossy(&m.value))
// //                                         .unwrap(),
// //                                 };
// //                                 match sender_clone.send(Message::new(message)) {
// //                                     Ok(_) => {
// //                                         // let _ = con.consume_message(&topic_clone, partition, m.offset);
// //                                         // println!("Im here");
// //                                         let _ = con.consume_message(&topic, 0, m.offset);
// //                                     }
// //                                     Err(arg) => {
// //                                         println!("Closing Kafka Consumer Connection : {:#?}", arg);
// //                                         connection_status = false;
// //                                         break;
// //                                     }
// //                                 }
// //                             }
// //                             if connection_status == false {
// //                                 break;
// //                             }
// //                             let _ = con.consume_messageset(ms);
// //                         }
// //                         con.commit_consumed().unwrap();
// //                     }
// //                 } else {
// //                     println!("Relayer turn off command received at kafka receiver");
// //                     break;
// //                 }
// //             }
// //             while !rx_consumed.is_empty() || !connection_status {
// //                 match rx_consumed.recv() {
// //                     Ok((partition, offset)) => {
// //                         let e = con.consume_message(&topic, partition, offset);

// //                         if e.is_err() {
// //                             println!("Kafka connection failed {:?}", e);
// //                             connection_status = false;
// //                             break;
// //                         }

// //                         let e = con.commit_consumed();
// //                         if e.is_err() {
// //                             println!("Kafka connection failed {:?}", e);
// //                             connection_status = false;
// //                             break;
// //                         }
// //                     }
// //                     Err(_e) => {
// //                         connection_status = false;
// //                         println!(
// //                             "The consumed channel is closed: {:?}",
// //                             thread::current().name()
// //                         );
// //                         break;
// //                     }
// //                 }
// //             }
// //             thread::sleep(time::Duration::from_millis(3000));
// //             // thread::park();
// //         })
// //         .unwrap();
// //     Ok((Arc::new(Mutex::new(receiver)), tx_consumed))
// // }
