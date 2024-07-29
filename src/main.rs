// mod config;
// mod db;
// mod kafkalib;
// mod ordertest;
// mod postgresqllib;
// mod pricefeederlib;
// mod questdb;
// mod redislib;
// mod relayer;
// use config::CORE_EVENT_LOG;
// use db::snapshot;
// use kafka::client::FetchOffset;
// use relayer::*;
// use std::{process, thread, time};
// #[macro_use]
// extern crate lazy_static;

mod config;
mod db;
mod kafkalib;
// mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;
use config::CORE_EVENT_LOG;
use db::{snapshot, Event};
use kafka::client::FetchOffset;
use relayer::*;
use std::{
    process, thread,
    time::{self, SystemTime},
};
#[macro_use]
extern crate lazy_static;

fn main() {
    dotenv::dotenv().ok();
    // println!("{:#?}", kafkalib::kafkacmd::check_kafka_topics());
    heartbeat();
    loop {
        thread::sleep(time::Duration::from_millis(10000));
        if get_relayer_status() {
        } else {
            thread::sleep(time::Duration::from_millis(5000));
            println!("Relayer relayer started taking snapshot");
            let _ = snapshot();
            thread::sleep(time::Duration::from_millis(10000));
            println!("Relayer Shutting down");
            process::exit(0);
        }
    }

    // let time = ServerTime::now().epoch;
    // let event_timestamp = time.clone();
    // let event_stoper_string = format!("snapsot-start-{}", time);
    // let eventstop: Event = Event::Stop(event_stoper_string.clone());
    // Event::send_event_to_kafka_queue(
    //     eventstop.clone(),
    //     CORE_EVENT_LOG.clone().to_string(),
    //     String::from("StopLoadMSGtest_kafka"),
    // );
    // let time_offset: i64 = SystemTime::now()
    //     .duration_since(SystemTime::UNIX_EPOCH)
    //     .unwrap()
    //     .as_millis()
    //     .try_into()
    //     .unwrap();

    // let fetchoffset = FetchOffset::Earliest;
    // let mut offset = 0;
    // let fetchoffset = FetchOffset::ByTime(time_offset);
    // let (receiver, tx_consumed) = Event::receive_event_for_snapshot_from_kafka_queue(
    //     CORE_EVENT_LOG.clone().to_string(),
    //     // format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
    //     "test_kafka".to_string(),
    //     fetchoffset,
    //     "test_group",
    // )
    // .unwrap();
    // let recever1 = receiver.lock().unwrap();
    // loop {
    //     match recever1.recv() {
    //         Ok(data) => match data.value {
    //             Event::Stop(timex) => {
    //                 if timex == event_stoper_string {
    //                     println!("last data:{:?}", data.offset);
    //                     tx_consumed.send((data.partition, data.offset)).unwrap();
    //                     // break;
    //                 }
    //             }
    //             _ => {
    //                 // println!("data:{:?}", data.offset);
    //                 // thread::sleep(time::Duration::from_millis(500));
    //                 if data.offset % 10000 == 0 {
    //                     tx_consumed.send((data.partition, data.offset)).unwrap();
    //                 }
    //                 if offset > data.offset {
    //                     println!("wrong offset :{:?}", data.offset);
    //                 }
    //                 offset = data.offset;
    //             }
    //         },
    //         Err(_) => {}
    //     }
    // }
    // thread::sleep(time::Duration::from_millis(5000));
}
