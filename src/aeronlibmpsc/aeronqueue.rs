use crate::aeronlibmpsc::types::{AeronDirectMessage, AeronMessage, AeronMessageMPSC, StreamId};
use crate::aeronlibmpsc::{publisher_aeron, subscriber_aeron};
// use crate::config::{AERONTOPICCONSUMERHASHMAPMPSC, AERONTOPICPRODUCERHASHMAP};
use crate::aeronlibmpsc::types::{AERONTOPICCONSUMERHASHMAPMPSC, AERONTOPICPRODUCERHASHMAP};
use crate::relayer;
use crate::relayer::ThreadPool;
use mpsc::{channel, Receiver, SyncSender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

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

// pub fn send_aeron_msg(topic: StreamId, msg: String) {
//     let aeron_topic_hashmap = AERONTOPICPRODUCERHASHMAP.lock().unwrap();
//     let topic_sender = aeron_topic_hashmap.get(&(topic as i32)).unwrap().clone();
//     drop(aeron_topic_hashmap);
//     topic_sender.lock().unwrap().send(msg).unwrap();
// }

pub fn send_aeron_msg(topic: StreamId, msg: String) {
    let direct_msg = AeronDirectMessage {
        topic: topic.clone(),
        msg,
    };
    let aeron_topic_hashmap = AERONTOPICPRODUCERHASHMAP.lock().unwrap();
    let topic_sender = aeron_topic_hashmap
        .get(&(StreamId::DirectStreamId as i32))
        .unwrap()
        .clone();
    drop(aeron_topic_hashmap);
    topic_sender
        .lock()
        .unwrap()
        .send(direct_msg.serialize())
        .unwrap();
}

pub fn start_aeron_topic_consumer(topic: StreamId) {
    let topic_clone = topic.clone();
    let (sender, receiver): (SyncSender<AeronMessage>, Receiver<AeronMessage>) =
        mpsc::sync_channel(1);
    let receiver = Arc::new(Mutex::new(receiver));
    let sender = Arc::new(Mutex::new(sender));
    let mut aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAPMPSC.lock().unwrap();
    aeron_topic_consumer_hashmap.insert(topic as i32, AeronMessageMPSC { sender, receiver });
    drop(aeron_topic_consumer_hashmap);
    subscriber_aeron::sub_aeron(topic_clone);
}

pub fn rec_aeron_msg(topic: StreamId) -> AeronMessage {
    let aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAPMPSC.lock().unwrap();
    let receiver = aeron_topic_consumer_hashmap
        .get(&(StreamId::DirectStreamId as i32))
        .unwrap()
        .rec()
        .clone();
    drop(aeron_topic_consumer_hashmap);
    let msg: AeronMessage = receiver.lock().unwrap().recv().unwrap();
    msg
}
pub fn rec_aeron_msg_direct() {
    let create_trader_order_thread_pool: ThreadPool = ThreadPool::new(3);
    let create_or_execute_lend_order_thread_pool: ThreadPool = ThreadPool::new(1);
    let cancel_trader_order_thread_pool: ThreadPool = ThreadPool::new(1);
    let execute_trader_order_thread_pool: ThreadPool = ThreadPool::new(3);
    loop {
        let aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAPMPSC.lock().unwrap();
        let receiver = aeron_topic_consumer_hashmap
            .get(&(StreamId::DirectStreamId as i32))
            .unwrap()
            .rec()
            .clone();
        drop(aeron_topic_consumer_hashmap);
        let msg: AeronMessage = receiver.lock().unwrap().recv().unwrap();
        let new_msg = AeronDirectMessage::deserialize(&msg.extract_msg());

        match new_msg.topic {
            StreamId::CreateTraderOrder => {
                create_trader_order_thread_pool
                    .execute(move || relayer::get_new_trader_order(new_msg.msg));
            }

            StreamId::CreateLendOrder => {
                create_or_execute_lend_order_thread_pool
                    .execute(move || relayer::get_new_lend_order(new_msg.msg));
            }
            StreamId::CancelTraderOrder => {
                cancel_trader_order_thread_pool
                    .execute(move || relayer::cancel_trader_order(new_msg.msg));
            }
            StreamId::ExecuteTraderOrder => {
                execute_trader_order_thread_pool
                    .execute(move || relayer::execute_trader_order(new_msg.msg));
            }
            StreamId::ExecuteLendOrder => {
                create_or_execute_lend_order_thread_pool
                    .execute(move || relayer::execute_lend_order(new_msg.msg));
            }
            StreamId::GetPoolShare => println!("GetPoolShare {:#?}", new_msg.msg),
            _ => println!("Rest of the number"),
        }
    }
}
