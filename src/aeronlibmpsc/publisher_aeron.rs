use crate::aeronlibmpsc::types::StreamId;
use aeron_rs::{
    aeron::Aeron,
    concurrent::{
        atomic_buffer::{AlignedBuffer, AtomicBuffer},
        status::status_indicator_reader::channel_status_to_str,
    },
    context::Context,
};
use std::{
    ffi::CString,
    io::{stdout, Write},
    time::Duration,
};

extern crate nix;

use nix::NixPath;

#[derive(Clone, Debug)]
struct Settings {
    dir_prefix: String,
    channel: String,
    stream_id: i32,
}

impl Settings {
    pub fn new(channel: &str, topic: StreamId, dir: &str) -> Self {
        Self {
            dir_prefix: String::from(dir),
            channel: String::from(channel),
            stream_id: topic as i32,
        }
    }
}

fn str_to_c(val: &str) -> CString {
    CString::new(val).expect("Error converting str to CString")
}

pub fn pub_aeron(
    topic: StreamId,
    receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<String>>>,
) {
    let topic_clone = topic.clone();
    let settings = Settings::new(
        "aeron:udp?endpoint=localhost:40123",
        topic,
        "/dev/shm/aeron-default",
    );

    let mut context = Context::new();
    // println!("context1 {:#?}", settings.dir_prefix);
    if !settings.dir_prefix.is_empty() {
        context.set_aeron_dir(settings.dir_prefix.clone());
    }

    // context.set_new_publication_handler(Box::new(on_new_publication_handler));
    // context.set_error_handler(Box::new(error_handler));
    context.set_pre_touch_mapped_memory(true);

    let aeron = Aeron::new(context);

    if aeron.is_err() {
        println!("Error creating Aeron instance: {:?}", aeron.err());
        return;
    }

    let mut aeron = aeron.unwrap();

    // add the publication to start the process
    let publication_id = aeron
        .add_publication(str_to_c(&settings.channel), settings.stream_id)
        .expect("Error adding publication");
    let mut publication = aeron.find_publication(publication_id);
    while publication.is_err() {
        std::thread::yield_now();
        publication = aeron.find_publication(publication_id);
    }

    let publication = publication.unwrap();

    let channel_status = publication.lock().unwrap().channel_status();

    println!(
        "Publication channel status {}: {}, topic:{:#?}  ",
        channel_status,
        channel_status_to_str(channel_status),
        topic_clone
    );

    let buffer = AlignedBuffer::with_capacity(1024);
    let src_buffer = AtomicBuffer::from_aligned(&buffer);

    while !publication.lock().unwrap().is_connected() {
        // println!("No active subscribers detected");
        std::thread::sleep(Duration::from_millis(100));
    }

    loop {
        let message = receiver.lock().unwrap().recv().unwrap();
        let str_msg = format!("{}", message);
        let c_str_msg = CString::new(str_msg).unwrap();
        src_buffer.put_bytes(0, c_str_msg.as_bytes());

        let _unused = stdout().flush();

        let result = publication
            .lock()
            .unwrap()
            .offer_part(src_buffer, 0, c_str_msg.len() as i32);

        if let Ok(_code) = result {
            // println!("Sent with code {}!", code);
        } else {
            println!("Offer with error: {:?}", result.err());
        }
    }
}
