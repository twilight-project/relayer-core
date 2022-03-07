use crate::aeronlib::aeronqueue::*;
use crate::aeronlib::types::*;
use crate::relayer::api::*;
use crate::relayer::TraderOrder;
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

pub fn execute_lend_order() {
    loop {
        let lend_order_msg = ExecuteTraderOrder::deserialize(
            rec_aeron_msg(StreamId::ExecuteTraderOrder).extract_msg(),
        );
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
        let lend_order_msg = ExecuteTraderOrder::deserialize(
            rec_aeron_msg(StreamId::ExecuteTraderOrder).extract_msg(),
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
