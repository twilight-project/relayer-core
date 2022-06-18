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

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candle {
    pub min: f64,
    pub max: f64,
    pub open: f64,
    pub close: f64,
    pub sell_volume: f64,
    pub buy_volume: f64,
    pub timestamp: std::time::SystemTime,
}

use chrono::prelude::{DateTime, Utc};

fn time_till_min(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    let date_time = format!("{}", dt.format("%d-%m-%Y-%H:%M"));
    date_time
}
