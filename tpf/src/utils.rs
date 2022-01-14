#![allow(dead_code)]
use std::thread;
use std::time;

pub fn delay_in_millisecond(millisecond: u64) {
    thread::sleep(time::Duration::from_millis(millisecond));
}
