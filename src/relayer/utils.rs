use crate::config::*;
use crate::relayer::*;

pub fn entryvalue(initial_margin: f64, leverage: f64) -> f64 {
    initial_margin * leverage
}
pub fn positionsize(entryvalue: f64, entryprice: f64) -> f64 {
    entryvalue * (entryprice)
}

// Linear perpetual position size calculation for USDC
// entryvalue is the notional value in USDC
pub fn positionsize_linear(entryvalue: f64, entryprice: f64) -> f64 {
    if entryprice > 0.0 {
        entryvalue / entryprice
    } else {
        0.0
    }
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

// Linear perpetual PnL calculation for USDC
// positionsize is denominated in quote currency (USDC)
pub fn unrealizedpnl_linear(
    position_type: &PositionType,
    positionsize: f64,
    entryprice: f64,
    settleprice: f64,
) -> f64 {
    if entryprice > 0.0 && settleprice > 0.0 {
        match position_type {
            &PositionType::LONG => positionsize * (settleprice - entryprice) / entryprice,
            &PositionType::SHORT => positionsize * (entryprice - settleprice) / entryprice,
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

// Linear perpetual bankruptcy value calculation for USDC
pub fn bankruptcyvalue_linear(positionsize: f64, bankruptcyprice: f64) -> f64 {
    if bankruptcyprice > 0.0 {
        positionsize * bankruptcyprice
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

// Linear perpetual liquidation price calculation for USDC
pub fn liquidationprice_linear(
    entryprice: f64,
    positionsize: f64,
    positionside: i32,
    mm: f64,
    im: f64,
) -> f64 {
    if entryprice == 0.0 || positionsize == 0.0 {
        return 0.0;
    }
    // For linear perpetuals:
    // LONG position (positionside = -1): liquidation_price = entry_price - (margin_difference / position_size)
    // SHORT position (positionside = 1): liquidation_price = entry_price + (margin_difference / position_size)
    entryprice + (positionside as f64) * (im - mm) / positionsize
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
    println!("{:#?}MB", data_size(value) / (8 * 1024 * 1024));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_perpetual_order() {
        // Test parameters
        let initial_margin = 1000.0; // 1000 USDC margin
        let leverage = 10.0; // 10x leverage
        let entry_price = 98000.0; // 98,000 USDC per BTC
        let position_type = PositionType::LONG;

        // Calculate entry value (notional value)
        let entry_value = entryvalue(initial_margin, leverage);
        assert_eq!(entry_value, 10000.0); // Should be 10,000 USDC

        // Calculate position size for linear perpetual
        let position_size = positionsize_linear(entry_value, entry_price);
        let expected_position_size = 10000.0 / 98000.0; // ~0.102 BTC
        assert!((position_size - expected_position_size).abs() < 0.0001);
        println!("Position size: {} BTC", position_size);

        // Calculate bankruptcy price
        let bankruptcy_price = bankruptcyprice(&position_type, entry_price, leverage);
        let expected_bankruptcy_price = 98000.0 * 10.0 / (10.0 + 1.0); // ~89,090.91 USDC
        assert!((bankruptcy_price - expected_bankruptcy_price).abs() < 0.01);
        println!("Bankruptcy price: {} USDC", bankruptcy_price);

        // Calculate bankruptcy value for linear
        let bankruptcy_value = bankruptcyvalue_linear(position_size, bankruptcy_price);
        let expected_bankruptcy_value = position_size * bankruptcy_price;
        assert!((bankruptcy_value - expected_bankruptcy_value).abs() < 0.01);
        println!("Bankruptcy value: {} USDC", bankruptcy_value);

        // Test PnL calculation at different prices
        let settle_price_profit = 100000.0; // Price goes up to 100k
        let settle_price_loss = 96000.0; // Price goes down to 96k

        // Test profit scenario
        let pnl_profit = unrealizedpnl_linear(
            &position_type,
            position_size,
            entry_price,
            settle_price_profit,
        );
        let expected_pnl_profit = position_size * (settle_price_profit - entry_price) / entry_price;
        assert!((pnl_profit - expected_pnl_profit).abs() < 0.0001);
        println!("PnL at 100k: {} USDC", pnl_profit);

        // Test loss scenario
        let pnl_loss = unrealizedpnl_linear(
            &position_type,
            position_size,
            entry_price,
            settle_price_loss,
        );
        let expected_pnl_loss = position_size * (settle_price_loss - entry_price) / entry_price;
        assert!((pnl_loss - expected_pnl_loss).abs() < 0.0001);
        println!("PnL at 96k: {} USDC", pnl_loss);

        // Test liquidation price calculation
        let mm = 50.0; // Maintenance margin
        let im = initial_margin; // Initial margin
        let position_side = positionside(&position_type);

        let liquidation_price =
            liquidationprice_linear(entry_price, position_size, position_side, mm, im);
        println!("Liquidation price: {} USDC", liquidation_price);

        // For a LONG position, liquidation price should be below entry price
        assert!(liquidation_price < entry_price);

        println!("\n=== Linear Perpetual Test Results ===");
        println!("Initial Margin: {} USDC", initial_margin);
        println!("Leverage: {}x", leverage);
        println!("Entry Price: {} USDC/BTC", entry_price);
        println!("Entry Value (Notional): {} USDC", entry_value);
        println!("Position Size: {} BTC", position_size);
        println!("Bankruptcy Price: {} USDC", bankruptcy_price);
        println!("Bankruptcy Value: {} USDC", bankruptcy_value);
        println!("Liquidation Price: {} USDC", liquidation_price);
        println!("PnL at 100k: {} USDC", pnl_profit);
        println!("PnL at 96k: {} USDC", pnl_loss);
    }

    #[test]
    fn test_linear_vs_inverse_comparison() {
        let initial_margin = 1000.0;
        let leverage = 10.0;
        let entry_price = 98000.0;
        let settle_price = 100000.0;
        let position_type = PositionType::LONG;

        let entry_value = entryvalue(initial_margin, leverage);

        // Linear calculations
        let position_size_linear = positionsize_linear(entry_value, entry_price);
        let pnl_linear = unrealizedpnl_linear(
            &position_type,
            position_size_linear,
            entry_price,
            settle_price,
        );

        // Inverse calculations
        let position_size_inverse = positionsize(entry_value, entry_price);
        let pnl_inverse = unrealizedpnl(
            &position_type,
            position_size_inverse,
            entry_price,
            settle_price,
        );

        println!("\n=== Linear vs Inverse Comparison ===");
        println!("Entry Value: {} USDC", entry_value);
        println!("Entry Price: {} USDC/BTC", entry_price);
        println!("Settle Price: {} USDC/BTC", settle_price);
        println!();
        println!("Linear Perpetual:");
        println!("  Position Size: {} BTC", position_size_linear);
        println!("  PnL: {} USDC", pnl_linear);
        println!();
        println!("Inverse Perpetual:");
        println!("  Position Size: {} (BTC units)", position_size_inverse);
        println!("  PnL: {} (BTC units)", pnl_inverse);

        // Verify they're different
        assert!((position_size_linear - position_size_inverse).abs() > 0.001);
        assert!((pnl_linear - pnl_inverse).abs() > 0.001);
    }
}
