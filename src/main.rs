mod perpetual;
use perpetual::interface::{
    getandupdateallordersonfundingcycle, updatefundingrate, OrderStatus, OrderType, PositionType,
    TraderOrder,
};
extern crate tpf;
mod ordertest;
extern crate stopwatch;
use std::{thread, time};
use stopwatch::Stopwatch;
use tpf::redislib::redis_db;

fn main() {
    let one_sec = time::Duration::from_millis(1000);

    let sw = Stopwatch::start_new();

    let o: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::MARKET,
        10.0,
        1000.0,
        1000.0,
        OrderStatus::PENDING,
        40000.00,
        0.0,
    )
    .newtraderorderinsert(); //only call this function if docker is running with postgresql
    ordertest::generateorder();

    // println!("Thing took {}ms", sw.elapsed_ms());
    println!("Thing took {:#?}", sw.elapsed());

    println!("1st: {:#?}", o);

    // {"uuid":"3537d499-9498-41f4-9a12-0f25f4864195","account_id":"account_id","position_type":"SHORT","position_side":1,"order_status":"PENDING","order_type":"MARKET","entryprice":42514.01,"execution_price":0.0,"positionsize":3231277330.05,"leverage":5.0,"initial_margin":15201.0,"available_margin":15201.0,"timestamp":1642167758764,"bankruptcy_price":53142.512500000004,"bankruptcy_value":60804.0,"maintenance_margin":320.43708,"liquidation_price":52863.919643478876}
    println!("2nd: {}", o.serialize());

    // thread::sleep(one_sec);

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
    thread::sleep(one_sec);
    redis_db::set(&"CurrentPrice", &"40400.00");
    thread::sleep(one_sec);
    o.calculatepayment();
    // getandupdateallordersonfundingcycle();
    thread::sleep(one_sec);
}
