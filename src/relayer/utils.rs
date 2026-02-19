use crate::config::*;
use crate::relayer::*;

pub fn entryvalue(initial_margin: f64, leverage: f64) -> f64 {
    initial_margin * leverage
}
pub fn positionsize(entryvalue: f64, entryprice: f64) -> f64 {
    entryvalue * (entryprice)
}

// execution_price = settle price
pub fn unrealizedpnl(
    position_type: &PositionType,
    positionsize: f64,
    entryprice: f64,
    settleprice: f64,
) -> f64 {
    if entryprice > 0.0 && settleprice > 0.0 {
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
    } else {
        0.0
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
    if entryprice == 0.0 || positionsize == 0.0 {
        return 0.0;
    }
    entryprice * positionsize / ((positionside as f64) * entryprice * (mm - im) + positionsize)
}
pub fn positionside(position_type: &PositionType) -> i32 {
    match position_type {
        &PositionType::LONG => -1,
        &PositionType::SHORT => 1,
    }
}

pub fn get_localdb(key: &str) -> f64 {
    let local_storage = LOCALDB.lock().unwrap();
    let price = local_storage.get(key).unwrap().clone();
    drop(local_storage);
    price.round()
}

pub fn set_localdb(key: &'static str, value: f64) {
    let mut local_storage = LOCALDB.lock().unwrap();
    local_storage.insert(key.to_string(), value);
    drop(local_storage);
}

pub fn get_lock_error_for_trader_settle(trader_order: TraderOrder) -> i128 {
    let lock_error = ((trader_order.unrealized_pnl.round() as i128)
        * trader_order.entryprice.round() as i128
        * trader_order.settlement_price.round() as i128)
        - (trader_order.positionsize.round() as i128
            * (match trader_order.position_type {
                PositionType::LONG => -1,

                PositionType::SHORT => 1,
            })
            * (trader_order.entryprice.round() as i128
                - trader_order.settlement_price.round() as i128));
    lock_error
}
pub fn get_lock_error_for_lend_create(lend_order: LendOrder) -> i128 {
    let lock_error = (((lend_order.npoolshare / 10000.0).round() * lend_order.tlv0.round()).round()
        as i128)
        - ((lend_order.deposit.round() * lend_order.tps0.round()).round() as i128);
    lock_error
}
pub fn get_lock_error_for_lend_settle(lend_order: LendOrder) -> i128 {
    // let lock_error = (((lend_order.npoolshare / 10000.0).round() * lend_order.tlv0.round()).round()
    //     as i128)
    //     - ((lend_order.deposit.round() * lend_order.tps0.round()).round() as i128);

    let lock_error = (((lend_order.nwithdraw / 10000.0).round() * lend_order.tps2.round()).round()
        as i128)
        - (((lend_order.npoolshare / 10000.0).round() * lend_order.tlv2.round()).round() as i128);
    lock_error
}

pub fn get_relayer_status() -> bool {
    let status = IS_RELAYER_ACTIVE.lock().unwrap();
    let status_result = *status;
    drop(status);
    status_result
}
pub fn set_relayer_status(new_status: bool) {
    let mut status = IS_RELAYER_ACTIVE.lock().unwrap();
    *status = new_status;
    drop(status);
}

use datasize::data_size;
use datasize::DataSize;
pub fn get_size_in_mb<T>(value: &T)
where
    T: DataSize,
{
    crate::log_heartbeat!(warn, "{:#?}MB", data_size(value) / (8 * 1024 * 1024));
}

pub fn get_fee(key: FeeType) -> f64 {
    let local_storage = LOCALDB.lock().unwrap();
    let fee = local_storage.get::<String>(&key.into()).unwrap().clone();
    drop(local_storage);
    fee
}

pub fn set_fee(key: FeeType, value: f64) {
    let mut local_storage = LOCALDB.lock().unwrap();
    local_storage.insert(key.into(), value);
    drop(local_storage);
}

pub fn calculate_fee_on_open_order(fee_persentage: f64, positionsize: f64, price: f64) -> f64 {
    let fee = (fee_persentage / 100.0) * positionsize / price;
    if fee < 1.0 {
        return 1.0;
    }
    fee
}
