use std::{
    ffi::CString,
    slice,
    sync::atomic::{AtomicBool, AtomicI64, Ordering},
};

use crate::aeronlib::types::{AeronMessage, StreamId};
use crate::config::AERONTOPICCONSUMERHASHMAP;
use aeron_rs::{
    aeron::Aeron,
    concurrent::{
        atomic_buffer::AtomicBuffer,
        logbuffer::header::Header,
        status::status_indicator_reader::channel_status_to_str,
        strategies::{SleepingIdleStrategy, Strategy},
    },
    context::Context,
    image::Image,
    utils::{errors::AeronError, types::Index},
};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref RUNNING: AtomicBool = AtomicBool::from(true);
    pub static ref SUBSCRIPTION_ID: AtomicI64 = AtomicI64::new(-1);
}

#[derive(Clone)]
struct Settings {
    dir_prefix: String,
    channel: String,
    stream_id: i32,
}

impl Settings {
    pub fn new(channel: &str, topic: StreamId) -> Self {
        Self {
            dir_prefix: String::new(),
            channel: String::from(channel),
            stream_id: topic as i32,
        }
    }
}

fn available_image_handler(image: &Image) {
    println!(
        "Available image correlation_id={} session_id={} at position={} from {}",
        image.correlation_id(),
        image.session_id(),
        image.position(),
        image.source_identity().to_str().unwrap()
    );
}

fn unavailable_image_handler(image: &Image) {
    println!(
        "Unavailable image correlation_id={} session_id={} at position={} from {}",
        image.correlation_id(),
        image.session_id(),
        image.position(),
        image.source_identity().to_str().unwrap()
    );
}

fn error_handler(error: AeronError) {
    println!("Error: {:?}", error);
}

fn str_to_c(val: &str) -> CString {
    CString::new(val).expect("Error converting str to CString")
}

pub fn sub_aeron(topic: StreamId) {
    let settings = Settings::new("aeron:udp?endpoint=localhost:40123", topic);

    let mut context = Context::new();

    if !settings.dir_prefix.is_empty() {
        context.set_aeron_dir(settings.dir_prefix.clone());
    }

    context.set_new_subscription_handler(Box::new(
        |channel: CString, stream_id: i32, correlation_id: i64| {
            println!(
                "Subscription: {} {} {}",
                channel.to_str().unwrap(),
                stream_id,
                correlation_id
            )
        },
    ));
    context.set_available_image_handler(Box::new(available_image_handler));
    context.set_unavailable_image_handler(Box::new(unavailable_image_handler));
    context.set_error_handler(Box::new(error_handler));
    context.set_pre_touch_mapped_memory(true);

    let aeron = Aeron::new(context);

    if aeron.is_err() {
        println!("Error creating Aeron instance: {:?}", aeron.err());
        return;
    }

    let mut aeron = aeron.unwrap();

    let subscription_id = aeron
        .add_subscription(str_to_c(&settings.channel), settings.stream_id)
        .expect("Error adding subscription");

    SUBSCRIPTION_ID.store(subscription_id, Ordering::SeqCst);

    let mut subscription = aeron.find_subscription(subscription_id);
    while subscription.is_err() {
        std::thread::yield_now();
        subscription = aeron.find_subscription(subscription_id);
    }

    let subscription = subscription.unwrap();

    let channel_status = subscription.lock().expect("Fu").channel_status();

    println!(
        "Subscription channel status {}: {}",
        channel_status,
        channel_status_to_str(channel_status)
    );

    let idle_strategy = SleepingIdleStrategy::new(10);

    loop {
        let fragments_read = subscription.lock().expect("Fu").poll(
            &mut |buffer: &AtomicBuffer, offset: Index, length: Index, header: &Header| unsafe {
                let slice_msg = slice::from_raw_parts_mut(
                    buffer.buffer().offset(offset as isize),
                    length as usize,
                );
                let msg = CString::new(slice_msg).unwrap();
                let aeron_topic_consumer_hashmap = AERONTOPICCONSUMERHASHMAP.lock().unwrap();
                let aeron_topic_consumer_hashmap_clone = aeron_topic_consumer_hashmap.clone();
                drop(aeron_topic_consumer_hashmap);
                let att = aeron_topic_consumer_hashmap_clone
                    .get(&header.stream_id())
                    .unwrap();
                att.send()
                    .lock()
                    .unwrap()
                    .send(AeronMessage {
                        stream_id: header.stream_id(),
                        session_id: header.session_id(),
                        length: length,
                        offset: offset,
                        msg: msg.to_str().unwrap().to_string(),
                    })
                    .unwrap();
            },
            10,
        );
        idle_strategy.idle_opt(fragments_read);
    }
}
