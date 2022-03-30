use crate::aeronlib::aeronqueue::*;
use crate::aeronlib::types::*;
use crate::relayer::api::*;
use crate::relayer::traderorder::*;
use crate::relayer::types::*;
use crate::relayer::utils::get_localdb;
use stopwatch::Stopwatch;

pub fn get_new_trader_order() {
    loop {
        let trader_order_msg = CreateTraderOrder::deserialize(
            rec_aeron_msg(StreamId::CreateTraderOrder).extract_msg(),
        );
        let sw = Stopwatch::start_new();
        let ordertx = trader_order_msg.fill_order();
        let ordertx_inserted = ordertx.newtraderorderinsert();
        let time_ec = sw.elapsed();
        println!("time: {:#?} Order data : {:#?}", time_ec, ordertx_inserted);
    }
}

pub fn get_new_lend_order() {
    loop {
        let lend_order_msg =
            CreateLendOrder::deserialize(rec_aeron_msg(StreamId::CreateLendOrder).extract_msg());
        let sw = Stopwatch::start_new();
        let ordertx = lend_order_msg.fill_order();
        let ordertx_inserted = ordertx.newlendorderinsert();
        let time_ec = sw.elapsed();
        println!("time: {:#?} Order data : {:#?}", time_ec, ordertx_inserted);
    }
}

pub fn execute_trader_order() {
    loop {
        let lend_order_msg = ExecuteTraderOrder::deserialize(
            rec_aeron_msg(StreamId::ExecuteTraderOrder).extract_msg(),
        );
        let execution_price = lend_order_msg.execution_price.clone();
        let ordertx = lend_order_msg.clone().get_order();
        let current_price = get_localdb("CurrentPrice");
        if lend_order_msg.order_type == OrderType::MARKET {
            let ordertx_caluculated = ordertx.calculatepayment();
        } else {
            match ordertx.position_type {
                PositionType::LONG => {
                    if execution_price <= current_price {
                        let ordertx_caluculated = ordertx.calculatepayment();
                    } else {
                        let ordertx_caluculated =
                            ordertx.set_execution_price_for_limit_order(execution_price);
                    }
                }
                PositionType::SHORT => {
                    if execution_price >= current_price {
                        let ordertx_caluculated = ordertx.calculatepayment();
                    } else {
                        let ordertx_caluculated =
                            ordertx.set_execution_price_for_limit_order(execution_price);
                    }
                }
            }
        }
        //use ordertx_caluculated for log
    }
}

pub fn execute_lend_order() {
    loop {
        let lend_order_msg =
            ExecuteLendOrder::deserialize(rec_aeron_msg(StreamId::ExecuteLendOrder).extract_msg());
        let sw = Stopwatch::start_new();
        let ordertx = lend_order_msg.get_order();
        let ordertx_caluculated = ordertx.calculatepayment();
        let time_ec = sw.elapsed();
        println!(
            "time: {:#?} Order data : {:#?}",
            time_ec, ordertx_caluculated
        );
    }
}

pub fn cancel_trader_order() {
    loop {
        let lend_order_msg = CancelTraderOrder::deserialize(
            rec_aeron_msg(StreamId::CancelTraderOrder).extract_msg(),
        );
        let sw = Stopwatch::start_new();
        let ordertx = lend_order_msg.get_order();
        let (ordertx_cancelled, status): (TraderOrder, bool) = ordertx.cancelorder();
        let time_ec = sw.elapsed();
        println!(
            "time: {:#?} Order data : {:#?} Status:{}",
            time_ec, ordertx_cancelled, status
        );
    }
}
