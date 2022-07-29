#![allow(dead_code)]
#![allow(unused_imports)]
// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
use crate::relayer::{ThreadPool, TraderOrder};
use mpsc::{channel, Receiver, Sender};
use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Mutex, RwLock};

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

    pub static ref QUESTDB_POOL_CONNECTION: r2d2::Pool<PostgresConnectionManager<NoTls>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        // POSTGRESQL_URL
        let postgresql_url =
            std::env::var("QUESTDB_URL").expect("missing environment variable POSTGRESQL_URL");
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

    // static mutex;
    pub static ref BUSYSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref LENDSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref QUERYSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref LIMITSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref SETTLEMENTLIMITSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref LIQUIDATIONTICKERSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref LIQUIDATIONORDERSTATUS:Mutex<i32> = Mutex::new(0);
    pub static ref ORDERTEST:Mutex<i32> = Mutex::new(0);
    pub static ref TRADERPAYMENT:Mutex<i32> = Mutex::new(0);

    // local database hashmap
    pub static ref LOCALDB: Mutex<HashMap<&'static str,f64>> = Mutex::new(HashMap::new());

    // local orderbook
    pub static ref LOCALDBSTRING: Mutex<HashMap<&'static str,String>> = Mutex::new(HashMap::new());

    // https://github.com/palfrey/serial_test/blob/main/serial_test/src/code_lock.rs
    pub static ref LOCK: Arc<RwLock<HashMap<String, ReentrantMutex<()>>>> = Arc::new(RwLock::new(HashMap::new()));

    // sync sender threadpool with buffer size = 1
    // using in candle data
    pub static ref THREADPOOL:Arc<Mutex<ThreadPool>> = Arc::new(Mutex::new(ThreadPool::new(2,String::from("THREADPOOL"))));

    // threadpool for public api
    pub static ref PUBLIC_THREADPOOL:Arc<Mutex<ThreadPool>> = Arc::new(Mutex::new(ThreadPool::new(2,String::from("PUBLIC_THREADPOOL"))));

    // kafka threadpool with buffer/threads = 10
    //  pub static ref THREADPOOL_ORDERKAFKAQUEUE:Arc<Mutex<ThreadPool>> = Arc::new(Mutex::new(ThreadPool::new(10)));


    // sync sender threadpool with buffer size = 1 for price and funding rate
    pub static ref THREADPOOL_PSQL_SEQ_QUEUE:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PSQL_SEQ_QUEUE")));
    pub static ref THREADPOOL_PSQL_ORDER_INSERT_QUEUE:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(5,String::from("THREADPOOL_PSQL_ORDER_INSERT_QUEUE")));
    pub static ref THREADPOOL_REDIS_SEQ_QUEUE:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_REDIS_SEQ_QUEUE")));
    pub static ref THREADPOOL_MAX_ORDER_INSERT:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(10,String::from("THREADPOOL_MAX_ORDER_INSERT")));
    pub static ref THREADPOOL_PRICE_CHECK_PENDING_ORDER:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_PENDING_ORDER")));
    pub static ref THREADPOOL_PRICE_CHECK_LIQUIDATION:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_LIQUIDATION")));
    pub static ref THREADPOOL_PRICE_CHECK_SETTLE_PENDING:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_SETTLE_PENDING")));
    // removeorderfromredis
    pub static ref THREADPOOL_REDIS_ORDER_REMOVE:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(10,String::from("THREADPOOL_REDIS_ORDER_REMOVE")));

    pub static ref RELAYER_VERSION: String =
    std::env::var("RelayerVersion").expect("missing environment variable RelayerVersion");
pub static ref SNAPSHOT_VERSION: String =
    std::env::var("SnapshotVersion").expect("missing environment variable SnapshotVersion");
pub static ref RPC_QUEUE_MODE: String =
    std::env::var("RPC_QUEUE_MODE").expect("missing environment variable RPC_QUEUE_MODE");
pub static ref RPC_SERVER_SOCKETADDR: String = std::env::var("RPC_SERVER_SOCKETADDR")
    .expect("missing environment variable RPC_SERVER_SOCKETADDR");
pub static ref RPC_SERVER_THREAD: usize = std::env::var("RPC_SERVER_THREAD")
    .expect("missing environment variable RPC_SERVER_THREAD")
    .parse::<usize>()
    .unwrap();
    pub static ref KAFKA_STATUS: String = std::env::var("KAFKA_STATUS")
    .expect("missing environment variable KAFKA_STATUS");
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
