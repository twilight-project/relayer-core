use crate::aeronlib::types::{AeronMessage, AeronMessageMPSC, StreamId};
use crate::aeronlib::{publisher_aeron, subscriber_aeron};
use crate::config::AERON_BROADCAST;
use crate::config::{
    AERONTOPICCONSUMERHASHMAP, AERONTOPICPRODUCERHASHMAP, DEFAULT_CHANNEL, DEFAULT_STREAM_ID,
};

use mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use serde_derive::{Deserialize, Serialize};
use std::sync::{mpsc, Arc, Mutex};

use std::{thread, time};

pub fn aeron_send(message: String) {
    let send = AERON_BROADCAST.0.lock().unwrap();
    send.send(message).unwrap();
    drop(send);
}

pub fn aeron_rec() -> String {
    let receive = AERON_BROADCAST.1.lock().unwrap();
    return receive.recv().unwrap();
}

pub fn start_aeron_topic_producer(topic: StreamId) {
    let topic_clone_sender = topic.clone();
    let (sender, receiver) = channel();
    let receiver = Arc::new(Mutex::new(receiver));
    thread::spawn(move || {
        publisher_aeron::pub_aeron(topic, receiver);
    });
    let mut aeron_topic_hashmap = AERONTOPICPRODUCERHASHMAP.lock().unwrap();
    aeron_topic_hashmap.insert(topic_clone_sender as i32, Arc::new(Mutex::new(sender)));
    drop(aeron_topic_hashmap);
}

pub fn send_aeron_msg(topic: StreamId, msg: String) {
    let aeron_topic_hashmap = AERONTOPICPRODUCERHASHMAP.lock().unwrap();
    let topic_sender = aeron_topic_hashmap.get(&(topic as i32)).unwrap().clone();
    drop(aeron_topic_hashmap);
    topic_sender.lock().unwrap().send(msg).unwrap();
}

pub fn start_aeron_topic_consumer(topic: StreamId) {
    let topic_clone = topic.clone();
    let (sender, receiver): (SyncSender<AeronMessage>, Receiver<AeronMessage>) =
        mpsc::sync_channel(1);
    let receiver = Arc::new(Mutex::new(receiver));
    let sender = Arc::new(Mutex::new(sender));
    let mut aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAP.lock().unwrap();
    aeron_topic_consumer_hashmap.insert(topic as i32, AeronMessageMPSC { sender, receiver });
    drop(aeron_topic_consumer_hashmap);
    subscriber_aeron::sub_aeron(topic_clone);
}

pub fn rec_aeron_msg(topic: StreamId) -> AeronMessage {
    let aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAP.lock().unwrap();
    let receiver = aeron_topic_consumer_hashmap
        .get(&(topic as i32))
        .unwrap()
        .rec()
        .clone();
    drop(aeron_topic_consumer_hashmap);
    let msg: AeronMessage = receiver.lock().unwrap().recv().unwrap();
    msg
}
