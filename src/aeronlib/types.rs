use aeron_rs::utils::types::Index;
// use mpsc::{channel, Receiver, Sender};
use crate::aeronlib::aeronqueue::{start_aeron_topic_consumer, start_aeron_topic_producer};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
// use std::sync::{mpsc, Arc, Mutex};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, EnumIter)]
pub enum StreamId {
    CreateTraderOrder = 1001,  //TraderOrder
    CreateLendOrder = 1002,    //LendOrder
    ExecuteTraderOrder = 1003, //TraderOrder
    ExecuteLendOrder = 1004,   //LendOrder
    CancelTraderOrder = 1005,  //TraderOrder
    GetPnL = 1006,             //TraderOrder
    GetPoolShare = 1007,       //LendOrder
}

// impl std::fmt::Display for StreamId {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         write!(f, "{:?}", self)
//         // or, alternatively:
//         // fmt::Debug::fmt(self, f)
//     }
// }

pub fn init_aeron_queue() {
    for streamid in StreamId::iter() {
        // println!("My favorite color is {:?}", streamid as i32);
        let streamid_clone = streamid.clone();
        std::thread::spawn(move || {
            start_aeron_topic_consumer(streamid);
        });
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            start_aeron_topic_producer(streamid_clone);
        });
    }
}

// thread::spawn(move || {
//     start_aeron_topic_consumer(StreamId::CreateTraderOrder);
// });
// thread::spawn(move || {
//     thread::sleep(time::Duration::from_millis(10));
//     start_aeron_topic_producer(StreamId::CreateTraderOrder);
// });

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AeronMessage {
    pub stream_id: i32,
    pub session_id: i32,
    pub length: Index,
    pub offset: Index,
    pub msg: String,
}

impl AeronMessage {
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }
    pub fn deserialize(json: &String) -> Self {
        let deserialized: AeronMessage = serde_json::from_str(json).unwrap();
        deserialized
    }
    pub fn extract_msg(self) -> String {
        self.msg
    }
}

#[derive(Debug, Clone)]
pub struct AeronMessageMPSC {
    pub sender: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::SyncSender<AeronMessage>>>,
    pub receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<AeronMessage>>>,
}

impl AeronMessageMPSC {
    pub fn send(
        &self,
    ) -> &std::sync::Arc<std::sync::Mutex<std::sync::mpsc::SyncSender<AeronMessage>>> {
        &self.sender
    }
    pub fn rec(&self) -> std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<AeronMessage>>> {
        self.receiver.clone()
    }
}
