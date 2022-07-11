use crate::relayer::traderorder::*;
use crate::relayer::types::*;
use crate::relayer::utils::get_localdb;
use crate::relayer::*;

//check for negative leverage
//put limit of leverage
// best lender selection for lend trans

// pub struct CreateTraderOrder {
//     pub account_id: String,
//     pub position_type: PositionType,
//     pub order_type: OrderType,
//     pub leverage: f64,
//     pub initial_margin: f64,
//     pub available_margin: f64,
//     pub order_status: OrderStatus,
//     pub entryprice: f64,
//     pub execution_price: f64,
// }
pub fn get_new_trader_order(msg: String) {
    let mut trader_order_msg = CreateTraderOrder::deserialize(msg);
    let current_price = get_localdb("CurrentPrice");
    if trader_order_msg.order_type == OrderType::LIMIT {
        match trader_order_msg.position_type {
            PositionType::LONG => {
                if trader_order_msg.entryprice >= current_price {
                    // may cancel the order
                    trader_order_msg.order_type = OrderType::MARKET;
                } else {
                }
            }
            PositionType::SHORT => {
                if trader_order_msg.entryprice <= current_price {
                    // may cancel the order
                    trader_order_msg.order_type = OrderType::MARKET;
                } else {
                }
            }
        }
    } else if trader_order_msg.order_type == OrderType::MARKET {
    }
    let ordertx = trader_order_msg.fill_order();
    // println!("{:#?}", ordertx);
    // let ordertx_inserted = ordertx.newtraderorderinsert();
    let ordertx_inserted = orderinsert(ordertx);
}

pub fn get_new_lend_order(msg: String) {
    let lend_order_msg = CreateLendOrder::deserialize(msg);
    let ordertx = lend_order_msg.fill_order();
    let ordertx_inserted = ordertx.newlendorderinsert();
}

pub fn execute_trader_order(msg: String) {
    let handle = std::thread::Builder::new()
        .name("thread1".to_string())
        .spawn(move || -> Result<(), std::io::Error> {
            let lend_order_msg = ExecuteTraderOrder::deserialize(msg);
            let execution_price = lend_order_msg.execution_price.clone();

            match lend_order_msg.clone().get_order() {
                Ok(ordertx) => match ordertx.order_status {
                    OrderStatus::FILLED => {
                        let current_price = get_localdb("CurrentPrice");
                        if lend_order_msg.order_type == OrderType::MARKET {
                            let ordertx_caluculated = ordertx.calculatepayment();
                        } else {
                            match ordertx.position_type {
                                PositionType::LONG => {
                                    if execution_price <= current_price {
                                        let ordertx_caluculated = ordertx.calculatepayment();
                                    } else {
                                        let ordertx_caluculated = ordertx
                                            .set_execution_price_for_limit_order(execution_price);
                                    }
                                }
                                PositionType::SHORT => {
                                    if execution_price >= current_price {
                                        let ordertx_caluculated = ordertx.calculatepayment();
                                    } else {
                                        let ordertx_caluculated = ordertx
                                            .set_execution_price_for_limit_order(execution_price);
                                    }
                                }
                            }
                        }
                    }

                    _ => {
                        println!("order not found !!");
                    }
                },
                Err(arg) => println!("order not found !!, {:#?}", arg),
                // Err(arg) => panic!("this is a terrible mistake!"),
            }
            Ok(())
        });
    match handle.unwrap().join() {
        Ok(_) => {}
        Err(arg) => println!("order not found!!"),
    }
    //use ordertx_caluculated for log
}

pub fn execute_lend_order(msg: String) {
    let lend_order_msg = ExecuteLendOrder::deserialize(msg);
    match lend_order_msg.get_order() {
        Ok(ordertx) => {
            // let ordertx_caluculated = ordertx.calculatepayment();
            match ordertx.calculatepayment() {
                Ok(ordertx_caluculated) => println!("executed order : {:#?}", ordertx_caluculated),
                Err(arg) => println!("Error found !!, {:#?}", arg),
            }
        }
        Err(arg) => println!("order not found !!, {:#?}", arg),
    }
}

pub fn cancel_trader_order(msg: String) {
    let lend_order_msg = CancelTraderOrder::deserialize(msg);

    match lend_order_msg.get_order() {
        Ok(ordertx) => {
            let (ordertx_cancelled, status): (TraderOrder, bool) = ordertx.cancelorder();
        }
        Err(arg) => println!("order not found !!, {:#?}", arg),
    }
}
