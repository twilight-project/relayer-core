// mod perpetual;
// use crate::perpetual::interface::{LendOrder, OrderStatus, OrderType, PositionType, TraderOrder};
use crate::relayer::*;
use crate::tpf::redislib::redis_db;
pub fn generateorder() {
    //short limit order
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::SHORT,
    //     OrderType::LIMIT,
    //     5.0,
    //     15201.0,
    //     15201.0,
    //     OrderStatus::PENDING,
    //     38000.01,
    //     34440.02,
    // )
    // .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::SHORT,
    //     OrderType::LIMIT,
    //     5.0,
    //     10201.0,
    //     10201.0,
    //     OrderStatus::PENDING,
    //     38500.01,
    //     33440.02,
    // )
    // .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::SHORT,
    //     OrderType::LIMIT,
    //     5.0,
    //     18201.0,
    //     18201.0,
    //     OrderStatus::PENDING,
    //     40000.01,
    //     36440.02,
    // )
    // .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::SHORT,
    //     OrderType::LIMIT,
    //     5.0,
    //     20201.0,
    //     20201.0,
    //     OrderStatus::PENDING,
    //     41000.01,
    //     30440.02,
    // )
    // .newtraderorderinsert();

    //long limit orders
    TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        39000.01,
        44440.02,
    )
    .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::LONG,
    //     OrderType::LIMIT,
    //     5.0,
    //     18201.0,
    //     18201.0,
    //     OrderStatus::PENDING,
    //     39500.01,
    //     46440.02,
    // )
    // .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::LONG,
    //     OrderType::LIMIT,
    //     5.0,
    //     20201.0,
    //     20201.0,
    //     OrderStatus::PENDING,
    //     40000.01,
    //     48440.02,
    // )
    // .newtraderorderinsert();
    // TraderOrder::new(
    //     "account_id",
    //     PositionType::LONG,
    //     OrderType::LIMIT,
    //     5.0,
    //     12201.0,
    //     12201.0,
    //     OrderStatus::PENDING,
    //     41000.01,
    //     50440.02,
    // )
    // .newtraderorderinsert();
}

pub fn generatelendorder() {
    LendOrder::new(
        "Lend account_id",
        10.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        1.01,
    )
    .newtraderorderinsert();
    LendOrder::new(
        "Lend account_id",
        15.0,
        OrderType::LEND,
        OrderStatus::PENDING,
        2.5,
    )
    .newtraderorderinsert();
}

pub fn initprice() {
    redis_db::set("Fee", "0.0");
    redis_db::set("FundingRate", "0.0");
    // redis_db::set("LendNonce", "0");
    redis_db::set("CurrentPrice", "40000.0");
}
