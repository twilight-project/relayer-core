use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
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
        // r2d2::Pool::builder().build(manager).unwrap()
    };
    // pub static ref BUSYSTATUS:Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref BUSYSTATUS:Mutex<i32> =Mutex::new(0);

    pub static ref LENDSTATUS:Mutex<i32> =Mutex::new(0);
    pub static ref QUERYSTATUS:Mutex<i32> =Mutex::new(0);


    // https://github.com/palfrey/serial_test/blob/main/serial_test/src/code_lock.rs
    pub static ref LOCK: Arc<RwLock<HashMap<String, ReentrantMutex<()>>>> =
        Arc::new(RwLock::new(HashMap::new()));
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

/// Relayer_Order
///
///
///
/// r#"{
///     "trader_acc": "{\n\t\"Pk\": {\n\t\t\"Gr\": \"8AfCD9BdbGVyegc/dQTL2Sk9NZzAjhqcNggAeIfq1hQ=\",\n\t\t\"Grsk\": \"IotLv6Dgg4KoFEwhRISZmhkCI40iAQvzhSSfcU9gLxg=\"\n\t},\n\t\"Comm\": {\n\t\t\"C\": \"DmRkz19lQN9m5MhdGZPbHl+wKnnErHVomLK3r/4JNhU=\",\n\t\t\"D\": \"gmffSmIKdG8irntHCFPr9WhG32tSqoGQEDWzhizQHlc=\"\n\t}\n}",
///     "prover_sign": "{\n\t\"E1\": \"prur/oGe8HbK7mLINBnR9UUsEpwzbQT/+HLEu8y8ITk=\",\n\t\"F1\": \"elCL9qyNa8d2CAysh3aFqxrkv7iOOxHj1YBbavOYMxw=\",\n\t\"F2\": \"Zopr2OLluodzzdYzu1eBS/ckdVunW4N4YnO30ACnrUQ=\",\n\t\"XBytes\": \"aalj9OFQJmx4IEMM13oXF9WRibbcdfOycdHzqoN1vw4=\",\n\t\"PHashBytes\": \"tGsiHZbxP9Rcd0muthwlSzSyoPMNOjyMJvNV1gLtdQ5x52puZaE1xYbpM1W0UyijqUQ+vaFq+GemEOhP3XMVyw==\",\n\t\"Zv\": \"9DLnRSut+fDssTaWqCIFmMg/HDgEbup1gAxOBzhD0AI=\",\n\t\"Zsk\": \"CIe936sqIG7hlRtZ7BF/9pJGkitB3c/YHp+Qd35pDQQ=\",\n\t\"Zr\": \"YIQEiOQ6TIS3zwX3u6mngJWNIEckodVMA247UUHjzg0=\"\n}",
///     "order_program_bytes": "\"ACAAAABCztG8tRVF0ZRkKyYan/iI4l01FZU6lq2BF55s1iJDagYKACAAAAAk2AbSFQK1/XwvWV24WDdrro+J7vKqarLZdGPKQuYIagYKCwwPAg==\"",
///     "program_proof": "\"ACYeJJVEW/PAdvgI9hcQVxrt9HHlg9xQ5upDM79K7d9SuN06IeHJt+kN7YmFTV6NCN04gKC0jkMPolYxmIrI/XcsCfxDMjjJCnV1t6CpZ1WakqnzrnnCal4uUmnChXFsLjKY1Ys7i7bzTqO1X9cxV7PqaC0pyExdaMyajzSHsUBDUKzZ0FFyBe0myUBvUGtTOUmj6PfT61OIvBpBWprtLBYOIC6D9U854fu4EMgEgt6ANubkZ68W93k08bXOdgYlNGw7x5/6xwExaJo1AQ3Xwvs80QF9Y5SVi9c8ad8ltmIyyrvS3Oe1K1y8Oe/gX9CUYb4bE6ALouBjuLB5mvKKihkEnqmYuuDbRYRdoQI8kBr1dHhZBHvUeSUZaYgjuS6pCrPy+Cnee65w2TW2HWas+HFzEEzMa3hYNQ3eie+tSaoDuDbYUNoW+sgW2fYUA3altia9W4Om80yxqpfyXVgHeQoWyDPTkrxl6AIonDEbYtPl6q01dF5t53FgY/fzMnPVNSADhpRlOKThiAGlqejn5/XHL9NLoi5onniwaCLyPC4cADqMRaAfZHC4AnFF8js90/yGszi0L4r32ZslYlmv31aUEHrgva8e2MX8Lxt1WX225ZNfH+kMgtIiRMqVwcY/DOgRvYi/m+KhQLfxHx8l0aeI7QgiLK4Ya9A0UhCAD6Efwvr3gClhCJQBo4mhQW86sNvilF0EmP+VWdQb5C9ZGR+iPYI7QWdegLgIIMEOWXgDzarIzwFZHTeoxv7X24y1N2Qa66F3HdkpAImZ1cYWRhhhwc+0Fx+WgeasJKPU7WwjWmk977r9gehFnDZCFpeMzkyJFUjttA+KVi4R+kkuvlMEhuG1BRqQrUGLvjqtm/ngVbE5o9BGSP7mnzMTZl6yelzLxGvlNjRJhwEU4/Yjwp9FHb4KzCr0crIJolVVdlNcpggc54YC4ItJ4Rh0Qmtzco0gQw6IyAge2J0qckKP9S3WhA4QFjlptgsJNgAjnorNpNjy+hIuYQp2WGj3zv94AcDrFtW9nK5NOu+57aqxzpIwBNosca27/MXIfvAS3UcJ\"",
///     "relayer_lock": "{\n\t\"R\": \"OuUOaG8oy8KntNYpjyZ94DRdielpFjL0I/XbnkDd5SE=\",\n\t\"X\": \"CXAb1/ehSLSilGljQbecql9G44hf0V17v4u5u9/Y5gY=\",\n\t\"Zp\": \"d5CXyHnuaCcqU6SrifEUNppJD8F+d5XWa8wZaEu1EAk=\"\n}",
///     "position_size": "288408",
///     "position_side": "-1"
/// }"#
///
// #[macro_use]
// extern crate serde_derive;
// extern crate serde;
// extern crate serde_json;
// use serde::{Deserialize, Serialize};
// use serde_json::{Error, Value};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RELAYERJSONARRAY {
    #[serde(rename = "trader_acc")]
    #[serde(with = "serde_with::json::nested")]
    pub trader_acc: TraderAcc,
    #[serde(rename = "prover_sign")]
    #[serde(with = "serde_with::json::nested")]
    pub prover_sign: ProverSign,
    #[serde(rename = "order_program_bytes")]
    pub order_program_bytes: String,
    #[serde(rename = "program_proof")]
    pub program_proof: String,
    #[serde(rename = "relayer_lock")]
    #[serde(with = "serde_with::json::nested")]
    pub relayer_lock: RelayerLock,
    #[serde(rename = "position_size")]
    pub position_size: String,
    #[serde(rename = "position_side")]
    pub position_side: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderAcc {
    #[serde(rename = "Pk")]
    pub pk: Pk,
    #[serde(rename = "Comm")]
    pub comm: Comm,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pk {
    #[serde(rename = "Gr")]
    pub gr: String,
    #[serde(rename = "Grsk")]
    pub grsk: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comm {
    #[serde(rename = "C")]
    pub c: String,
    #[serde(rename = "D")]
    pub d: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProverSign {
    #[serde(rename = "E1")]
    pub e1: String,
    #[serde(rename = "F1")]
    pub f1: String,
    #[serde(rename = "F2")]
    pub f2: String,
    #[serde(rename = "XBytes")]
    pub xbytes: String,
    #[serde(rename = "PHashBytes")]
    pub phash_bytes: String,
    #[serde(rename = "Zv")]
    pub zv: String,
    #[serde(rename = "Zsk")]
    pub zsk: String,
    #[serde(rename = "Zr")]
    pub zr: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayerLock {
    #[serde(rename = "R")]
    pub r: String,
    #[serde(rename = "X")]
    pub x: String,
    #[serde(rename = "Zp")]
    pub zp: String,
}

// fn main() {
// 	let j = r#"{
// 		"trader_acc": "{\n\t\"Pk\": {\n\t\t\"Gr\": \"8AfCD9BdbGVyegc/dQTL2Sk9NZzAjhqcNggAeIfq1hQ=\",\n\t\t\"Grsk\": \"IotLv6Dgg4KoFEwhRISZmhkCI40iAQvzhSSfcU9gLxg=\"\n\t},\n\t\"Comm\": {\n\t\t\"C\": \"DmRkz19lQN9m5MhdGZPbHl+wKnnErHVomLK3r/4JNhU=\",\n\t\t\"D\": \"gmffSmIKdG8irntHCFPr9WhG32tSqoGQEDWzhizQHlc=\"\n\t}\n}",
// 		"prover_sign": "{\n\t\"E1\": \"prur/oGe8HbK7mLINBnR9UUsEpwzbQT/+HLEu8y8ITk=\",\n\t\"F1\": \"elCL9qyNa8d2CAysh3aFqxrkv7iOOxHj1YBbavOYMxw=\",\n\t\"F2\": \"Zopr2OLluodzzdYzu1eBS/ckdVunW4N4YnO30ACnrUQ=\",\n\t\"XBytes\": \"aalj9OFQJmx4IEMM13oXF9WRibbcdfOycdHzqoN1vw4=\",\n\t\"PHashBytes\": \"tGsiHZbxP9Rcd0muthwlSzSyoPMNOjyMJvNV1gLtdQ5x52puZaE1xYbpM1W0UyijqUQ+vaFq+GemEOhP3XMVyw==\",\n\t\"Zv\": \"9DLnRSut+fDssTaWqCIFmMg/HDgEbup1gAxOBzhD0AI=\",\n\t\"Zsk\": \"CIe936sqIG7hlRtZ7BF/9pJGkitB3c/YHp+Qd35pDQQ=\",\n\t\"Zr\": \"YIQEiOQ6TIS3zwX3u6mngJWNIEckodVMA247UUHjzg0=\"\n}",
// 		"order_program_bytes": "\"ACAAAABCztG8tRVF0ZRkKyYan/iI4l01FZU6lq2BF55s1iJDagYKACAAAAAk2AbSFQK1/XwvWV24WDdrro+J7vKqarLZdGPKQuYIagYKCwwPAg==\"",
// 		"program_proof": "\"ACYeJJVEW/PAdvgI9hcQVxrt9HHlg9xQ5upDM79K7d9SuN06IeHJt+kN7YmFTV6NCN04gKC0jkMPolYxmIrI/XcsCfxDMjjJCnV1t6CpZ1WakqnzrnnCal4uUmnChXFsLjKY1Ys7i7bzTqO1X9cxV7PqaC0pyExdaMyajzSHsUBDUKzZ0FFyBe0myUBvUGtTOUmj6PfT61OIvBpBWprtLBYOIC6D9U854fu4EMgEgt6ANubkZ68W93k08bXOdgYlNGw7x5/6xwExaJo1AQ3Xwvs80QF9Y5SVi9c8ad8ltmIyyrvS3Oe1K1y8Oe/gX9CUYb4bE6ALouBjuLB5mvKKihkEnqmYuuDbRYRdoQI8kBr1dHhZBHvUeSUZaYgjuS6pCrPy+Cnee65w2TW2HWas+HFzEEzMa3hYNQ3eie+tSaoDuDbYUNoW+sgW2fYUA3altia9W4Om80yxqpfyXVgHeQoWyDPTkrxl6AIonDEbYtPl6q01dF5t53FgY/fzMnPVNSADhpRlOKThiAGlqejn5/XHL9NLoi5onniwaCLyPC4cADqMRaAfZHC4AnFF8js90/yGszi0L4r32ZslYlmv31aUEHrgva8e2MX8Lxt1WX225ZNfH+kMgtIiRMqVwcY/DOgRvYi/m+KhQLfxHx8l0aeI7QgiLK4Ya9A0UhCAD6Efwvr3gClhCJQBo4mhQW86sNvilF0EmP+VWdQb5C9ZGR+iPYI7QWdegLgIIMEOWXgDzarIzwFZHTeoxv7X24y1N2Qa66F3HdkpAImZ1cYWRhhhwc+0Fx+WgeasJKPU7WwjWmk977r9gehFnDZCFpeMzkyJFUjttA+KVi4R+kkuvlMEhuG1BRqQrUGLvjqtm/ngVbE5o9BGSP7mnzMTZl6yelzLxGvlNjRJhwEU4/Yjwp9FHb4KzCr0crIJolVVdlNcpggc54YC4ItJ4Rh0Qmtzco0gQw6IyAge2J0qckKP9S3WhA4QFjlptgsJNgAjnorNpNjy+hIuYQp2WGj3zv94AcDrFtW9nK5NOu+57aqxzpIwBNosca27/MXIfvAS3UcJ\"",
// 		"relayer_lock": "{\n\t\"R\": \"OuUOaG8oy8KntNYpjyZ94DRdielpFjL0I/XbnkDd5SE=\",\n\t\"X\": \"CXAb1/ehSLSilGljQbecql9G44hf0V17v4u5u9/Y5gY=\",\n\t\"Zp\": \"d5CXyHnuaCcqU6SrifEUNppJD8F+d5XWa8wZaEu1EAk=\"\n}",
// 		"position_size": "288408",
// 		"position_side": "-1"
// 	}"#;

// 	let relayer_order_deserialize: RELAYERJSONARRAY = serde_json::from_str(j).unwrap();
// 	// println!("{:#?}", relayer_order_deserialize);

// 	let relayer_order_serialize = serde_json::to_string_pretty(&relayer_order_deserialize).unwrap();
// 	// println!(
// 	// 	" \n \n \n relayer_order_serialize : {}",
// 	// 	relayer_order_serialize
// 	// );
// 	let relayer_order_deserialize2: Value = serde_json::from_str(&relayer_order_serialize).unwrap();
// 	println!(
// 		" \n \n \n relayer_order_serialize : {:#?}",
// 		relayer_order_deserialize2
// 	);
// }
