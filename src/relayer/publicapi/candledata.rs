// use super::orderbook::Side;
use super::recentorder::CloseTrade;
// use crate::redislib::redis_db;
// use crate::relayer::TraderOrder;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
// use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::sync::Mutex;
// use std::{thread, time};
// use uuid::Uuid;
use super::checkservertime::ServerTime;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candle {
    pub low: f64,
    pub high: f64,
    pub open: f64,
    pub close: f64,
    pub sell_volume: f64,
    pub buy_volume: f64,
    pub timestamp: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candles {
    pub candles: Vec<Candle>,
}

use chrono::prelude::{DateTime, Utc};

fn time_till_min(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    let date_time = format!("{}", dt.format("%d-%m-%Y-%H:%M"));
    date_time
}
// sample_by = 1m/5m/30m/1hr/4hr/8hr/24hr
pub fn get_candle(sample_by: String, limit: i32, pagination: i32) -> Candles {
    return Candles {
        candles: vec![Candle {
            low: 19138.94,
            high: 19141.47,
            open: 19141.47,
            close: 19138.94,
            sell_volume: 861311.7000000001,
            buy_volume: 661311.7000000001,
            timestamp: ServerTime::now().epoch,
        }],
    };
}
