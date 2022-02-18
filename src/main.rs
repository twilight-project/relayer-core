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
use crate::config::{LOCALDB, ORDERTEST, REDIS_POOL_CONNECTION, THREADPOOL};
//
use r2d2_redis::redis;
use std::process::Command;
//

use config::local_serial_core;
// use redis::Commands;
use redislib::redis_db;
use relayer::*;
use std::{thread, time};

// use std::env;
// use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;

// fn main() {
//     let one_sec = time::Duration::from_millis(1000);
//     // let handle = runloop_price_ticker();
//     // loop {
//     // let sw = Stopwatch::start_new();
//     ordertest::initprice();
//     thread::sleep(one_sec);

//     // let sw = Stopwatch::start_new();
//     LendOrder::new(
//         "Lend account_id",
//         10.0,
//         OrderType::LEND,
//         OrderStatus::PENDING,
//         1.01,
//     );
//     // .newtraderorderinsert();

//     // println!("pool took {:#?}", sw.elapsed());

//     // start_cronjobs();
//     // println!(" {:#?}", redis_db::zrangegetliquidateorderforshort(52300.0));
//     // println!(" {:#?}", redis_db::zrangegetliquidateorderforlong(43000.0));

//     // updatefundingrate(0.1);
//     // redis_db::set(&"CurrentPrice", &"38000.0");
//     // o.calculatepayment();
//     // get_and_update_all_orders_on_funding_cycle();
//     // let pool = THREADPOOL.lock().unwrap();
//     // // drop(local_storage);
//     // pool.execute(|| {
//     //     ordertest::generatelendorder(1);
//     // });
//     // thread::sleep(one_sec);
//     // drop(pool);

//     // let pool2 = ThreadPool::new(1);
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(1);
//     // });
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(2);
//     // });
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(3);
//     // });
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(4);
//     // });
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(5);
//     // });
//     // pool2.execute(|| {
//     //     ordertest::generatelendorder(6);
//     // });
//     thread::sleep(one_sec);
//     thread::sleep(one_sec);

//     // handle.join().unwrap();
//     // loop {
//     //     thread::sleep(time::Duration::from_millis(100000000));
//     // }
// }

// // fn main() {
// //     let one_sec = time::Duration::from_millis(1000);
// //     ordertest::initprice();
// //     // let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
// //     start_cronjobs();

// //     let sw = Stopwatch::start_new();
// //     // set_localdb("CurentPrice", 40023.12);
// //     // let current_price = get_localdb("CurrentPrice");
// //     // println!("{:#?} : {:#?}", price, btcprice);

// //     // TraderOrder::new(
// //     //     "account_id",
// //     //     PositionType::LONG,
// //     //     OrderType::MARKET,
// //     //     5.0,
// //     //     1.0201,
// //     //     1.0201,
// //     //     OrderStatus::PENDING,
// //     //     42500.01,
// //     //     33440.02,
// //     // )
// //     // .newtraderorderinsert();

// //     // let (k1, k2, k3): (i32, i32, f64) = redis::pipe()
// //     //     .cmd("SET")
// //     //     .arg("LendNonce")
// //     //     .arg(5)
// //     //     .ignore()
// //     //     .cmd("SET")
// //     //     .arg("LendNonce2")
// //     //     .arg(6)
// //     //     .ignore()
// //     //     .cmd("GET")
// //     //     .arg("LendNonce")
// //     //     .cmd("GET")
// //     //     .arg("LendNonce2")
// //     //     .cmd("GET")
// //     //     .arg("CurrentPrice")
// //     //     .query(&mut *conn)
// //     //     .unwrap();
// //     // let rev_data: Vec<f64> = redis_db::mget_f64(vec!["CurrentPrice", "btc:price"]);
// //     // let (currentprice, btc_price) = (rev_data[0], rev_data[1]);
// //     // println!("{:#?}", currentprice);
// //     // // let x = redis_db::get_type_f64("CurrentPrice");
// //     // println!("mutex took {:#?}", sw.elapsed());
// //     // println!("mutex took {:#?}, and {}", sw.elapsed(), bar);
// //     thread::sleep(one_sec);
// //     thread::sleep(one_sec);
// //     loop {
// //         thread::sleep(time::Duration::from_millis(100000000));
// //     }
// // }

use jsonrpc_http_server::jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::ServerBuilder;

fn main() {
    let mut io = IoHandler::default();
    io.add_method("say_hello", move |params: Params| async move {
        let x: LendOrder = params.parse().unwrap();
        println!("{:#?}", x);
        Ok(Value::String(
            LendOrder::new(
                "Lend account_id",
                10.0,
                OrderType::LEND,
                OrderStatus::PENDING,
                1.01,
            )
            .serialize(),
        ))
    });

    let server = ServerBuilder::new(io)
        .threads(3)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .unwrap();

    server.wait();
}
