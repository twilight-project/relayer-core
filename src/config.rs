#![allow(dead_code)]
#![allow(unused_imports)]
// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
use crate::relayer::{ThreadPool, TraderOrder};
use mpsc::{channel, Receiver, Sender};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Condvar, Mutex, RwLock};
use twilight_relayer_sdk::twilight_client_sdk::programcontroller::ContractManager;
use twilight_relayer_sdk::utxo_in_memory;
use twilight_relayer_sdk::utxo_in_memory::db::LocalDBtrait;
use twilight_relayer_sdk::zkvm;

lazy_static! {
    pub static ref LIMITSTATUS:Mutex<i32> = Mutex::new(0);

    pub static ref SETTLEMENTLIMITSTATUS:Mutex<i32> = Mutex::new(0);

    pub static ref LIQUIDATIONTICKERSTATUS:Mutex<i32> = Mutex::new(0);

    pub static ref TRADERPAYMENT:Mutex<i32> = Mutex::new(0);

    // local database hashmap
    pub static ref LOCALDB: Mutex<HashMap<String,f64>> = Mutex::new(HashMap::new());


    pub static ref THREADPOOL_PRICE_CHECK_PENDING_ORDER:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_PENDING_ORDER")));

    pub static ref THREADPOOL_PRICE_CHECK_LIQUIDATION:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_LIQUIDATION")));

    pub static ref THREADPOOL_PRICE_CHECK_SETTLE_PENDING:Mutex<ThreadPool> = Mutex::new(ThreadPool::new(1,String::from("THREADPOOL_PRICE_CHECK_SETTLE_PENDING")));


    pub static ref SNAPSHOT_VERSION: String =
    std::env::var("SNAPSHOT_VERSION").unwrap_or("v0.1.0".to_string());

    pub static ref EVENTLOG_VERSION: String =
    std::env::var("EVENTLOG_VERSION").unwrap_or("v0.1.0".to_string());

    pub static ref RELAYER_SERVER_SOCKETADDR: String = std::env::var("RELAYER_SERVER_SOCKETADDR")
    .unwrap_or("0.0.0.0:3031".to_string());

    pub static ref RELAYER_SERVER_THREAD: usize = std::env::var("RELAYER_SERVER_THREAD")
    .unwrap_or("2".to_string())
    .parse::<usize>()
    .unwrap_or(2);

    pub static ref RPC_CLIENT_REQUEST: String = std::env::var("RPC_CLIENT_REQUEST")
    .unwrap_or("CLIENT-REQUEST".to_string());

    // pub static ref BROKER: String = std::env::var("BROKER").unwrap_or("localhost:9092".to_string());
    pub static ref BROKERS: Vec<String> = {
        dotenv::dotenv().ok();

        std::env::var("BROKER")
            .unwrap_or_else(|_| "localhost:9092".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };

    pub static ref RELAYER_CORE_GROUP_CLIENT_REQUEST: String = std::env::var("RELAYER_CORE_GROUP_CLIENT_REQUEST")
    .unwrap_or("RELAYER_CORE_GROUP_CLIENT_REQUEST".to_string());

    pub static ref CORE_EVENT_LOG: String = std::env::var("CORE_EVENT_LOG")
    .unwrap_or("CoreEventLogTopic".to_string());

    pub static ref RELAYER_STATE_QUEUE: String = std::env::var("RELAYER_STATE_QUEUE")
    .unwrap_or("RelayerStateQueue".to_string());


    pub static ref OUTPUT_STORAGE: Arc<Mutex<utxo_in_memory::db::LocalStorage::<Option<zkvm::zkos_types::Output>>>> =
    Arc::new(Mutex::new(utxo_in_memory::db::LocalStorage::<
        Option<zkvm::zkos_types::Output>,
    >::new(1)));

    pub static ref WALLET_PROGRAM_PATH: String =
    std::env::var("WALLET_PROGRAM_PATH").unwrap_or("./relayerprogram.json".to_string());

    pub static ref RELAYER_SNAPSHOT_FILE_LOCATION: String =
    std::env::var("RELAYER_SNAPSHOT_FILE_LOCATION").unwrap_or("/usr/bin/relayer_snapshot/snapshot-version".to_string());

    // for enabling chain transaction
    pub static ref ENABLE_ZKOS_CHAIN_TRANSACTION: bool = std::env::var("ENABLE_ZKOS_CHAIN_TRANSACTION")
    .unwrap_or("true".to_string())
    .parse::<bool>()
    .unwrap_or(true);

    pub static ref IS_RELAYER_ACTIVE: (Mutex<bool>, Condvar) = (Mutex::new(true), Condvar::new());

    // Stale price detection thresholds (in seconds)
    pub static ref STALE_PRICE_WARN_THRESHOLD_SECS: u64 = std::env::var("STALE_PRICE_WARN_THRESHOLD_SECS")
    .unwrap_or("10".to_string())
    .parse::<u64>()
    .unwrap_or(10);

    pub static ref STALE_PRICE_HALT_THRESHOLD_SECS: u64 = std::env::var("STALE_PRICE_HALT_THRESHOLD_SECS")
    .unwrap_or("30".to_string())
    .parse::<u64>()
    .unwrap_or(30);
}

/// Binance Individual Symbol Mini Ticker Stream Payload Struct
///
/// https://binance-docs.github.io/apidocs/spot/en/#individual-symbol-mini-ticker-stream
///
///  ### BinanceMiniTickerPayload Struct
/// ```rust,no_run
///
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

/// Binance Individual Symbol aggTicker Stream Payload Struct
///
/// https://binance-docs.github.io/apidocs/spot/en/#individual-symbol-mini-ticker-stream
/// wss://stream.binance.com/ws/btcusdt@aggTrade
///  ### BinanceAggTradePayload Struct
/// ```rust,no_run
///
/// use serde_derive::Deserialize;
/// use serde_derive::Serialize;
/// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// #[serde(rename_all = "camelCase")]
/// pub struct BinanceAggTradePayload {
///     #[serde(rename = "e")]
///     pub event_type: String,
///     #[serde(rename = "E")]
///     pub event_time: i64,
///     #[serde(rename = "s")]
///     pub symbol: String,
///     #[serde(rename = "a")]
///     pub agg_trade_id: i64,
///     #[serde(rename = "p")]
///     pub price: String,
///     #[serde(rename = "q")]
///     pub quantity: String,
///     #[serde(rename = "f")]
///     pub first_trade_id: i64,
///     #[serde(rename = "l")]
///     pub last_trade_id: i64,
///     #[serde(rename = "t")]
///     pub trade_time: i64,
///     #[serde(rename = "m")]
///     pub is_market_maker: bool,
///     #[serde(rename = "M")]
///     pub ignore: bool,
/// }

/// ```
///
/// ### Example Payload
/// ```json
/// {
///     "e": "aggTrade",  // Event type
///     "E": 1672515782136,   // Event time
///     "s": "BNBBTC",    // Symbol
///     "a": 12345,       // Aggregate trade ID
///     "p": "0.001",     // Price
///     "q": "100",       // Quantity
///     "f": 100,         // First trade ID
///     "l": 105,         // Last trade ID
///     "T": 1672515782136,   // Trade time
///     "m": true,        // Is the buyer the market maker?
///     "M": true         // Ignore
///   }
/// ```

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceAggTradePayload {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub agg_trade_id: i64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "l")]
    pub last_trade_id: i64,
    #[serde(rename = "T")]
    pub trade_time: i64,
    #[serde(rename = "m")]
    pub is_market_maker: bool,
    #[serde(rename = "M")]
    pub ignore: bool,
}
