extern crate twilight_relayer_rust;
use crate::twilight_relayer_rust::redislib::redis_db;
use crate::twilight_relayer_rust::relayer::*;
use std::{thread, time};

fn gettestorder() -> TraderOrder {
    let orderjson_long_limit="{\"uuid\":\"a411fca6-ba86-44c8-9d99-11afe566f0e5\",\"account_id\":\"account_id\",\"position_type\":\"LONG\",\"order_status\":\"PENDING\",\"order_type\":\"LIMIT\",\"entryprice\":39000.01,\"execution_price\":44440.02,\"positionsize\":585000.15,\"leverage\":10.0,\"initial_margin\":1.5,\"available_margin\":1.5,\"timestamp\":1643186233834,\"bankruptcy_price\":35454.55454545455,\"bankruptcy_value\":16.5,\"maintenance_margin\":0.060134451422482665,\"liquidation_price\":35584.24174890031,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":0,\"exit_nonce\":0,\"entry_sequence\":0}";
    let order: TraderOrder = serde_json::from_str(orderjson_long_limit).unwrap();
    order
}

#[test]
fn test_entryvalue() {
    let trader_order = gettestorder();
    // let initial_margin = 1.5;
    let initial_margin = trader_order.initial_margin;
    // let leverage = 10.0;
    let leverage = trader_order.leverage;

    let entry_value = entryvalue(initial_margin, leverage);
    assert_eq!(entry_value, 15.0);
}

#[test]
fn test_positionsize() {
    let trader_order = gettestorder();
    // let initial_margin = 1.5;
    let initial_margin = trader_order.initial_margin;
    // let leverage = 10.0;
    let leverage = trader_order.leverage;
    // let entryprice = 39000.01;
    let entryprice = trader_order.entryprice;
    let entry_value = entryvalue(initial_margin, leverage);
    let position_size = positionsize(entry_value, entryprice);
    assert_eq!(position_size, 585000.15);
}

#[test]
fn test_unrealizedpnl() {
    let trader_order = gettestorder();

    let position_type: PositionType = trader_order.position_type;
    let positionsize: f64 = trader_order.positionsize;
    let entryprice: f64 = trader_order.entryprice;
    let settleprice: f64 = 42365.51;
    let un_pnl = unrealizedpnl(&position_type, positionsize, entryprice, settleprice);
    let precision = 9;
    let unpnl = format!("{:.1$}", un_pnl, precision);
    assert_eq!(unpnl, "1.191594295");
}

#[test]
fn test_bankruptcyprice() {
    let trader_order = gettestorder();

    let position_type: PositionType = trader_order.position_type;
    let entryprice: f64 = trader_order.entryprice;
    let leverage = trader_order.leverage;
    let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
    let precision = 5;
    let value = format!("{:.1$}", bankruptcy_price, precision);
    println!("{}", value);
    assert_eq!(value, "35454.55455");
}

#[test]
fn test_bankruptcyvalue() {
    let trader_order = gettestorder();
    let position_type: PositionType = trader_order.position_type;
    let entryprice: f64 = trader_order.entryprice;
    let leverage = trader_order.leverage;
    let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
    let positionsize: f64 = trader_order.positionsize;

    let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
    let precision = 5;
    let value = format!("{:.1$}", bankruptcy_value, precision);
    println!("{}", value);
    assert_eq!(value, "16.50000");
}

#[test]
fn test_maintenancemargin() {
    let trader_order = gettestorder();

    let initial_margin = trader_order.initial_margin;
    let leverage = trader_order.leverage;
    let entry_value = entryvalue(initial_margin, leverage);

    let position_type: PositionType = trader_order.position_type;
    let entryprice: f64 = trader_order.entryprice;
    let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
    let positionsize: f64 = trader_order.positionsize;

    let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
    let funding_rate = 0.006459911;
    let fee = 0.025;
    let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, funding_rate);
    let precision = 5;
    let value = format!("{:.1$}", maintenance_margin, precision);
    println!("{}", value);
    assert_eq!(value, "0.06519");
}

#[test]
fn test_liquidationprice() {
    let trader_order = gettestorder();
    let entryprice: f64 = trader_order.entryprice;
    let positionsize: f64 = trader_order.positionsize;

    let position_type: PositionType = trader_order.position_type;
    let position_side = positionside(&position_type);

    let initial_margin = trader_order.initial_margin;
    let leverage = trader_order.leverage;
    let entry_value = entryvalue(initial_margin, leverage);
    let funding_rate = 0.006459911;
    let fee = 0.025;
    let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
    let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);

    let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, funding_rate);

    let liquidation_price = liquidationprice(
        entryprice,
        positionsize,
        position_side,
        maintenance_margin,
        initial_margin,
    );

    let precision = 5;
    let value = format!("{:.1$}", liquidation_price, precision);
    println!("{}", value);
    assert_eq!(value, "35595.18981");
}

#[test]
fn test_positionside() {
    let trader_order = gettestorder();
    let position_type: PositionType = trader_order.position_type;
    let position_side = positionside(&position_type);
    assert_eq!(position_side, -1);
}

//ignore this test if no redisdb is runnning
#[test]
#[ignore]
fn test_updatefundingrate() {
    let totalshort = redis_db::get_type_f64("TotalShortPositionSize");
    let totallong = redis_db::get_type_f64("TotalLongPositionSize");
    let allpositionsize = redis_db::get_type_f64("TotalPoolPositionSize");
    let psi = 1.0;
    let fundingrate;
    if allpositionsize == 0.0 {
        fundingrate = 0.0;
    } else {
        fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);
    }

    updatefundingrate(1.0);
    let funding_rate = redis_db::get("FundingRate").parse::<f64>().unwrap();
    println!("{}={}", fundingrate, funding_rate);
    assert_eq!(funding_rate, fundingrate);
}

// Lend order unit test

#[test]
fn test_normalize_pool_share() {}

#[test]
fn test_normalize_withdraw() {}

// create new Trader order // all utils function for trader order will also gets tested again
#[test]
fn test_traderorder_new() {
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
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
}

// test senario for insert new order in database
// test 1 : Limit order PositionType Long entry price < Current Price
// test 2 : Limit order PositionType Long entry price > Current Price
// test 3 : Limit order PositionType Short entry price < Current Price
// test 4 : Limit order PositionType Short entry price > Current Price
// test 5 : Market Order PositionType Long
// test 6 : Market Order PositionType Short
#[test]
// #[should_panic]
fn test_traderorder_newtraderorderinsert() {
    // test 1
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
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
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(50));
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

    // test 2
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "38500.0");
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
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(50));
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

    // test 3

    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "38500.0");
    thread::sleep(time::Duration::from_millis(50));

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
    thread::sleep(time::Duration::from_millis(50));
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

    // test 4
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "39500.0");
    thread::sleep(time::Duration::from_millis(50));

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
    thread::sleep(time::Duration::from_millis(50));
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

    // test 5
    println!("{}", redis_db::del_test());
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
    let totx_clone = totx.clone();
    totx_clone.newtraderorderinsert();
    thread::sleep(time::Duration::from_millis(50));
    println!("{:#?}", redis_db::get(&totx.uuid.to_string()));
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

    // test 6
    println!("{}", redis_db::del_test());
    thread::sleep(time::Duration::from_millis(50));
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    redis_db::set("CurrentPrice", "40000.0");
    thread::sleep(time::Duration::from_millis(50));
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
    thread::sleep(time::Duration::from_millis(50));
    println!("{:#?}", redis_db::get(&totx.uuid.to_string()));
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
