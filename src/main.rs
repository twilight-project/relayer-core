mod perpetual;
use perpetual::interface::{OrderStatus, OrderType, PositionType, TraderOrder};
extern crate tpf;
fn main() {
    let o: TraderOrder = TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::MARKET,
        5.0,
        15201.0,
        15201.0,
        OrderStatus::PENDING,
        42514.01,
        0.0,
    );
    // .newtraderorderinsert(); //only call this function if docker is running with postgresql

    println!("1st: {:#?}", o);

    // {"uuid":"3537d499-9498-41f4-9a12-0f25f4864195","account_id":"account_id","position_type":"SHORT","position_side":1,"order_status":"PENDING","order_type":"MARKET","entryprice":42514.01,"execution_price":0.0,"positionsize":3231277330.05,"leverage":5.0,"initial_margin":15201.0,"available_margin":15201.0,"timestamp":1642167758764,"bankruptcy_price":53142.512500000004,"bankruptcy_value":60804.0,"maintenance_margin":320.43708,"liquidation_price":52863.919643478876}
    println!("2nd: {}", o.serialize());
}
