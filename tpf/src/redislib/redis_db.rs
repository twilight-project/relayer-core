//! ## redis_db
//!redis_db provide two main redis function `get` and `set` key-value pairs in redis database.
//!
//! ### Examples
//!   
//! Basic usage:
//!
//! ```rust,no_run
//! mod redis_db
//!
//! fn main() {
//! 	redis_db::set(&"name", &"John");
//! 	println!("{}", redis_db::get(&"name")); //output: John
//! }
//! ```

#![allow(dead_code)]
// extern crate redis;
// extern crate stopwatch;
use crate::config::REDIS_POOL_CONNECTION;
use r2d2_redis::redis;
use std::process::Command;

/// Returns the redis Connection
// fn connect() -> redis::Connection {
//     dotenv::dotenv().expect("Failed loading dotenv");

//     // *******  docker run --rm -p 6379:6379 redis  *******///////
//     let redis_host_name =
//         std::env::var("REDIS_HOSTNAME").expect("missing environment variable REDIS_HOSTNAME");

//     redis::Client::open(redis_host_name)
//         .expect("Invalid connection URL")
//         .get_connection()
//         .expect("failed to connect to Redis")
// }

/// use to set key/value in redis
/// return type bool
///```rust,no_run
/// pub fn set(key: &str, value: &str) -> bool {
///     let mut conn = connect();
///     let _: () = redis::cmd("SET")
///         .arg(key)
///         .arg(value)
///         .query(&mut conn)
///         .expect("failed to execute SET for 'foo'");
///     return true;
/// }
/// ```
pub fn set(key: &str, value: &str) -> bool {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .query::<String>(&mut *conn)
        .unwrap();
    // .query(&mut conn)
    // .expect("failed to execute SET for 'foo'");
    return true;
}

/// use to get value in redis
/// return type bool
///```rust,no_run
/// pub fn get(key: &str) -> String {
///     let mut conn = connect();
///     let bar: Option<String> = redis::cmd("GET")
///         .arg(key)
///         .query(&mut conn)
///         .expect("failed to execute GET for given key");
///
///     return match bar {
///         Some(s) => s,
///         None => String::from("key not found"),
///     };
///
///
/// ```
pub fn get(key: &str) -> String {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    return match redis::cmd("GET").arg(key).query::<String>(&mut *conn) {
        Ok(s) => s,
        Err(_) => String::from("key not found"),
    };
}
/// `save_redis_backup` can copy redis backup created in redis container to host system
///
/// ###Example:
///
///```rust,no_run
///save_redis_backup(String::from("src/redislib/."));
/// ```
///Or to make a call from main.rs
///```rust,no_run
///redislib::redis_db::save_redis_backup(String::from("src/redislib/backup/."));
/// ```
pub fn save_redis_backup(filepath: String) {
    let mut cmd_backup = Command::new("docker");
    cmd_backup.arg("cp");
    cmd_backup.arg("redis:/data/dump.rdb");
    // cmd_backup.arg("src/redislib/.");
    cmd_backup.arg(filepath);
    cmd_backup.output().expect("process failed to execute");
}

pub fn zadd(key: &str, value: &str, score: &str) -> bool {
    // let mut conn = connect();
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("ZADD")
        .arg(key)
        .arg(score)
        .arg(value)
        .query::<i32>(&mut *conn)
        .unwrap();
    // .query(&mut conn)
    // .expect("failed to execute SET for 'foo'");
    return true;
}

pub fn del(key: &str) -> bool {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("DEL").arg(key).query::<i32>(&mut *conn).unwrap();
    return true;
}

pub fn zdel(key: &str, member: &str) -> bool {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("ZREM")
        .arg(key)
        .arg(member)
        .query::<i32>(&mut *conn)
        .unwrap();
    return true;
}
//INCRBYFLOAT
pub fn incrbyfloat(key: &str, value: &str) {
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(value)
        .query::<String>(&mut *conn)
        .unwrap();
}
//DECRBYFLOAT
pub fn decrbyfloat(key: &str, value: &str) {
    let negvalue = format!("-{}", value);
    let mut conn = REDIS_POOL_CONNECTION.get().unwrap();
    let _ = redis::cmd("INCRBYFLOAT")
        .arg(key)
        .arg(negvalue)
        .query::<String>(&mut *conn)
        .unwrap();
}
