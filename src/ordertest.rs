// mod perpetual;
// use crate::perpetual::interface::{LendOrder, OrderStatus, OrderType, PositionType, TraderOrder};
use crate::config::LOCALDB;
use crate::redislib::*;
use crate::relayer::*;

pub fn generateorder() {
    // // short limit order
    TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::LIMIT,
        5.0,
        1.5201,
        1.5201,
        OrderStatus::PENDING,
        40000.00,
        34440.02,
    )
    .newtraderorderinsert();
    TraderOrder::new(
        "account_id",
        PositionType::SHORT,
        OrderType::LIMIT,
        5.0,
        1.0201,
        1.0201,
        OrderStatus::PENDING,
        42500.01,
        33440.02,
    )
    .newtraderorderinsert();
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
    TraderOrder::new(
        "account_id",
        PositionType::LONG,
        OrderType::LIMIT,
        10.0,
        1.5,
        1.5,
        OrderStatus::PENDING,
        40000.0,
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
        10.0,
    )
    .newlendorderinsert();
}
// use r2d2_redis::redis;

// use self::redis::{from_redis_value, FromRedisValue, RedisResult, Value};
use redis_pipeline::{execute_pipeline, RedisPinelineQuery};

pub fn initprice() {
    // redis_db::set("Fee", "0.0");
    // redis_db::set("FundingRate", "0.0");
    // redis_db::set("LendNonce", "0");
    // redis_db::set("CurrentPrice", "18000.0");
    // redis_db::set("btc:price", "18000.0");
    let mut cmd_array: Vec<RedisPinelineQuery> = Vec::new();
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("Fee"), String::from("0.0")],
        ignore: false,
    });
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("FundingRate"), String::from("0.0")],
        ignore: false,
    });
    // cmd_array.push(RedisPinelineQuery {
    //     cmd: String::from("set"),
    //     arg: vec![String::from("LendNonce"), String::from("0")],
    //     ignore: false,
    // });
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("CurrentPrice"), String::from("18000.0")],
        ignore: false,
    });
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("btc:price"), String::from("17000.0")],
        ignore: false,
    });

    let receiver = execute_pipeline(cmd_array).lock().unwrap().recv().unwrap();
    // let (st, rate): (String, f64) = FromRedisValue::from_redis_value(&receiver.unwrap()).unwrap();
    // println!("{},,{}", st, rate);
    match receiver {
        Ok(_value) => {}

        Err(arg) => {
            println!("redis_pipe_error:{:#?}", arg);
        }
    }

    let mut local_storage = LOCALDB.lock().unwrap();
    local_storage.insert("CurrentPrice", 18000.0);
    local_storage.insert("btc:price", 18000.0);
    local_storage.insert("FundingRate", 0.0);
    local_storage.insert("Fee", 0.0);
    // drop(local_storage);
    initialize_lend_pool(100000.0, 10.0);
    update_recent_order_from_db();
}
