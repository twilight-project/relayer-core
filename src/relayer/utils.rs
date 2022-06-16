use crate::relayer::lendorder::LendOrder;
use crate::relayer::traderorder::TraderOrder;
// use crate::relayer::traderorder::TraderOrder;
use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::config::{LENDSTATUS, TRADERPAYMENT};
use crate::redislib::redis_db;
use crate::relayer::types::*;
use std::thread;

pub fn entryvalue(initial_margin: f64, leverage: f64) -> f64 {
    initial_margin * leverage
}
pub fn positionsize(entryvalue: f64, entryprice: f64) -> f64 {
    entryvalue * entryprice
}

// execution_price = settle price
pub fn unrealizedpnl(
    position_type: &PositionType,
    positionsize: f64,
    entryprice: f64,
    settleprice: f64,
) -> f64 {
    match position_type {
        // &PositionType::LONG => positionsize * (1.0 / entryprice - 1.0 / settleprice),
        &PositionType::LONG => {
            (positionsize * (settleprice - entryprice)) / (entryprice * settleprice)
        }
        // &PositionType::SHORT => positionsize * (1.0 / settleprice - 1.0 / entryprice),
        &PositionType::SHORT => {
            (positionsize * (entryprice - settleprice)) / (entryprice * settleprice)
        }
    }
}

pub fn bankruptcyprice(position_type: &PositionType, entryprice: f64, leverage: f64) -> f64 {
    match position_type {
        &PositionType::LONG => entryprice * leverage / (leverage + 1.0),
        &PositionType::SHORT => {
            if leverage > 1.0 {
                entryprice * leverage / (leverage - 1.0)
            } else {
                0.0
            }
        }
    }
}
pub fn bankruptcyvalue(positionsize: f64, bankruptcyprice: f64) -> f64 {
    if bankruptcyprice > 0.0 {
        positionsize / bankruptcyprice
    } else {
        0.0
    }
}
pub fn maintenancemargin(entry_value: f64, bankruptcyvalue: f64, fee: f64, funding: f64) -> f64 {
    (0.4 * entry_value + fee * bankruptcyvalue + funding * bankruptcyvalue) / 100.0
}

pub fn liquidationprice(
    entryprice: f64,
    positionsize: f64,
    positionside: i32,
    mm: f64,
    im: f64,
) -> f64 {
    // if positionside > 0 {
    //     entryprice * positionsize
    //         / ((positionside as f64) * entryprice * (im - mm) + positionsize)
    // } else {
    entryprice * positionsize / ((positionside as f64) * entryprice * (mm - im) + positionsize)
    // }
}
pub fn positionside(position_type: &PositionType) -> i32 {
    match position_type {
        &PositionType::LONG => -1,
        &PositionType::SHORT => 1,
    }
}

/// also add ammount in tlv **  update nonce also

pub fn updatelendaccountontraderordersettlement(payment: f64) -> u128 {
    let payment_lock = TRADERPAYMENT.lock().unwrap();

    let mut is_payment_done = true;
    let mut remaining_payment = payment;
    while is_payment_done {
        let best_lend_account_order_id = redis_db::getbestlender();
        let mut best_lend_account: LendOrder =
            LendOrder::deserialize(&redis_db::get(&best_lend_account_order_id[0]));
        println!("lend order id {}", best_lend_account.uuid);
        if best_lend_account.new_lend_state_amount >= remaining_payment {
            is_payment_done = false;
            let lend_state_amount = best_lend_account.new_lend_state_amount;
            best_lend_account.new_lend_state_amount = redis_db::zdecr_lend_pool_account(
                &best_lend_account.uuid.to_string(),
                remaining_payment,
            );
            //update best lender tx for newlendstate
            redis_db::set(
                &best_lend_account_order_id[0],
                &best_lend_account.serialize(),
            );
            // psql update for lend order
            let query = format!(
                "UPDATE public.newlendorder SET new_lend_state_amount={} WHERE uuid='{}';",
                best_lend_account.new_lend_state_amount, &best_lend_account_order_id[0]
            );
            thread::spawn(move || {
                let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                client.execute(&query, &[]).unwrap();
            });
        } else {
            let lend_state_amount = best_lend_account.new_lend_state_amount;
            best_lend_account.new_lend_state_amount = redis_db::zdecr_lend_pool_account(
                &best_lend_account.uuid.to_string(),
                best_lend_account.new_lend_state_amount,
            );
            //update best lender tx for newlendstate
            redis_db::set(
                &best_lend_account_order_id[0],
                &best_lend_account.serialize(),
            );
            // psql update for lend order
            let query = format!(
                "UPDATE public.newlendorder SET new_lend_state_amount={} WHERE uuid='{}';",
                best_lend_account.new_lend_state_amount, &best_lend_account_order_id[0]
            );
            thread::spawn(move || {
                let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                client.execute(&query, &[]).unwrap();
            });
            remaining_payment = remaining_payment - lend_state_amount;
        }
    }
    redis_db::decrbyfloat_type_f64("tlv", payment);
    println!("lend account  changed by {}sats", payment);
    drop(payment_lock);
    redis_db::incr_lend_nonce_by_one()
}

// need to create new order **done
// need to create recalculate order **done
// need to create settle order initial_margin **done
// impl for order -> new, recaculate, liquidate etc  **done
// create function bankruptcy price, bankruptcy rate, liquidation price  **done
// remove position side from traderorder stuct  **done
// create_ts timestamp DEFAULT CURRENT_TIMESTAMP ,
// update_ts timestamp DEFAULT CURRENT_TIMESTAMP
// need to create parameter table in psql and then update funding rate in psql, same for all pool size n all
// tlv tps mutex check
// limit order execution - updating limit orders on price ticker
// add entry nonce and exit nonce for traderorder as well as lendorder

//////****** Lending fn ***************/
///
/// undate this function to return both poolshare and npoolshare
pub fn normalize_pool_share(tlv: f64, tps: f64, deposit: f64) -> f64 {
    let npoolshare = tps * deposit * 10000.0 / tlv;

    return npoolshare;
}

pub fn poolshare(tlv: f64, tps: f64, deposit: f64) -> (f64, f64) {
    let npoolshare = tps * deposit * 10000.0 / tlv;
    let poolshare = tps * deposit / tlv;

    return (poolshare, npoolshare);
}

/// undate this function to return both nwithdraw and withdraw
pub fn normalize_withdraw(tlv: f64, tps: f64, npoolshare: f64) -> f64 {
    let nwithdraw = tlv * npoolshare / tps;
    return nwithdraw;
}

pub fn initialize_lend_pool(tlv: f64, tps: f64) -> f64 {
    redis_db::set("tlv", &tlv.to_string());
    redis_db::set("tps", &tps.to_string());
    tlv
}

// use stopwatch::Stopwatch;

pub fn getset_new_lend_order_tlv_tps_poolshare(
    deposit: f64,
) -> (f64, f64, f64, f64, f64, f64, u128, u128) {
    let lend_lock = LENDSTATUS.lock().unwrap();

    let ndeposit = deposit * 10000.0;

    // let tlv0 = redis_db::get_type_f64("tlv");
    // let tps0 = redis_db::get_type_f64("tps");

    let rev_data: Vec<f64> = redis_db::mget_f64(vec!["tlv", "tps"]);

    let (tlv0, tps0) = (rev_data[0], rev_data[1]);

    let (poolshare, npoolshare): (f64, f64) = poolshare(tlv0, tps0, ndeposit);
    let tps1 = redis_db::incrbyfloat_type_f64("tps", poolshare);
    let tlv1 = redis_db::incrbyfloat_type_f64("tlv", ndeposit);
    let entry_nonce = redis_db::incr_lend_nonce_by_one();
    let entry_sequence = redis_db::incr_entry_sequence_by_one_lend_order();
    drop(lend_lock);

    return (
        tlv0,
        tps0,
        tlv1,
        tps1,
        poolshare,
        npoolshare,
        entry_nonce,
        entry_sequence,
    );
}

pub fn getset_settle_lend_order_tlv_tps_poolshare(
    npoolshare: f64,
) -> Result<(f64, f64, f64, f64, f64, f64, u128), std::io::Error> {
    let lend_lock = LENDSTATUS.lock().unwrap();
    // let tlv2 = redis_db::get_type_f64("tlv");
    // let tps2 = redis_db::get_type_f64("tps");

    let rev_data: Vec<f64> = redis_db::mget_f64(vec!["tlv", "tps"]);
    let (tlv2, tps2) = (rev_data[0], rev_data[1]);

    let nwithdraw = normalize_withdraw(tlv2, tps2, npoolshare);
    let withdraw = nwithdraw / 10000.0;
    // assert!(tlv2 < withdraw, "insufficient pool fund!");
    if tlv2 + 10000.0 < withdraw {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "insufficient pool fund!",
        ));
    }
    //convert into pipeline
    let tlv3 = redis_db::decrbyfloat_type_f64("tlv", nwithdraw / 10000.0);
    let tps3 = redis_db::decrbyfloat_type_f64("tps", npoolshare / 10000.0);
    let exit_nonce = redis_db::incr_lend_nonce_by_one();
    //
    drop(lend_lock);
    Ok((tlv2, tps2, tlv3, tps3, withdraw, nwithdraw, exit_nonce))
    // return (tlv2, tps2, tlv3, tps3, withdraw, nwithdraw, exit_nonce);
}

pub fn liquidateposition(mut ordertx: TraderOrder, current_price: f64) -> TraderOrder {
    ordertx.exit_nonce =
        updatelendaccountontraderordersettlement(-10000.0 * ordertx.initial_margin);
    // ordertx.available_margin = 0.0;
    ordertx.settlement_price = current_price;
    ordertx.liquidation_price = current_price;
    ordertx
}

use crate::config::LOCALDB;

pub fn get_localdb(key: &str) -> f64 {
    let local_storage = LOCALDB.lock().unwrap();
    let price = local_storage.get(key).unwrap().clone();
    drop(local_storage);
    price
}

pub fn set_localdb(key: &'static str, value: f64) {
    let mut local_storage = LOCALDB.lock().unwrap();
    local_storage.insert(key, value);
    drop(local_storage);
}
use crate::config::LOCALDBSTRING;

pub fn get_localdb_string(key: &str) -> String {
    let local_storage = LOCALDBSTRING.lock().unwrap();
    let data = local_storage.get(key).unwrap().clone();
    drop(local_storage);
    data
}
pub fn set_localdb_string(key: &'static str, value: String) {
    let mut local_storage = LOCALDBSTRING.lock().unwrap();
    local_storage.insert(key, value);
    drop(local_storage);
}
