// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;
mod utils;
use crate::config::{LOCALDB, ORDERTEST, REDIS_POOL_CONNECTION};
//
use r2d2_redis::redis;
use std::process::Command;
//

use config::local_serial_core;
// use redis::Commands;
use redislib::redis_db;
use relayer::*;
use std::{thread, time};

use std::env;
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;

fn main() {
    let one_sec = time::Duration::from_millis(1000);
    // let handle = runloop_price_ticker();
    // loop {
    // let sw = Stopwatch::start_new();
    ordertest::initprice();
    ordertest::generatelendorder();
    thread::sleep(one_sec);

    start_cronjobs();
    // println!(" {:#?}", redis_db::zrangegetliquidateorderforshort(52300.0));
    // println!(" {:#?}", redis_db::zrangegetliquidateorderforlong(43000.0));

    // updatefundingrate(0.1);
    // redis_db::set(&"CurrentPrice", &"38000.0");
    // o.calculatepayment();
    // get_and_update_all_orders_on_funding_cycle();
    thread::sleep(one_sec);
    // handle.join().unwrap();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
}

// fn main() {
//     let one_sec = time::Duration::from_millis(1000);
//     ordertest::initprice();
//     // let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
//     start_cronjobs();

//     let sw = Stopwatch::start_new();
//     // set_localdb("CurentPrice", 40023.12);
//     // let current_price = get_localdb("CurrentPrice");
//     // println!("{:#?} : {:#?}", price, btcprice);

//     // TraderOrder::new(
//     //     "account_id",
//     //     PositionType::LONG,
//     //     OrderType::MARKET,
//     //     5.0,
//     //     1.0201,
//     //     1.0201,
//     //     OrderStatus::PENDING,
//     //     42500.01,
//     //     33440.02,
//     // )
//     // .newtraderorderinsert();

//     // let (k1, k2, k3): (i32, i32, f64) = redis::pipe()
//     //     .cmd("SET")
//     //     .arg("LendNonce")
//     //     .arg(5)
//     //     .ignore()
//     //     .cmd("SET")
//     //     .arg("LendNonce2")
//     //     .arg(6)
//     //     .ignore()
//     //     .cmd("GET")
//     //     .arg("LendNonce")
//     //     .cmd("GET")
//     //     .arg("LendNonce2")
//     //     .cmd("GET")
//     //     .arg("CurrentPrice")
//     //     .query(&mut *conn)
//     //     .unwrap();
//     // let rev_data: Vec<f64> = redis_db::mget_f64(vec!["CurrentPrice", "btc:price"]);
//     // let (currentprice, btc_price) = (rev_data[0], rev_data[1]);
//     // println!("{:#?}", currentprice);
//     // // let x = redis_db::get_type_f64("CurrentPrice");
//     // println!("mutex took {:#?}", sw.elapsed());
//     // println!("mutex took {:#?}, and {}", sw.elapsed(), bar);
//     thread::sleep(one_sec);
//     thread::sleep(one_sec);
//     loop {
//         thread::sleep(time::Duration::from_millis(100000000));
//     }
// }
