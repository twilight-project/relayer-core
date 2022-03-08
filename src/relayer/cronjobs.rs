use crate::aeronlib::types::init_aeron_queue;
use crate::ordertest::generateorder;
use crate::redislib::redis_db;
use crate::relayer::*;
use clokwerk::{Scheduler, TimeUnits};
use std::time::SystemTime;
use std::{thread, time};

// use stopwatch::Stopwatch;

pub fn start_cronjobs() {
    // main thread for scheduler
    thread::spawn(move || {
        let mut scheduler = Scheduler::with_tz(chrono::Utc);
        // scheduler
        //     .every(200000.seconds())
        //     .run(move || generateorder());

        // make backup of redis db in backup/redisdb folder every 5 sec //comments for local test
        scheduler.every(500000.seconds()).run(move || {
            // scheduler.every(5.seconds()).run(move || {
            redis_db::save_redis_backup(format!(
                "aeron:backup/redisdb/dump_{}.rdb",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ))
        });

        // funding update every 1 hour //comments for local test
        scheduler.every(3000.seconds()).run(move || {
            // scheduler.every(1.hour()).run(move || {
            updatefundingrate(1.0);
            get_and_update_all_orders_on_funding_cycle();
        });

        let thread_handle = scheduler.watch_thread(time::Duration::from_millis(100));
        loop {
            thread::sleep(time::Duration::from_millis(100000000));
        }
    });

    // can't use scheduler because it allows minimum 1 second time to schedule any job
    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_millis(250));
        thread::spawn(move || {
            getsetlatestprice();
        });
    });

    thread::spawn(move || {
        startserver();
    });
    thread::spawn(move || {
        get_new_trader_order();
    });
    thread::spawn(move || {
        get_new_lend_order();
    });
    thread::spawn(move || {
        execute_trader_order();
    });
    thread::spawn(move || {
        execute_lend_order();
    });
    thread::spawn(move || {
        cancel_trader_order();
    });

    // initial aeron
    init_aeron_queue();
}
