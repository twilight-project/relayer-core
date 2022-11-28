#![allow(dead_code)]
use crate::config::REDIS_POOL_CONNECTION;
use crate::redislib::redis_db;
use crate::relayer::*;
use r2d2_redis::redis;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
// use datasize::data_size;
// use datasize::DataSize;
// use std::thread;
// use stopwatch::Stopwatch;

pub fn getdata_redis_batch(
    partition_size: usize,
) -> (usize, usize, Arc<Mutex<mpsc::Receiver<Vec<TraderOrder>>>>) {
    let (sender, receiver) = mpsc::channel();

    let length: usize;
    let mut loop_length: usize = 0;
    length = zrangeallopenorders_batch_count();
    let key_threadpool: ThreadPool;
    let data_threadpool: ThreadPool;
    if length > 0 {
        // let length: usize = orderid_list.len();
        // let part_size = 250000;
        let part_size = partition_size;
        loop_length = (length + part_size) / part_size;

        if loop_length > 10 {
            key_threadpool = ThreadPool::new(10, String::from("getdata_redis_batch_key"));
            data_threadpool = ThreadPool::new(10, String::from("getdata_redis_batch_order"));
        } else {
            data_threadpool =
                ThreadPool::new(loop_length, String::from("getdata_redis_batch_order"));
            key_threadpool = ThreadPool::new(loop_length, String::from("getdata_redis_batch_key"));
        }

        for i in 0..loop_length {
            let startlimit = i * part_size;
            let mut endlimit = (i + 1) * part_size - 1;
            if endlimit > length {
                endlimit = length;
            }
            let sender_clone = sender.clone();
            let (sender_orderid, receiver_orderid) = mpsc::channel();
            // let sender_orderid_clone = sender_orderid.clone();

            key_threadpool.execute(move || {
                sender_orderid
                    .send(zrangeallopenorders_batch(startlimit, endlimit))
                    .unwrap();
            });

            data_threadpool.execute(move || {
                let ordertx_array: Vec<TraderOrder> =
                    redis_db::mget_trader_order(receiver_orderid.recv().unwrap()).unwrap();
                // get_size_in_mb(&format!("{:#?}", ordertx_array));
                sender_clone.send(ordertx_array).unwrap();
            });
        }
    }

    let receiver = Arc::new(Mutex::new(receiver));
    return (loop_length, length, receiver);
}

/// get list of all open orderid
pub fn zrangeallopenorders_batch(from_limit: usize, to_limit: usize) -> Vec<String> {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZRANGE")
        .arg("TraderOrder")
        .arg(from_limit)
        .arg(to_limit)
        .query::<Vec<String>>(&mut *conn)
        .unwrap();
}
/// get list of all open orderid
pub fn zrangeallopenorders_batch_count() -> usize {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return redis::cmd("ZCOUNT")
        .arg("TraderOrder")
        .arg("0")
        .arg("+inf")
        .query::<usize>(&mut *conn)
        .unwrap();
}
