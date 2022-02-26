use crate::aeronlib::{publisher_aeron, subscriber_aeron};
use crate::config::AERON_BROADCAST;
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

pub fn start_aeron() {
    thread::spawn(move || {
        publisher_aeron::pub_aeron();
    });
    thread::spawn(move || {
        subscriber_aeron::sub_aeron();
    });

    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}
