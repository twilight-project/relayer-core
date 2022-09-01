use crate::config::*;
use crate::relayer::*;

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

use datasize::data_size;
use datasize::DataSize;
pub fn get_size_in_mb<T>(value: &T)
where
    T: DataSize,
{
    println!("{:#?}MB", data_size(value) / (8 * 1024 * 1024));
}
