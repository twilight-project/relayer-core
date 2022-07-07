// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
// mod aeronlib;
// mod aeronlibmpsc;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;

use crate::config::{
    LOCALDB, ORDERTEST, QUESTDB_POOL_CONNECTION, REDIS_POOL_CONNECTION, THREADPOOL,
};
use config::local_serial_core;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::{redis_db, redis_db_orderbook};
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

// fn main() {
//     // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
//     // println!("time:{}", relayer::check_server_time());
//     // relayer::get_fudning_data_from_psql(10);

//     init_psql();
//     ordertest::initprice();
//     ordertest::generatelendorder();
//     thread::sleep(time::Duration::from_millis(100));
//     start_cronjobs();
//     // thread::sleep(time::Duration::from_millis(3000));
//     // let sw = Stopwatch::start_new();
//     // println!("{}", relayer::get_localdb_string("OrderBook"));
//     // let time_ec = sw.elapsed();
//     // println!("time: {:#?} ", time_ec);
//     loop {
//         thread::sleep(time::Duration::from_millis(100000000));
//     }

//     // let sw = Stopwatch::start_new();
//     // relayer::get_latest_orderbook();
//     // let time_ec = sw.elapsed();
//     // println!("time: {:#?} ", time_ec);
// }
// use chrono::Utc;
// use serde_json::json;

use std::io::prelude::*;
// use std::net::TcpStream;
use std::net::{Shutdown, TcpStream};

fn main() {
    // thread::sleep(time::Duration::from_millis(2000));

    // let bytes_written =
    // stream.write(b"recentorders,side=0, price=19141.47, amount=287122.05000000005 1e6/n")?;
    // for i in 1..10 {
    //     let bytes_written = stream.write(
    //         b"recentorders,side=0,price=19141.47,amount=287122.05000000005 1655968892947000\n",
    //     )?;
    //     // stream.read(&mut [0; 128])?;
    // }
    // for i in 1..10 {
    //     stream
    //         .write(
    //             b"recentorders,side=0,price=19141.47,amount=287123.05000000005 1655968892947000\n",
    //         )
    //         .unwrap();
    // }
    // stream
    //     .write(b"recentorders,side=0,price=18141.47,amount=287122.05000000005 1655968892948")
    //     .unwrap();
    // // thread::sleep(time::Duration::from_millis(100));
    // stream.flush()?;

    // let data0 = b"recentorders,side=0,price=18141.47,amount=287122.05000000005 1655968892948\n";
    // let data = b"recentorders side=5i,price=1814.47,amount=287122.05005 1556813561098000000\n";
    // // let bytes_written = stream.write(data0).unwrap();
    // // let bytes_written = stream.write(data0).unwrap();
    // // let bytes_written = stream.write(data0).unwrap();
    // for i in 1..10 {
    //     let bytes_written = stream.write(data).unwrap();
    // }

    // if bytes_written < data.len() {
    //     println!(
    //         "{:#?}",
    //         format!("Sent {}/{} bytes", bytes_written, data.len())
    //     );
    // }
    let data1 = b"recentorders side=6i,price=1814.47,amount=287122.05005 1556813561098000000\n";

    // stream.flush().unwrap();
    // thread::sleep(time::Duration::from_millis(60000));
    // for i in 1..100000 {
    //     let bytes_written = stream.write(data1).unwrap();
    // }
    // stream.flush().unwrap();

    // stream.shutdown(Shutdown::Both);
    // Ok(())
    let mut client = QUESTDB_POOL_CONNECTION.get().unwrap();
    let sw = Stopwatch::start_new();
    let mut stream = TcpStream::connect("127.0.0.1:9009").unwrap();
    for i in 1..1000 {
        stream.write(data1).unwrap();
    }
    stream.flush().unwrap();

    // for i in 1..1000 {
    //     let query = format!(
    //         "INSERT INTO recentorders VALUES (10, 1814.47,287122.05005,1556813561098000000)"
    //     );
    //     client.execute(&query, &[]).unwrap();
    // }
    let k = sw.elapsed();
    println!("time : {:#?}", k);
    // drop(client);
}
//1655968892947000
