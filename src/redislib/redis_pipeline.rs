#![allow(dead_code)]
use self::redis::{from_redis_value, FromRedisValue, RedisResult, Value};
use crate::config::REDIS_POOL_CONNECTION;
use crate::redislib::redis_db;
use crate::relayer::*;
use r2d2_redis::redis;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
    pub static ref REDIS_PIPELINE_THREADPOOL: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(
        2,
        String::from("REDIS_PIPELINE_THREADPOOL")
    ));
}

// use datasize::data_size;
// use datasize::DataSize;
// use std::thread;
// use stopwatch::Stopwatch;

#[derive(Debug, Clone, PartialEq)]
pub struct RedisPinelineQuery {
    pub cmd: String,
    pub arg: Vec<String>,
    pub ignore: bool,
}

pub fn execute_pipeline(
    data: Vec<RedisPinelineQuery>,
) -> Arc<Mutex<mpsc::Receiver<RedisResult<Value>>>> {
    let (sender, receiver) = mpsc::channel();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let mut pipeline = redis::pipe();

    for cmd in data {
        pipeline.cmd(&cmd.cmd);
        for arg in cmd.arg {
            pipeline.arg(arg);
        }
        if cmd.ignore {
            pipeline.ignore();
        }
    }

    let result = pipeline.query(&mut *conn);
    sender.send(result).unwrap();

    let receiver_result = Arc::new(Mutex::new(receiver));
    return receiver_result;
}

pub fn check_pipe() {
    // let (v1, v2): (String, String);
    let mut pipeline = redis::pipe();
    pipeline
        .cmd("mset")
        .arg(format!("wikey-{}", -1))
        .arg(format!("ivalue-{}", -1));
    pipeline
        .arg(format!("wikey-{}", 0))
        .arg(format!("ivalue-{}", 0));
    for i in 0..500000 {
        pipeline
            .arg(format!("wikey-{}", i))
            .arg(format!("ivalue-{}ivalue-ivalue-ivalue-ivalue-ivalue-i-ivalue-ivalue-ivalivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-ivalue-iv-ivalue-ivalue-ivalue-", i));
    }
    pipeline
        .cmd("mset")
        .arg(format!("wikey-{}", -3))
        .arg(format!("ivalue-{}", -3));
    println!("Im here");
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();

    let (v1, v2): (String, String) = pipeline.query(&mut *conn).unwrap();
    println!("{}{}", v1, v2);
}
