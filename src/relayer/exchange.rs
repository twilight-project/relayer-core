use crate::aeronlib::aeronqueue::*;
use crate::aeronlib::types::*;
use crate::relayer::api::*;
use stopwatch::Stopwatch;

pub fn get_pending_trader_order() {
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

pub fn get_pending_lend_order() {
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
