mod relayer;
use relayer::*;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod redislib;
mod utils;
extern crate stopwatch;
use std::{thread, time};
use stopwatch::Stopwatch;

#[macro_use]
extern crate lazy_static;

fn main() {
    let one_sec = time::Duration::from_millis(1000);

    let sw = Stopwatch::start_new();

    // ordertest::initprice();
    // loop {
    // thread::sleep(one_sec);
    // thread::sleep(one_sec);
    // thread::sleep(one_sec);

    // ordertest::generatelendorder();
    // }
    ordertest::generateorder();

    // println!("Thing took {}ms", sw.elapsed_ms());
    println!("Thing took {:#?}", sw.elapsed());

    thread::sleep(one_sec);

    // println!(
    //     "{:#?}",
    //     redis_db::zrangelonglimitorderbyexecutionprice(50000.0)
    // );
    // println!(
    //     " {:#?}",
    //     redis_db::zrangeshortlimitorderbyexecutionprice(39000.0)
    // );

    // println!(" {:#?}", redis_db::zrangegetliquidateorderforshort(52300.0));
    // println!(" {:#?}", redis_db::zrangegetliquidateorderforlong(35500.0));

    // updatefundingrate(0.1);
    // redis_db::set(&"CurrentPrice", &"40400.00");
    // o.calculatepayment();
    getandupdateallordersonfundingcycle();
    // thread::sleep(one_sec);
}
