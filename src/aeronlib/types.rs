use aeron_rs::utils::types::Index;
// use mpsc::{channel, Receiver, Sender};
use serde_derive::Deserialize;
use serde_derive::Serialize;
// use std::sync::{mpsc, Arc, Mutex};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum StreamId {
    CreateOrder = 1001, //TraderOrder
    AERONMSGTWO = 1002, //LendOrder
}

// impl std::fmt::Display for StreamId {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         write!(f, "{:?}", self)
//         // or, alternatively:
//         // fmt::Debug::fmt(self, f)
//     }
// }

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
