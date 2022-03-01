use aeron_rs::utils::types::Index;
// use mpsc::{channel, Receiver, Sender};
use serde_derive::Deserialize;
use serde_derive::Serialize;
// use std::sync::{mpsc, Arc, Mutex};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum StreamId {
    AERONMSG = 1001,    //TraderOrder
    AERONMSGTWO = 1002, //LendOrder
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

/// Valid types of messages (in the default implementation)
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Type {
    /// Message with UTF8 test
    Text = 1,
    /// Message containing binary data
    Binary = 2,
    /// Ping message with data
    Ping = 9,
    /// Pong message with data
    Pong = 10,
    /// Close connection message with optional reason
    Close = 8,
}

// #[derive(Eq, PartialEq, Clone, Debug)]
// pub enum AeronMessage {
//     /// A message containing UTF-8 text data
//     Text(String),
//     /// A message containing binary data
//     Binary(Vec<u8>),
//     /// A ping message - should be responded to with a pong message.
//     /// Usually the pong message will be sent with the same data as the
//     /// received ping message.
//     Ping(Vec<u8>),
//     /// A pong message, sent in response to a Ping message, usually
//     /// containing the same data as the received ping message.
//     Pong(Vec<u8>),
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AeronMessage {
    pub stream_id: i32,
    pub session_id: i32,
    pub length: Index,
    pub offset: Index,
    pub msg: String,
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
