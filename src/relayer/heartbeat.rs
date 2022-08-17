use crate::config::*;
use crate::db::*;
use crate::pricefeederlib::price_feeder::receive_btc_price;
use crate::redislib::redis_db;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
use std::{thread, time};
use uuid::Uuid;

pub fn heartbeat() {
    // main thread for scheduler
    // thread::Builder::new()
    //     .name(String::from("cronjob scheduler"))
    //     .spawn(move || {
    //         let mut scheduler = Scheduler::with_tz(chrono::Utc);

    //         // funding update every 1 hour //comments for local test
    //         // scheduler.every(600.seconds()).run(move || {
    //         scheduler.every(1.hour()).run(move || {
    //             // updatefundingrate(1.0);
    //         });

    //         let thread_handle = scheduler.watch_thread(time::Duration::from_millis(100));
    //         loop {
    //             thread::sleep(time::Duration::from_millis(100000000));
    //         }
    //     })
    //     .unwrap();

    // can't use scheduler because it allows minimum 1 second time to schedule any job
    thread::Builder::new()
        .name(String::from("price_check_and_update"))
        .spawn(move || loop {
            thread::sleep(time::Duration::from_millis(250));
            thread::spawn(move || {
                price_check_and_update();
            });
        })
        .unwrap();

    // thread::Builder::new()
    //     .name(String::from("json-RPC startserver"))
    //     .spawn(move || {
    //         startserver();
    //     })
    //     .unwrap();

    // thread::Builder::new()
    //     .name(String::from("BTC Binance Websocket Connection"))
    //     .spawn(move || {
    //         thread::sleep(time::Duration::from_millis(1000));
    //         // receive_btc_price();
    //     })
    //     .unwrap();

    // QueueResolver::new(String::from("questdb_queue"));

    println!("Initialization done..................................");
}

pub fn price_check_and_update() {
    let current_time = std::time::SystemTime::now();

    //get_localdb with single mutex unlock
    let local_storage = LOCALDB.lock().unwrap();
    let currentprice = local_storage.get("Latest_Price").unwrap().clone();
    let old_price = local_storage.get("CurrentPrice").unwrap().clone();
    drop(local_storage);

    if currentprice != old_price {
        set_localdb("CurrentPrice", currentprice);
        redis_db::set("CurrentPrice", &currentprice.clone().to_string());
        let treadpool_pending_order = THREADPOOL_PRICE_CHECK_PENDING_ORDER.lock().unwrap();
        treadpool_pending_order.execute(move || {
            check_pending_limit_order_on_price_ticker_update_localdb(currentprice.clone());
        });
        // let treadpool_liquidation_order = THREADPOOL_PRICE_CHECK_LIQUIDATION.lock().unwrap();
        // treadpool_liquidation_order.execute(move || {
        //     // check_liquidating_orders_on_price_ticker_update(currentprice.clone());
        // });
        // let treadpool_settling_order = THREADPOOL_PRICE_CHECK_SETTLE_PENDING.lock().unwrap();
        // treadpool_settling_order.execute(move || {
        //     // check_settling_limit_order_on_price_ticker_update(currentprice.clone());
        // });
        // // println!("Price update: not same price");
        // let pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
        // pool.execute(move || {
        //     // insert_current_price_psql(currentprice.clone(), current_time);
        // });
        drop(treadpool_pending_order);
        // drop(treadpool_liquidation_order);
        // drop(treadpool_settling_order);
        // drop(pool);
    }
}

pub fn check_pending_limit_order_on_price_ticker_update_localdb(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    let mut get_open_order_short_list = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
    let orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();

    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    let mut thread_count: usize = (total_order_count) / 10;
    if total_order_count > 0 {
        println!(
            "long:{:#?},\n Short:{:#?}",
            orderid_list_long, orderid_list_short
        );
    }

    if total_order_count > 0 {
        if thread_count > 10 {
            thread_count = 10;
        } else if thread_count == 0 {
            thread_count = 1;
        }
        // let entry_nonce = redis_db::get_nonce_usize();
        let mut ordertx_short: Vec<TraderOrder> = Vec::new();
        let mut ordertx_long: Vec<TraderOrder> = Vec::new();
        let mut starting_entry_sequence_short: usize = 0;
        let mut starting_entry_sequence_long: usize = 0;
        if orderid_list_short_len > 0 {
            // ordertx_short = redis_db::mget_trader_order(orderid_list_short.clone()).unwrap();
            // let bulk_entry_sequence_short =
            //     redis_db::incr_entry_seque
            nce_bulk_trader_order(orderid_list_short_len);
            // starting_entry_sequence_short =
            //     bulk_entry_sequence_short - usize::try_from(orderid_list_short_len).unwrap();
        }
        if orderid_list_long_len > 0 {
            // ordertx_long = redis_db::mget_trader_order(orderid_list_long.clone()).unwrap();
            // let bulk_entry_sequence_long =
            //     redis_db::incr_entry_sequence_bulk_trader_order(orderid_list_long_len);
            // starting_entry_sequence_long =
            //     bulk_entry_sequence_long - usize::try_from(orderid_list_long_len).unwrap();
        }

        let local_threadpool: ThreadPool =
            ThreadPool::new(thread_count, String::from("limitorder_local_threadpool"));

        for ordertx in ordertx_short {
            starting_entry_sequence_short += 1;
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = starting_entry_sequence_short;
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }

        for ordertx in ordertx_long {
            starting_entry_sequence_long += 1;
            local_threadpool.execute(move || {
                // let state = println!("order of {} is {:#?}", ordertx.uuid, OrderStatus::FILLED);
                let entry_sequence = starting_entry_sequence_long;
                update_limit_pendingorder(ordertx, current_price, entry_nonce, entry_sequence);
            });
        }
    }
    drop(limit_lock);
}
