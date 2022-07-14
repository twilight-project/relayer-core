use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use crate::relayer::utils::{entryvalue, liquidationprice, maintenancemargin, positionside};
// use std::thread;
use crate::config::{POSTGRESQL_POOL_CONNECTION, THREADPOOL_PSQL_SEQ_QUEUE};
use crate::redislib::*;
use crate::relayer::utils::*;
////********* operation on each funding cycle **** //////

pub fn updatefundingrate(psi: f64) {
    // let totalshort = redis_db::get_type_f64("TotalShortPositionSize");
    // let totallong = redis_db::get_type_f64("TotalLongPositionSize");
    // let allpositionsize = redis_db::get_type_f64("TotalPoolPositionSize");
    let current_time = std::time::SystemTime::now();
    let current_price = get_localdb("CurrentPrice");
    let redis_result = redis_db::mget_f64(vec![
        "TotalShortPositionSize",
        "TotalLongPositionSize",
        "TotalPoolPositionSize",
    ])
    .unwrap();
    let (mut totalshort, mut totallong, mut allpositionsize): (f64, f64, f64) = (0.0, 0.0, 0.0);
    if redis_result.len() > 2 {
        totalshort = redis_result[0];
        totallong = redis_result[1];
        allpositionsize = redis_result[2];
    }
    println!(
        "totalshort:{}, totallong:{}, allpositionsize:{}",
        totalshort, totallong, allpositionsize
    );
    //powi is faster then powf
    //psi = 8.0 or  ψ = 8.0
    let mut fundingrate;
    if allpositionsize == 0.0 {
        fundingrate = 0.0;
    } else {
        fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);
    }

    //positive funding if totallong > totalshort else negative funding
    if totallong > totalshort {
    } else {
        fundingrate = fundingrate * (-1.0);
    }
    updatefundingrateindb(fundingrate.clone(), current_price, current_time);

    println!("funding cycle processing...");
    get_and_update_all_orders_on_funding_cycle(current_price, fundingrate.clone());
    println!("fundingrate:{}", fundingrate);
}

pub fn updatefundingrateindb(
    fundingrate: f64,
    currentprice: f64,
    timestamp: std::time::SystemTime,
) {
    redis_db::set("FundingRate", &fundingrate.to_string());
    let pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
    pool.execute(move || {
        insert_funding_rate_psql(fundingrate, currentprice, timestamp);
    });
    drop(pool);
    // need to create parameter table in psql and then update funding rate in psql, same for all pool size n all
}

fn insert_funding_rate_psql(
    funding_rate: f64,
    currentprice: f64,
    timestamp: std::time::SystemTime,
) {
    let query = format!(
        "call api.insert_fundingrate({},{},$1);",
        funding_rate, currentprice
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
    client.execute(&query, &[&timestamp]).unwrap();
}

pub fn updatechangesineachordertxonfundingratechange(
    mut ordertx: TraderOrder,
    fundingratechange: f64,
    current_price: f64,
    fee: f64,
) -> TraderOrder {
    //get order from redis by orderid
    // let mut ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
    // to check liquidated orders
    let mut isliquidated = false;

    //update available margin
    match ordertx.position_type {
        PositionType::LONG => {
            if fundingratechange > 0.0 {
                ordertx.available_margin = ordertx.available_margin
                    - ((fundingratechange * ordertx.positionsize) / (current_price * 100.0))
            } else {
                ordertx.available_margin = ordertx.available_margin
                    - ((fundingratechange * ordertx.positionsize) / (current_price * 100.0))
            }
        }
        PositionType::SHORT => {
            if fundingratechange > 0.0 {
                ordertx.available_margin = ordertx.available_margin
                    + ((fundingratechange * ordertx.positionsize) / (current_price * 100.0))
            } else {
                ordertx.available_margin = ordertx.available_margin
                    + ((fundingratechange * ordertx.positionsize) / (current_price * 100.0))
            }
        }
    }

    if ordertx.available_margin < 0.0 {
        ordertx.available_margin = 0.0;
    }
    // update maintenancemargin
    ordertx.maintenance_margin = maintenancemargin(
        entryvalue(ordertx.initial_margin, ordertx.leverage),
        ordertx.bankruptcy_value,
        fee,
        fundingratechange,
    );

    // check if AM <= MM if true then call liquidate position else update liquidation price
    if ordertx.available_margin <= ordertx.maintenance_margin {
        //call liquidation
        ordertx = liquidateposition(ordertx, current_price);
        ordertx.order_status = OrderStatus::LIQUIDATE;
        isliquidated = true;
    } else {
        ordertx.liquidation_price = liquidationprice(
            ordertx.entryprice,
            ordertx.positionsize,
            positionside(&ordertx.position_type),
            ordertx.maintenance_margin,
            ordertx.initial_margin,
        );
        // idliquidate = false;
    }
    let ordertxclone = ordertx.clone();
    ordertxclone
        .update_trader_order_table_into_db_on_funding_cycle(ordertx.uuid.to_string(), isliquidated);
    return ordertx;
}

pub fn get_and_update_all_orders_on_funding_cycle(current_price: f64, fundingrate: f64) {
    let fee = get_localdb("Fee");

    let (loop_count, length, data_receiver) = redis_batch::getdata_redis_batch(10000);
    println!("total length : {}", length);
    for i in 0..loop_count {
        let ordertx_array = data_receiver.lock().unwrap().recv().unwrap();
        for ordertx in ordertx_array {
            let state = updatechangesineachordertxonfundingratechange(
                ordertx,
                fundingrate,
                current_price,
                fee,
            );
        }
    }
}

////********* operation on each funding cycle end **** //////
