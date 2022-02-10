use crate::relayer::traderorder::TraderOrder;
use crate::relayer::types::*;
use crate::relayer::utils::{entryvalue, liquidationprice, maintenancemargin, positionside};
// use std::thread;
use crate::redislib::redis_db;
use crate::relayer::utils::*;

////********* operation on each funding cycle **** //////

pub fn updatefundingrate(psi: f64) {
    let totalshort = redis_db::get_type_f64("TotalShortPositionSize");
    let totallong = redis_db::get_type_f64("TotalLongPositionSize");
    let allpositionsize = redis_db::get_type_f64("TotalPoolPositionSize");
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
    updatefundingrateindb(fundingrate);
    println!("fundingrate:{}", fundingrate);
}

pub fn updatefundingrateindb(fundingrate: f64) {
    redis_db::set("FundingRate", &fundingrate.to_string());

    // need to create parameter table in psql and then update funding rate in psql, same for all pool size n all
}

pub fn updatechangesineachordertxonfundingratechange(
    orderid: String,
    fundingratechange: f64,
    current_price: f64,
    fee: f64,
) -> TraderOrder {
    //get order from redis by orderid
    let mut ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
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
    ordertxclone.update_trader_order_table_into_db_on_funding_cycle(orderid, isliquidated);
    return ordertx;
}

pub fn get_and_update_all_orders_on_funding_cycle() {
    let orderid_list = redis_db::zrangeallopenorders();
    let current_price = get_localdb("CurrentPrice");
    let fundingrate = redis_db::get("FundingRate").parse::<f64>().unwrap();
    let fee = redis_db::get_type_f64("Fee");
    for orderid in orderid_list {
        let state =
            updatechangesineachordertxonfundingratechange(orderid, fundingrate, current_price, fee);
        // println!(
        //     "fundingcycle ,order of {} is {:#?}",
        //     state.uuid, state.order_status
        // );
    }
}

////********* operation on each funding cycle end **** //////
