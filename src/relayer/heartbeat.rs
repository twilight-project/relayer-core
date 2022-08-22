use crate::config::*;
use crate::db::*;
// use crate::pricefeederlib::price_feeder::receive_btc_price;
use crate::redislib::redis_db;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
use std::collections::HashMap;
use std::{thread, time};
use uuid::Uuid;

pub fn heartbeat() {
    // main thread for scheduler
    thread::Builder::new()
        .name(String::from("heartbeat scheduler"))
        .spawn(move || {
            let mut scheduler = Scheduler::with_tz(chrono::Utc);

            // funding update every 1 hour //comments for local test
            // scheduler.every(600.seconds()).run(move || {
            scheduler.every(1.hour()).run(move || {
                updatefundingrate_localdb(1.0);
            });

            let thread_handle = scheduler.watch_thread(time::Duration::from_millis(100));
            loop {
                thread::sleep(time::Duration::from_millis(100000000));
            }
        })
        .unwrap();

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
        let treadpool_liquidation_order = THREADPOOL_PRICE_CHECK_LIQUIDATION.lock().unwrap();
        treadpool_liquidation_order.execute(move || {
            check_liquidating_orders_on_price_ticker_update_localdb(currentprice.clone());
        });
        let treadpool_settling_order = THREADPOOL_PRICE_CHECK_SETTLE_PENDING.lock().unwrap();
        treadpool_settling_order.execute(move || {
            check_settling_limit_order_on_price_ticker_update_localdb(currentprice.clone());
        });
        // // println!("Price update: not same price");
        // let pool = THREADPOOL_PSQL_SEQ_QUEUE.lock().unwrap();
        // pool.execute(move || {
        //     // insert_current_price_psql(currentprice.clone(), current_time);
        // });
        drop(treadpool_pending_order);
        drop(treadpool_liquidation_order);
        drop(treadpool_settling_order);
        // drop(pool);
    }
}

pub fn check_pending_limit_order_on_price_ticker_update_localdb(current_price: f64) {
    let limit_lock = LIMITSTATUS.lock().unwrap();
    let mut get_open_order_short_list = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();

    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        println!(
            "long:{:#?},\n Short:{:#?}",
            orderid_list_long, orderid_list_short
        );

        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_micros()
                            .to_string(),
                    ),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }
        relayer_event_handler(RelayerCommand::PriceTickerOrderFill(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(limit_lock);
}

pub fn check_liquidating_orders_on_price_ticker_update_localdb(current_price: f64) {
    let liquidation_lock = LIQUIDATIONTICKERSTATUS.lock().unwrap();

    let mut get_open_order_short_list = TRADER_LP_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LP_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        println!(
            "long:{:#?},\n Short:{:#?}",
            orderid_list_long, orderid_list_short
        );

        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_micros()
                            .to_string(),
                    ),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }

        relayer_event_handler(RelayerCommand::PriceTickerLiquidation(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(liquidation_lock);
}
pub fn check_settling_limit_order_on_price_ticker_update_localdb(current_price: f64) {
    let limit_lock = SETTLEMENTLIMITSTATUS.lock().unwrap();
    let mut get_open_order_short_list = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
    let mut orderid_list_short: Vec<Uuid> =
        get_open_order_short_list.search_gt((current_price * 10000.0) as i64);
    drop(get_open_order_short_list);
    let mut get_open_order_long_list = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
    let orderid_list_long: Vec<Uuid> =
        get_open_order_long_list.search_lt((current_price * 10000.0) as i64);
    drop(get_open_order_long_list);
    let orderid_list_short_len = orderid_list_short.len();
    let orderid_list_long_len = orderid_list_long.len();
    let total_order_count = orderid_list_short_len + orderid_list_long_len;
    if total_order_count > 0 {
        println!(
            "long:{:#?},\n Short:{:#?}",
            orderid_list_long, orderid_list_short
        );

        let meta = Meta {
            metadata: {
                let mut hashmap = HashMap::new();
                hashmap.insert(
                    String::from("request_server_time"),
                    Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_micros()
                            .to_string(),
                    ),
                );
                hashmap.insert(
                    String::from("CurrentPrice"),
                    Some(current_price.to_string()),
                );
                hashmap
            },
        };
        if orderid_list_long_len > 0 {
            orderid_list_short.extend(orderid_list_long);
        }

        relayer_event_handler(RelayerCommand::PriceTickerOrderSettle(
            orderid_list_short,
            meta,
            current_price,
        ));
    }
    drop(limit_lock);
}

pub fn updatefundingrate_localdb(psi: f64) {
    let current_time = std::time::SystemTime::now();
    let current_price = get_localdb("CurrentPrice");
    let position_size_log = POSITION_SIZE_LOG.lock().unwrap();
    let totalshort = position_size_log.total_short_positionsize.clone();
    let totallong = position_size_log.total_long_positionsize.clone();
    let allpositionsize = position_size_log.totalpositionsize.clone();
    drop(position_size_log);
    println!(
        "totalshort:{}, totallong:{}, allpositionsize:{}",
        totalshort, totallong, allpositionsize
    );
    //powi is faster then powf
    //psi = 8.0 or  ψ = 8.0
    let mut fundingrate;
    if allpositionsize == 0.0 {
        fundingrate = 0.0;
    } else {
        fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);
    }

    //positive funding if totallong > totalshort else negative funding
    if totallong > totalshort {
    } else {
        fundingrate = fundingrate * (-1.0);
    }
    // comment below code and add kafka msg producer for fl=unding rate
    updatefundingrateindb(fundingrate.clone(), current_price, current_time);
    println!("funding cycle processing...");

    get_and_update_all_orders_on_funding_cycle(current_price, fundingrate.clone());
    println!("fundingrate:{}", fundingrate);
}
