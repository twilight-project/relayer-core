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
    println!("2nd: {}", o.serialize());
}
