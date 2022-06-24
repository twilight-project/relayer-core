extern crate twilight_relayer_rust;
use crate::twilight_relayer_rust::config::local_serial_core;
use crate::twilight_relayer_rust::redislib::redis_db;
use crate::twilight_relayer_rust::relayer::*;
use std::{thread, time};
extern crate lazy_static;

// create new Trader order // all utils function for trader order will also gets tested again

fn test_traderorder_new() {
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(50));

    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    thread::sleep(time::Duration::from_millis(50));

    assert_eq!(totx.position_type, PositionType::LONG);
    assert_eq!(totx.order_status, OrderStatus::PENDING);
    assert_eq!(totx.order_type, OrderType::LIMIT);
    assert_eq!(totx.entryprice, 39000.01);
    assert_eq!(totx.execution_price, 39000.01);
    assert_eq!(totx.positionsize, 585000.15);
    assert_eq!(totx.leverage, 10.0);
    assert_eq!(totx.initial_margin, 1.5);
    assert_eq!(totx.available_margin, 1.5);
    assert_eq!(totx.bankruptcy_price, 35454.55454545455);
    assert_eq!(totx.bankruptcy_value, 16.5);
    assert_eq!(totx.maintenance_margin, 0.06);
    assert_eq!(totx.liquidation_price, 35583.95072992701);
    assert_eq!(totx.unrealized_pnl, 0.0);
    assert_eq!(totx.settlement_price, 0.0);
    thread::sleep(time::Duration::from_millis(50));

    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(50));

    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::MARKET,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    thread::sleep(time::Duration::from_millis(50));

    assert_eq!(totx.position_type, PositionType::LONG);
    assert_eq!(totx.order_status, OrderStatus::PENDING);
    assert_eq!(totx.order_type, OrderType::MARKET);
    assert_eq!(totx.entryprice, 40000.0);
    assert_eq!(totx.execution_price, 39000.01);
    assert_eq!(totx.positionsize, 600000.0);
    assert_eq!(totx.leverage, 10.0);
    assert_eq!(totx.initial_margin, 1.5);
    assert_eq!(totx.available_margin, 1.5);
    assert_eq!(totx.bankruptcy_price, 36363.63636363636);
    assert_eq!(totx.bankruptcy_value, 16.5);
    assert_eq!(totx.maintenance_margin, 0.06);
    assert_eq!(totx.liquidation_price, 36496.3503649635);
    assert_eq!(totx.unrealized_pnl, 0.0);
    assert_eq!(totx.settlement_price, 0.0);
}
// need to run these all test in single thread by cmd "cargo test -- --test-threads 1"
// test senario for insert new order in database
// test 1 : Limit order PositionType Long entry price < Current Price
// test 2 : Limit order PositionType Long entry price > Current Price
// test 3 : Limit order PositionType Short entry price < Current Price
// test 4 : Limit order PositionType Short entry price > Current Price
// test 5 : Market Order PositionType Long
// test 6 : Market Order PositionType Short

// #[should_panic]
fn test_traderorder_newtraderorderinsert_test1() {
    // test 1
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(51));
    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(51));
    println!("{:#?}", redis_db::get(&totx.uuid.to_string()));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_eq!(totx, totx_inserted);
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_ne!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1
    );
}

fn test_traderorder_newtraderorderinsert_test2() {
    // test 2
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "38500.0");
    thread::sleep(time::Duration::from_millis(51));

    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();

    thread::sleep(time::Duration::from_millis(51));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_ne!(totx, totx_inserted);
    println!("{:#?}", totx_inserted);
    assert_ne!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_eq!(totx_inserted.order_status, OrderStatus::FILLED);
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1 //means does not exist in TraderOrder_LimitOrder_Pending_FOR_Long key
    );

    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbyLONGLimit"),
        0 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbySHORTLimit"),
        -1 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORLong"
        ),
        0 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORShort"
        ),
        -1 //means does  exist in TraderOrderbyLONGLimit key
    );

    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalLongPositionSize")
    );
    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalPoolPositionSize")
    );
}

fn test_traderorder_newtraderorderinsert_test3() {
    // test 3
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "38500.0");
    thread::sleep(time::Duration::from_millis(51));

    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(51));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_eq!(totx, totx_inserted);
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_ne!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1
    );
}

fn test_traderorder_newtraderorderinsert_test4() {
    // test 4
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "39500.0");
    thread::sleep(time::Duration::from_millis(51));

    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(51));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_ne!(totx, totx_inserted);
    assert_ne!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_eq!(totx_inserted.order_status, OrderStatus::FILLED);
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1 //means does not exist in TraderOrder_LimitOrder_Pending_FOR_Long key
    );

    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbySHORTLimit"),
        0
    );
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbyLONGLimit"),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORShort"
        ),
        0
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORLong"
        ),
        -1
    );

    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalShortPositionSize")
    );
    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalPoolPositionSize")
    );
}

fn test_traderorder_newtraderorderinsert_test5() {
    // test 5
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(51));
    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::MARKET,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(51));
    println!("{:#?}", redis_db::get(&totx.uuid.to_string()));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_ne!(totx, totx_inserted);
    assert_eq!(totx_inserted.entryprice, 40000.0);

    assert_ne!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_eq!(totx_inserted.order_status, OrderStatus::FILLED);
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1 //means does not exist in TraderOrder_LimitOrder_Pending_FOR_Long key
    );

    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbyLONGLimit"),
        -1 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbySHORTLimit"),
        -1 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORLong"
        ),
        0 //means does  exist in TraderOrderbyLONGLimit key
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORShort"
        ),
        -1 //means does  exist in TraderOrderbyLONGLimit key
    );

    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalLongPositionSize")
    );
    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalPoolPositionSize")
    );
}

fn test_traderorder_newtraderorderinsert_test6() {
    // test 6
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(51));
    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::MARKET,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(51));
    println!("{:#?}", redis_db::get(&totx.uuid.to_string()));
    let totx_inserted: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx.uuid.to_string()));
    assert_ne!(totx, totx_inserted);
    assert_eq!(totx_inserted.entryprice, 40000.0);
    assert_ne!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrder"),
        -1
    );
    assert_eq!(totx_inserted.order_status, OrderStatus::FILLED);
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Short"
        ),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrder_LimitOrder_Pending_FOR_Long"
        ),
        -1 //means does not exist in TraderOrder_LimitOrder_Pending_FOR_Long key
    );

    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbySHORTLimit"),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(&totx_inserted.uuid.to_string(), "TraderOrderbyLONGLimit"),
        -1
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORShort"
        ),
        0
    );
    assert_eq!(
        redis_db::zrank_test(
            &totx_inserted.uuid.to_string(),
            "TraderOrderbyLiquidationPriceFORLong"
        ),
        -1
    );

    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalShortPositionSize")
    );
    assert_eq!(
        totx_inserted.positionsize,
        redis_db::get_type_f64("TotalPoolPositionSize")
    );
}

fn test_traderorder_calculate_payment() {
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(51));
    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::MARKET,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        39000.01,
    );
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    println!("{:#?}", totx);
    // panic!();
}

#[test]
fn local_serial_core_test1() {
    local_serial_core("test", test_traderorder_new);
}
#[test]
fn local_serial_core_test2() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test1);
}
#[test]
fn local_serial_core_test3() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test2);
}
#[test]
fn local_serial_core_test4() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test3);
}
#[test]
fn local_serial_core_test5() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test4);
}
#[test]
fn local_serial_core_test6() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test5);
}
#[test]
fn local_serial_core_test7() {
    local_serial_core("test", test_traderorder_newtraderorderinsert_test6);
}
#[test]
fn local_serial_core_test8() {
    local_serial_core("test", test_traderorder_calculate_payment);
}
