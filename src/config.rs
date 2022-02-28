#![allow(dead_code)]
#![allow(unused_imports)]
use crate::relayer::ThreadPool;
use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
// use std::sync::Mutex;
use mpsc::{channel, Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex, RwLock};
//thread::JoinHandle<()>

// aeron constant, need to include inside env file
pub const DEFAULT_CHANNEL: &str = "aeron:udp?endpoint=localhost:40123";
pub const DEFAULT_PING_CHANNEL: &str = "aeron:udp?endpoint=localhost:40123";
pub const DEFAULT_PONG_CHANNEL: &str = "aeron:udp?endpoint=localhost:40123";
pub const DEFAULT_STREAM_ID: &str = "1001";
pub const DEFAULT_PING_STREAM_ID: &str = "1002";
pub const DEFAULT_PONG_STREAM_ID: &str = "1003";
pub const DEFAULT_NUMBER_OF_WARM_UP_MESSAGES: &str = "100000";
pub const DEFAULT_NUMBER_OF_MESSAGES: &str = "10000000";
pub const DEFAULT_MESSAGE_LENGTH: &str = "32";
pub const DEFAULT_LINGER_TIMEOUT_MS: &str = "0";
pub const DEFAULT_FRAGMENT_COUNT_LIMIT: &str = "10";
pub const DEFAULT_RANDOM_MESSAGE_LENGTH: bool = false;
pub const DEFAULT_PUBLICATION_RATE_PROGRESS: bool = false;

lazy_static! {
 /// Static Globle PostgreSQL Pool connection
 ///
 /// https://stackoverflow.com/questions/63150183/how-do-i-keep-a-global-postgres-connection
 ///
 pub static ref POSTGRESQL_POOL_CONNECTION: r2d2::Pool<PostgresConnectionManager<NoTls>> = {
     dotenv::dotenv().expect("Failed loading dotenv");
     // POSTGRESQL_URL
     let postgresql_url =
         std::env::var("POSTGRESQL_URL").expect("missing environment variable POSTGRESQL_URL");
     let manager = PostgresConnectionManager::new(
         // TODO: PLEASE MAKE SURE NOT TO USE HARD CODED CREDENTIALS!!!
         postgresql_url.parse().unwrap(),
         NoTls,
     );
     r2d2::Pool::new(manager).unwrap()
 };

 /// Static Globle REDIS Pool connection
 ///
 /// https://users.rust-lang.org/t/r2d2-redis-deref-to-wrong-type/12542
 ///
 pub static ref REDIS_POOL_CONNECTION: r2d2::Pool<RedisConnectionManager> = {
     dotenv::dotenv().expect("Failed loading dotenv");
     let redis_host_name =
     std::env::var("REDIS_HOSTNAME").expect("missing environment variable REDIS_HOSTNAME");

     // let config = Default::default();
     let manager = RedisConnectionManager::new(redis_host_name).unwrap();
     r2d2::Pool::new(manager).expect("expect db")
    //  r2d2::Pool::builder().build(manager).unwrap()
 };
 // pub static ref BUSYSTATUS:Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
 pub static ref BUSYSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref LENDSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref QUERYSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref LIMITSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref LIQUIDATIONTICKERSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref LIQUIDATIONORDERSTATUS:Mutex<i32> = Mutex::new(0);
 pub static ref ORDERTEST:Mutex<i32> = Mutex::new(0);

 // aeron publisher static mpsc
 pub static ref AERON_BROADCAST:(Arc<Mutex<Sender<String>>>,Arc<Mutex<Receiver<String>>>) ={
    let (sender, receiver) = channel();
    return (Arc::new(Mutex::new(sender)), Arc::new(Mutex::new(receiver)));
 };

 // aeron subscriber static mpsc
 pub static ref AERON_RECEIVER:(Arc<Mutex<Sender<String>>>,Arc<Mutex<Receiver<String>>>) ={
    let (sender, receiver) = channel();
    return (Arc::new(Mutex::new(sender)), Arc::new(Mutex::new(receiver)));
 };




 // pub static ref LOCALDB: DashMap<String, f64> = DashMap::new();
 // MAP.insert(String::from("key"), String::from("value"));
 // println!("Siddharth : {:?}", *MAP.get("key").unwrap());

 /// let mut map = HASHMAP.lock().unwrap();
 /// map.insert(3, "sample");
 /// let x = map.get(&3).unwrap().clone();
 /// drop(map);
 pub static ref LOCALDB: Mutex<HashMap<&'static str,f64>> = Mutex::new(HashMap::new());

 // https://github.com/palfrey/serial_test/blob/main/serial_test/src/code_lock.rs
 pub static ref LOCK: Arc<RwLock<HashMap<String, ReentrantMutex<()>>>> =
     Arc::new(RwLock::new(HashMap::new()));

 pub static ref THREADPOOL:Arc<Mutex<ThreadPool>> = Arc::new(Mutex::new(ThreadPool::new(1)));
 pub static ref THREADPOOL_ORDERKAFKAQUEUE:Arc<Mutex<ThreadPool>> = Arc::new(Mutex::new(ThreadPool::new(10)));

}

pub fn check_new_key(name: &str) {
    // Check if a new key is needed. Just need a read lock, which can be done in sync with everyone else
    let new_key = {
        let unlock = LOCK.read().unwrap();
        !unlock.deref().contains_key(name)
    };
    if new_key {
        // This is the rare path, which avoids the multi-writer situation mostly
        LOCK.write()
            .unwrap()
            .deref_mut()
            .insert(name.to_string(), ReentrantMutex::new(()));
    }
}
pub fn local_serial_core(name: &str, function: fn()) {
    check_new_key(name);

    let unlock = LOCK.read().unwrap();
    // _guard needs to be named to avoid being instant dropped
    let _guard = unlock.deref()[name].lock();
    function();
}

/// Binance Individual Symbol Mini Ticker Stream Payload Struct
///
/// https://binance-docs.github.io/apidocs/spot/en/#individual-symbol-mini-ticker-stream
///
///  ### BinanceMiniTickerPayload Struct
/// ```rust,no_run
///
/// use r2d2_postgres::postgres::NoTls;
/// use r2d2_postgres::PostgresConnectionManager;
/// use r2d2_redis::RedisConnectionManager;
/// use serde_derive::Deserialize;
/// use serde_derive::Serialize;
/// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// #[serde(rename_all = "camelCase")]
/// pub struct BinanceMiniTickerPayload {
///     pub e: String, // Event type
///     #[serde(rename = "E")]
///     pub e2: i64, // Event time
///     pub s: String, // Symbol
///     pub c: String, // Close price
///     pub o: String, // Open price
///     pub h: String, // High price
///     pub l: String, // Low price
///     pub v: String, // Total traded base asset volume
///     pub q: String, // Total traded quote asset volume
/// }
/// ```
///
/// ### Example Payload
/// ```json
/// {
///     "e": "24hrMiniTicker",  // Event type
///     "E": 123456789,         // Event time
///     "s": "BNBBTC",          // Symbol
///     "c": "0.0025",          // Close price
///     "o": "0.0010",          // Open price
///     "h": "0.0025",          // High price
///     "l": "0.0010",          // Low price
///     "v": "10000",           // Total traded base asset volume
///     "q": "18"               // Total traded quote asset volume
///   }
/// ```
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceMiniTickerPayload {
    pub e: String, // Event type
    #[serde(rename = "E")]
    pub e2: i64, // Event time
    pub s: String, // Symbol
    pub c: String, // Close price
    pub o: String, // Open price
    pub h: String, // High price
    pub l: String, // Low price
    pub v: String, // Total traded base asset volume
    pub q: String, // Total traded quote asset volume
}
