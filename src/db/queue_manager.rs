#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time;
use std::time::SystemTime;
use utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueState {
    pub to_liquidate: HashMap<Uuid, f64>,
    pub to_settle: HashMap<Uuid, f64>,
    pub to_fill: HashMap<Uuid, f64>,
    pub funding_update: HashMap<Uuid, (f64, f64)>,
}
impl QueueState {
    pub fn new() -> Self {
        QueueState {
            to_liquidate: HashMap::new(),
            to_settle: HashMap::new(),
            to_fill: HashMap::new(),
            funding_update: HashMap::new(),
        }
    }
    pub fn insert_liquidate(&mut self, order_id: Uuid, price: f64) {
        self.to_liquidate.insert(order_id, price);
    }
    pub fn insert_settle(&mut self, order_id: Uuid, price: f64) {
        self.to_settle.insert(order_id, price);
    }
    pub fn insert_fill(&mut self, order_id: Uuid, price: f64) {
        self.to_fill.insert(order_id, price);
    }
    pub fn insert_update(&mut self, order_id: Uuid, f_rate: f64, price: f64) {
        self.funding_update.insert(order_id, (f_rate, price));
    }
    pub fn remove_liquidate(&mut self, order_id: &Uuid) -> bool {
        match self.to_liquidate.remove(order_id) {
            Some(_) => true,
            None => false,
        }
    }
    pub fn remove_settle(&mut self, order_id: &Uuid) -> bool {
        match self.to_settle.remove(order_id) {
            Some(_) => true,
            None => false,
        }
    }
    pub fn remove_fill(&mut self, order_id: &Uuid) -> bool {
        match self.to_fill.remove(order_id) {
            Some(_) => true,
            None => false,
        }
    }
    pub fn remove_update(&mut self, order_id: &Uuid) -> bool {
        match self.funding_update.remove(order_id) {
            Some(_) => true,
            None => false,
        }
    }
    pub fn bulk_insert_to_liquidate(
        &mut self,
        pending_short_db: &mut SortedSet,
        pending_long_db: &mut SortedSet,
        price: f64,
    ) {
        let short_id_list: Vec<Uuid> = pending_short_db.search_lt((price * 10000.0) as i64);
        let long_id_list: Vec<Uuid> = pending_long_db.search_gt((price * 10000.0) as i64);
        if short_id_list.len() + long_id_list.len() > 0 {
            for order_id in short_id_list {
                self.insert_liquidate(order_id, price);
            }
            for order_id in long_id_list {
                self.insert_liquidate(order_id, price);
            }
        }
    }
    pub fn bulk_insert_to_settle(
        &mut self,
        pending_short_db: &mut SortedSet,
        pending_long_db: &mut SortedSet,
        price: f64,
    ) {
        let short_id_list: Vec<Uuid> = pending_short_db.search_gt((price * 10000.0) as i64);
        let long_id_list: Vec<Uuid> = pending_long_db.search_lt((price * 10000.0) as i64);
        if short_id_list.len() + long_id_list.len() > 0 {
            for order_id in short_id_list {
                self.insert_settle(order_id, price);
            }
            for order_id in long_id_list {
                self.insert_settle(order_id, price);
            }
        }
    }
    pub fn bulk_insert_to_fill(
        &mut self,
        pending_short_db: &mut SortedSet,
        pending_long_db: &mut SortedSet,
        price: f64,
    ) {
        let short_id_list: Vec<Uuid> = pending_short_db.search_lt((price * 10000.0) as i64);
        let long_id_list: Vec<Uuid> = pending_long_db.search_gt((price * 10000.0) as i64);
        if short_id_list.len() + long_id_list.len() > 0 {
            for order_id in short_id_list {
                self.insert_fill(order_id, price);
            }
            for order_id in long_id_list {
                self.insert_fill(order_id, price);
            }
        }
    }
}
