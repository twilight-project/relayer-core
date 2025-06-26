use crate::config::LOCALDB;
use crate::redislib::*;
use crate::relayer::*;
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
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("CurrentPrice"), String::from("18000.0")],
        ignore: false,
    });
    cmd_array.push(RedisPinelineQuery {
        cmd: String::from("set"),
        arg: vec![String::from("btc:price"), String::from("18000.0")],
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
    // if local_storage.contains_key("CurrentPrice") == false {
    //     local_storage.insert("CurrentPrice", 18000.0);
    // }
    // if local_storage.contains_key("Latest_Price") == false {
    //     let current_price = local_storage.get("CurrentPrice").unwrap().clone();
    //     local_storage.insert("Latest_Price", current_price);
    // }
    if local_storage.contains_key("FundingRate") == false {
        local_storage.insert("FundingRate", 0.0);
    }
    if local_storage.contains_key("Fee") == false {
        local_storage.insert("Fee", 0.0);
    }

    drop(local_storage);
    update_recent_order_from_db();
}
