// extern crate stopwatch;
#![allow(dead_code)]
#![allow(unused_imports)]
// mod aeronlib;
// mod aeronlibmpsc;
mod config;
mod kafkalib;
mod ordertest;
mod postgresqllib;
mod pricefeederlib;
mod questdb;
mod redislib;
mod relayer;

use config::local_serial_core;
use config::*;
use kafkalib::consumer_kafka::consume_kafka;
use r2d2_redis::redis;
use redislib::{redis_db, redis_db_orderbook};
use relayer::*;
use std::{thread, time};
use stopwatch::Stopwatch;
#[macro_use]
extern crate lazy_static;
use questdb::questdb::send_candledata_in_questdb;
use std::sync::{mpsc, Arc, Mutex};

fn main() {
    // kafkalib::kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
    // println!("time:{}", relayer::check_server_time());
    // relayer::get_fudning_data_from_psql(10);

    init_psql();
    ordertest::initprice();
    ordertest::generatelendorder();
    thread::sleep(time::Duration::from_millis(100));
    start_cronjobs();
    loop {
        thread::sleep(time::Duration::from_millis(100000000));
    }
    // let current_price = 11000.0;
    // let entry_nonce = redis_db::get_nonce_u128();

    // let orderid_list_short = redis_db::zrangegetpendinglimitorderforshort(current_price);
    // let orderid_list_long = redis_db::zrangegetpendinglimitorderforlong(current_price);
    // println!("orderid_list_short {:#?}", orderid_list_short);
    // println!("orderid_list_long {:#?}", orderid_list_long);
    // let orderid_list_short_len = orderid_list_short.len();
    // let orderid_list_long_len = orderid_list_long.len();
    // if orderid_list_short.len() > 0 {
    //     // redis_db::zdel_bulk(
    //     //     "TraderOrder_LimitOrder_Pending_FOR_Short",
    //     //     orderid_list_short.clone(),
    //     // );
    //     println!("deleting bulk order short len-{}", orderid_list_short_len);
    // }
    // if orderid_list_long.len() > 0 {
    //     // redis_db::zdel_bulk(
    //     //     "TraderOrder_LimitOrder_Pending_FOR_Long",
    //     //     orderid_list_long.clone(),
    //     // );
    //     println!("deleting bulk order long len-{}", orderid_list_long_len);
    // }

    // let total_order_count = orderid_list_short_len + orderid_list_long_len;
    // println!("total order count {}", total_order_count);
    // let mut thread_count: usize = (total_order_count) / 10;
    // if thread_count > 10 {
    //     thread_count = 10;
    // } else if thread_count == 0 {
    //     thread_count = 1;
    // }
    // println!("thread_count {}", thread_count);

    // if total_order_count > 0 {
    //     let mut ordertx_short: Vec<TraderOrder> = Vec::new();
    //     let mut ordertx_long: Vec<TraderOrder> = Vec::new();
    //     let mut starting_entry_sequence_short: u128 = 0;
    //     let mut starting_entry_sequence_long: u128 = 0;
    //     if orderid_list_short_len > 0 {
    //         ordertx_short = redis_db::mget_trader_order(orderid_list_short.clone()).unwrap();
    //         let bulk_entry_sequence_short =
    //             redis_db::incr_entry_sequence_bulk_trader_order(orderid_list_short_len);
    //         starting_entry_sequence_short =
    //             bulk_entry_sequence_short - u128::try_from(orderid_list_short_len).unwrap();
    //     }
    //     if orderid_list_long_len > 0 {
    //         ordertx_long = redis_db::mget_trader_order(orderid_list_long.clone()).unwrap();
    //         let bulk_entry_sequence_long =
    //             redis_db::incr_entry_sequence_bulk_trader_order(orderid_list_long_len);
    //         starting_entry_sequence_long =
    //             bulk_entry_sequence_long - u128::try_from(orderid_list_long_len).unwrap();
    //     }

    //     println!("ordertx_short {:#?}", ordertx_short);
    //     println!("ordertx_long {:#?}", ordertx_long);
    //     println!(
    //         "starting_entry_sequence_short {:#?}",
    //         starting_entry_sequence_short
    //     );
    //     println!(
    //         "starting_entry_sequence_long {:#?}",
    //         starting_entry_sequence_long
    //     );

    //     let local_threadpool: ThreadPool =
    //         ThreadPool::new(thread_count, String::from("local_threadpool"));

    //     for ordertx in ordertx_short {
    //         starting_entry_sequence_short += 1;
    //         local_threadpool.execute(move || {
    //             // let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
    //             let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
    //             let entry_sequence = starting_entry_sequence_short;
    //             // update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
    //             println!(
    //                 "ordertx.uuid :{} ,entry_sequence :{}",
    //                 ordertx.uuid, entry_sequence
    //             );
    //         });
    //     }

    //     for ordertx in ordertx_long {
    //         starting_entry_sequence_long += 1;
    //         local_threadpool.execute(move || {
    //             // let ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
    //             let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
    //             let entry_sequence = starting_entry_sequence_long;
    //             // update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
    //             println!(
    //                 "ordertx.uuid :{} ,entry_sequence :{} , entry_nonce :{} current_price {}",
    //                 ordertx.uuid, entry_sequence, entry_nonce, current_price
    //             );
    //         });
    //     }
    // }
}

use std::convert::{From, TryFrom};
