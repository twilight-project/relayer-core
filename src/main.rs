// extern crate stopwatch;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod relayer;
mod utils;
use crate::config::ORDERTEST;

// use clokwerk::Interval::*;
// use clokwerk::{Scheduler, TimeUnits};
use config::local_serial_core;
// use redislib::redis_db;
use relayer::*;
use std::{thread, time};

use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;

// fn main() {
//     let one_sec = time::Duration::from_millis(1000);
//     // let handle = runloop_price_ticker();
//     // loop {
//     // let sw = Stopwatch::start_new();
//     ordertest::initprice();
//     ordertest::generatelendorder();
//     thread::sleep(one_sec);

//     start_cronjobs();

//     // println!(" {:#?}", redis_db::zrangegetliquidateorderforshort(52300.0));
//     // println!(" {:#?}", redis_db::zrangegetliquidateorderforlong(35500.0));

//     // updatefundingrate(0.1);
//     // redis_db::set(&"CurrentPrice", &"38000.0");
//     // o.calculatepayment();
//     // get_and_update_all_orders_on_funding_cycle();
//     thread::sleep(one_sec);
//     // handle.join().unwrap();
//     loop {
//         thread::sleep(time::Duration::from_millis(100000000));
//     }
// }

fn main() {
    let one_sec = time::Duration::from_millis(1000);
    ordertest::initprice();
    let sw = Stopwatch::start_new();

    let xx = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        5.0,
        1.0201,
        1.0201,
        OrderStatus::PENDING,
        42500.01,
        33440.02,
    );
    // .newtraderorderinsert();
    println!("mutex took {:#?}", sw.elapsed());
    thread::sleep(one_sec);
    thread::sleep(one_sec);
    thread::sleep(one_sec);
}
