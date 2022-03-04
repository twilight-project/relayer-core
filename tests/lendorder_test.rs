extern crate twilight_relayer_rust;
use crate::twilight_relayer_rust::config::BUSYSTATUS;
use crate::twilight_relayer_rust::redislib::redis_db;
use crate::twilight_relayer_rust::relayer::*;
use std::{thread, time};
// #[macro_use]
extern crate lazy_static;

// to run test
// cargo test -- --nocapture --test test_lendorder_new --test-threads 1
#[test]
fn test_lendorder_new() {
    let mt = BUSYSTATUS.lock().unwrap();
    // drop old table of redisDB
    // println!("{}", redis_db::del_test());
    // create Lendorder TX
    let lotx = LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.0,
    )
    .newlendorderinsert();
    println!("Lend Order: {:#?} ", lotx);

    thread::sleep(time::Duration::from_millis(50));

    // assert_eq!(totx.position_type, PositionType::LONG);
    // assert_eq!(totx.order_status, OrderStatus::PENDING);
    // assert_eq!(totx.order_type, OrderType::LIMIT);
    // assert_eq!(totx.entryprice, 39000.01);
    // assert_eq!(totx.execution_price, 39000.01);
    // assert_eq!(totx.positionsize, 585000.15);
    // assert_eq!(totx.leverage, 10.0);
    // assert_eq!(totx.initial_margin, 1.5);
    // assert_eq!(totx.available_margin, 1.5);
    // assert_eq!(totx.bankruptcy_price, 35454.55454545455);
    // assert_eq!(totx.bankruptcy_value, 16.5);
    // assert_eq!(totx.maintenance_margin, 0.06);
    // assert_eq!(totx.liquidation_price, 35583.95072992701);
    // assert_eq!(totx.unrealized_pnl, 0.0);
    // assert_eq!(totx.settlement_price, 0.0);

    drop(mt);
}

// to run test
// cargo test -- --nocapture --test test_lendorder_calculate_settlement_test1 --test-threads 1
#[test]
fn test_lendorder_calculate_settlement_test1() {
    let mt = BUSYSTATUS.lock().unwrap();
    // drop old table of redisDB
    println!("{}", redis_db::del_test());
    // create Lendorder TX
    let lotx = LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.0,
    )
    .newlendorderinsert();
    println!("Lend Order: {:#?} ", lotx);

    thread::sleep(time::Duration::from_millis(500));

    let lotx_2 = LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        5.01,
    )
    .newlendorderinsert();
    thread::sleep(time::Duration::from_millis(500));

    println!("Lend Order: {:#?} ", lotx_2);
    let lotx_2_clone = lotx_2.clone();
    let lotx_2_settle = lotx_2_clone.calculatepayment();
    println!("Lend Order settle: {:#?} ", lotx_2_settle);
    thread::sleep(time::Duration::from_millis(500));

    assert_eq!(lotx_2_settle.order_status, OrderStatus::SETTLED);
    assert_eq!(lotx_2_settle.order_type, OrderType::LEND);
    assert_eq!(lotx_2_settle.deposit, 5.01);
    assert_eq!(lotx_2_settle.new_lend_state_amount, 50100.0);
    assert_eq!(lotx_2_settle.npoolshare, 50100.0);
    assert_eq!(lotx_2_settle.nwithdraw, 500999999.99999994);
    // assert_eq!(lotx_2_settle.payment, 0.0); lock error porblem ,help me out

    assert_eq!(lotx_2_settle.tlv0, 150000.0);
    assert_eq!(lotx_2_settle.tlv1, 200100.0);
    assert_eq!(lotx_2_settle.tlv2, 200100.0);
    assert_eq!(lotx_2_settle.tlv3, 150000.0);
    assert_eq!(lotx_2_settle.tps0, 15.0);
    assert_eq!(lotx_2_settle.tps1, 20.01);
    assert_eq!(lotx_2_settle.tps2, 20.01);
    assert_eq!(lotx_2_settle.tps3, 15.0);

    // pub order_status: OrderStatus, //lend or settle
    // pub order_type: OrderType,     // LEND
    // pub entry_nonce: u128,         // change it to u256
    // pub exit_nonce: u128,          // change it to u256
    // pub deposit: f64,
    // pub new_lend_state_amount: f64,
    // pub timestamp: u128,
    // pub npoolshare: f64,
    // pub nwithdraw: f64,
    // pub payment: f64,
    // pub tlv0: f64, //total locked value before lend tx
    // pub tps0: f64, // total poolshare before lend tx
    // pub tlv1: f64, // total locked value after lend tx
    // pub tps1: f64, // total poolshre value after lend tx
    // pub tlv2: f64, // total locked value before lend payment/settlement
    // pub tps2: f64, // total poolshare before lend payment/settlement
    // pub tlv3: f64, // total locked value after lend payment/settlement
    // pub tps3: f64, // total poolshare after lend payment/settlement
    // pub entry_sequence: u128,

    drop(mt);
}
