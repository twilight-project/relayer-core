extern crate twilight_relayer_rust;
use crate::twilight_relayer_rust::config::BUSYSTATUS;
use crate::twilight_relayer_rust::redislib::redis_db;
use crate::twilight_relayer_rust::relayer::*;
use std::{thread, time};
#[macro_use]
extern crate lazy_static;

//set funding rate to 0.0064599108719319495
fn setfundingratefortest_positive_funding() {
    redis_db::set("TotalShortPositionSize", "3478240.0");
    redis_db::set("TotalLongPositionSize", "5524940.0");
    redis_db::set("TotalPoolPositionSize", "9003180.0");
    updatefundingrate(1.0);
}

//set funding rate to -0.00024448784504781486
fn setfundingratefortest_negative_funding() {
    redis_db::set("TotalShortPositionSize", "6036240.0");
    redis_db::set("TotalLongPositionSize", "5524940.0");
    redis_db::set("TotalPoolPositionSize", "11561180.0");
    updatefundingrate(1.0);
}
// create new Trader order // all utils function for trader order will also gets tested again
#[test]
fn test_traderorder_new() {
    let mt = BUSYSTATUS.lock().unwrap();
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
    drop(mt);
}
// need to run these all test in single thread by cmd "cargo test -- --test-threads 1"
// to see all the print statement use "cargo test -- --nocapture" or "cargo test -- --nocapture --test-threads 1"
// test senario for insert new order in database
// test 1 : Limit order PositionType Long entry price < Current Price
// test 2 : Limit order PositionType Long entry price > Current Price
// test 3 : Limit order PositionType Short entry price < Current Price
// test 4 : Limit order PositionType Short entry price > Current Price
// test 5 : Market Order PositionType Long
// test 6 : Market Order PositionType Short
#[test]
// #[should_panic]
fn test_traderorder_newtraderorderinsert_test1() {
    let mt = BUSYSTATUS.lock().unwrap();
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
    drop(mt);
}

#[test]
fn test_traderorder_newtraderorderinsert_test2() {
    // test 2
    let mt = BUSYSTATUS.lock().unwrap();
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
    drop(mt);
}

#[test]
fn test_traderorder_newtraderorderinsert_test3() {
    // test 3
    let mt = BUSYSTATUS.lock().unwrap();
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
    drop(mt);
}

#[test]
fn test_traderorder_newtraderorderinsert_test4() {
    let mt = BUSYSTATUS.lock().unwrap(); // test 4
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
    drop(mt);
}

#[test]
fn test_traderorder_newtraderorderinsert_test5() {
    let mt = BUSYSTATUS.lock().unwrap(); // test 5
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
    drop(mt);
}

#[test]
fn test_traderorder_newtraderorderinsert_test6() {
    let mt = BUSYSTATUS.lock().unwrap(); // test 6
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
    drop(mt);
}

//test senario for calculate trader order payment
// case 1 : short order +ve settlement
// case 2 : short order -ve settlement
// case 3 : Long order +ve settlement
// case 4 : Long order -ve settlement
// case 5 : Long order +ve settlement after one funding cycle with +ve funding and fee 0.025
// case 6 : Long order +ve settlement after one funding cycle with -ve funding and fee 0.025
// case 7 : Short order +ve settlement after one funding cycle with +ve funding and fee 0.025
// case 8 : Short order +ve settlement after one funding cycle with -ve funding and fee 0.025

// cargo test -- --nocapture --test test_traderorder_calculate_payment_test1 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test1() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
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
    let mut totx_clone = totx.clone();
    totx_clone = totx_clone.newtraderorderinsert();
    println!("create : {:#?}", totx);
    redis_db::set("CurrentPrice", "38000.0");
    thread::sleep(time::Duration::from_millis(500));
    let totx_settle = totx_clone.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::SHORT);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.2894736842105265);
    assert_eq!(totx_settle.bankruptcy_price, 44444.444444444445);
    assert_eq!(totx_settle.bankruptcy_value, 13.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06);
    assert_eq!(totx_settle.liquidation_price, 44247.78761061947);
    assert_eq!(totx_settle.unrealized_pnl, 0.7894736842105263);
    assert_eq!(totx_settle.settlement_price, 38000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test2 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test2() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
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
    let mut totx_clone = totx.clone();
    totx_clone = totx_clone.newtraderorderinsert();
    println!("create : {:#?}", totx);
    redis_db::set("CurrentPrice", "42000.0");
    thread::sleep(time::Duration::from_millis(500));
    let totx_settle = totx_clone.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::SHORT);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 0.7857142857142857);
    assert_eq!(totx_settle.bankruptcy_price, 44444.444444444445);
    assert_eq!(totx_settle.bankruptcy_value, 13.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06);
    assert_eq!(totx_settle.liquidation_price, 44247.78761061947);
    assert_eq!(totx_settle.unrealized_pnl, -0.7142857142857143);
    assert_eq!(totx_settle.settlement_price, 42000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test3 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test3() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
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
    let mut totx_clone = totx.clone();
    totx_clone = totx_clone.newtraderorderinsert();
    println!("create : {:#?}", totx);
    redis_db::set("CurrentPrice", "42000.0");
    thread::sleep(time::Duration::from_millis(500));
    let totx_settle = totx_clone.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::LONG);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.2142857142857144);
    assert_eq!(totx_settle.bankruptcy_price, 36363.63636363636);
    assert_eq!(totx_settle.bankruptcy_value, 16.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06);
    assert_eq!(totx_settle.liquidation_price, 36496.3503649635);
    assert_eq!(totx_settle.unrealized_pnl, 0.7142857142857143);
    assert_eq!(totx_settle.settlement_price, 42000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test4 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test4() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
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
    let mut totx_clone = totx.clone();
    totx_clone = totx_clone.newtraderorderinsert();
    println!("create : {:#?}", totx);
    redis_db::set("CurrentPrice", "39000.0");
    thread::sleep(time::Duration::from_millis(500));
    let totx_settle = totx_clone.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::LONG);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 1.1153846153846154);
    assert_eq!(totx_settle.bankruptcy_price, 36363.63636363636);
    assert_eq!(totx_settle.bankruptcy_value, 16.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06);
    assert_eq!(totx_settle.liquidation_price, 36496.3503649635);
    assert_eq!(totx_settle.unrealized_pnl, -0.38461538461538464);
    assert_eq!(totx_settle.settlement_price, 39000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test5 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test5() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
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
    println!("create : {:#?}", totx);
    thread::sleep(time::Duration::from_millis(50));
    let mut totx_clone = totx.clone();
    setfundingratefortest_positive_funding();
    totx_clone = totx_clone.newtraderorderinsert();
    redis_db::set("Fee", "0.025");
    redis_db::set("CurrentPrice", "43000.0");
    println!("updated funding rate: {}", redis_db::get("FundingRate"));
    thread::sleep(time::Duration::from_millis(500));
    getandupdateallordersonfundingcycle();
    thread::sleep(time::Duration::from_millis(500));
    let ordertx: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx_clone.uuid.to_string()));
    println!("funding order tx {:#?}", ordertx);
    let totx_settle = ordertx.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::LONG);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.366235045434457);
    assert_eq!(totx_settle.bankruptcy_price, 36363.63636363636);
    assert_eq!(totx_settle.bankruptcy_value, 16.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06519088529386877);
    assert_eq!(totx_settle.liquidation_price, 36507.87762804682);
    assert_eq!(totx_settle.unrealized_pnl, 1.0465116279069768);
    assert_eq!(totx_settle.settlement_price, 43000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test6 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test6() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(500));
    redis_db::set("Fee", "0.025");
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
    let mut totx_clone = totx.clone();
    setfundingratefortest_negative_funding();
    totx_clone = totx_clone.newtraderorderinsert();
    println!("create : {:#?}", totx);
    redis_db::set("CurrentPrice", "43000.0");
    thread::sleep(time::Duration::from_millis(500));
    getandupdateallordersonfundingcycle();
    thread::sleep(time::Duration::from_millis(500));
    let ordertx: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx_clone.uuid.to_string()));
    println!("funding order tx {:#?}", ordertx);
    let totx_settle = ordertx.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::LONG);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.5533345445129623);
    assert_eq!(totx_settle.bankruptcy_price, 36363.63636363636);
    assert_eq!(totx_settle.bankruptcy_value, 16.5);
    assert_eq!(totx_settle.maintenance_margin, 0.0640846595055671);
    assert_eq!(totx_settle.liquidation_price, 36505.42045089109);
    assert_eq!(totx_settle.unrealized_pnl, 1.0465116279069768);
    assert_eq!(totx_settle.settlement_price, 43000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test7 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test7() {
    let mt = BUSYSTATUS.lock().unwrap();
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    println!("{:#?}", *mt);
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    thread::sleep(time::Duration::from_millis(50));
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
    let mut totx_clone = totx.clone();
    println!("create : {:#?}", totx);
    setfundingratefortest_positive_funding();
    totx_clone = totx_clone.newtraderorderinsert();
    redis_db::set("CurrentPrice", "37000.0");
    redis_db::set("Fee", "0.025");
    thread::sleep(time::Duration::from_millis(500));
    getandupdateallordersonfundingcycle();
    thread::sleep(time::Duration::from_millis(500));
    let ordertx: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx_clone.uuid.to_string()));
    println!("funding order tx {:#?}", ordertx);
    let totx_settle = ordertx.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::SHORT);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.925726839089685);
    assert_eq!(totx_settle.bankruptcy_price, 44444.444444444445);
    assert_eq!(totx_settle.bankruptcy_value, 13.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06424708796771082);
    assert_eq!(totx_settle.liquidation_price, 44233.93322967668);
    assert_eq!(totx_settle.unrealized_pnl, 1.2162162162162162);
    assert_eq!(totx_settle.settlement_price, 37000.0);
    drop(mt);
    // panic!();
}

// to run test
// cargo test -- --nocapture --test test_traderorder_calculate_payment_test8 --test-threads 1
#[test]
fn test_traderorder_calculate_payment_test8() {
    let mt = BUSYSTATUS.lock().unwrap();

    // create Lendorder TX
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newtraderorderinsert();

    // drop old table of redisDB
    println!("{}", redis_db::del_test());

    thread::sleep(time::Duration::from_millis(50));

    // set fee and funding rate to 0 and current price to 40000usd
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(51));

    // create test TraderOrder totx
    let totx: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::MARKET,
        10.0, // leverage
        1.5,  // initial margin, in BTC
        1.5,  // available margin, in BTC
        OrderStatus::PENDING,
        39000.01, // entry price, note: for market order entry price will be current price, in USD
        39000.01, // execution price, note: doesn't matter in case of market order
    );
    println!("create : {:#?}", totx);

    let mut totx_clone = totx.clone();

    // set funding rate to -0.00024448784504781486
    setfundingratefortest_negative_funding();

    // insert order to db (new status Pending -> Filled)
    totx_clone = totx_clone.newtraderorderinsert();

    // set current price to 37000usd and fee as 0.025
    redis_db::set("CurrentPrice", "37000.0");
    redis_db::set("Fee", "0.025");
    thread::sleep(time::Duration::from_millis(500));

    // run funding cycle with current price 37000usd, fee 0.025 and funding rate -0.00024448784504781486
    getandupdateallordersonfundingcycle();
    thread::sleep(time::Duration::from_millis(500));

    // get updated order
    let ordertx: TraderOrder =
        TraderOrder::deserialize(&redis_db::get(&totx_clone.uuid.to_string()));
    println!("funding order tx {:#?}", ordertx);

    // settle traderorder
    let totx_settle = ordertx.calculatepayment();
    println!("calculate {:#?}", totx_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(totx_settle.position_type, PositionType::SHORT);
    assert_eq!(totx_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(totx_settle.order_type, OrderType::MARKET);
    assert_eq!(totx_settle.entryprice, 40000.0);
    assert_eq!(totx_settle.execution_price, 39000.01);
    assert_eq!(totx_settle.positionsize, 600000.0);
    assert_eq!(totx_settle.leverage, 10.0);
    assert_eq!(totx_settle.initial_margin, 1.5);
    assert_eq!(totx_settle.available_margin, 2.7082868807011518);
    assert_eq!(totx_settle.bankruptcy_price, 44444.444444444445);
    assert_eq!(totx_settle.bankruptcy_value, 13.5);
    assert_eq!(totx_settle.maintenance_margin, 0.06334199414091855);
    assert_eq!(totx_settle.liquidation_price, 44236.88499922714);
    assert_eq!(totx_settle.unrealized_pnl, 1.2162162162162162);
    assert_eq!(totx_settle.settlement_price, 37000.0);
    drop(mt);
    // panic!();
}
