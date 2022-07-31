#![allow(dead_code)]
#![allow(unused_imports)]

// use crate::aeronlibmpsc::types::{AeronMessage, AeronMessageMPSC, StreamId};
// use crate::aeronlibmpsc;
use crate::db::SortedSet;
use crate::relayer::*;
use crate::relayer::{ThreadPool, TraderOrder};
use mpsc::{channel, Receiver, Sender};
use parking_lot::ReentrantMutex;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_redis::RedisConnectionManager;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::SystemTime;
use uuid::Uuid;

lazy_static! {
    pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
}
#[derive(Debug, Clone)]
pub struct OrderDB<T> {
    ordertable: HashMap<Uuid, Arc<RwLock<T>>>,
    sequence: usize,
    nonce: usize,
    cmd: Vec<RpcCommand>,
    event: Vec<Event<T>>,
    aggrigate_log_sequence: usize,
    last_snapshot_id: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event<T> {
    TraderOrder(T, RpcCommand, usize),
    LendOrder(T, RpcCommand, usize),
    RelayerUpdate(T, RelayerCommand, usize),
}

pub trait LocalDB<T> {
    fn new() -> Self;
    fn add(&mut self, order: T, cmd: RpcCommand) -> T;
    fn get_nonce(&mut self) -> usize;
    fn update_nonce(&mut self) -> usize;
    fn get(&mut self, id: Uuid) -> Result<T, std::io::Error>;
    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<T>>, std::io::Error>;
    fn update(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
    fn remove(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
    fn aggrigate_log_sequence(&mut self) -> usize;
    // fn get_from_snapshot(&self) {
    //     println!("{} says {}", self.name(), self.noise());
    // }
}

impl LocalDB<TraderOrder> for OrderDB<TraderOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            cmd: Vec::new(),
            event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }

    fn add(&mut self, mut order: TraderOrder, cmd: RpcCommand) -> TraderOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        order.entry_nonce = self.nonce;
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::TraderOrder(
            order.clone(),
            cmd.clone(),
            self.aggrigate_log_sequence,
        ));
        order.clone()
    }

    fn get_nonce(&mut self) -> usize {
        get_nonce()
    }

    fn update_nonce(&mut self) -> usize {
        update_nonce()
    }

    fn get(&mut self, id: Uuid) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.get(&id) {
            Some(order) => {
                let orx = order.read().unwrap();
                Ok(orx.clone())
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<TraderOrder>>, std::io::Error> {
        match self.ordertable.get_mut(&id) {
            Some(order) => Ok(Arc::clone(order)),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn update(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::TraderOrder(
            order.clone(),
            cmd.clone(),
            self.aggrigate_log_sequence,
        ));
        Ok(order.clone())
    }

    fn remove(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.remove(&order.uuid) {
            Some(_) => {
                self.cmd.push(cmd.clone());
                self.aggrigate_log_sequence += 1;
                order.exit_nonce = get_nonce();
                self.event.push(Event::TraderOrder(
                    order.clone(),
                    cmd.clone(),
                    self.aggrigate_log_sequence,
                ));
                Ok(order)
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ))
            }
        }
    }

    fn aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }
}

pub fn get_nonce() -> usize {
    let nonce = GLOBAL_NONCE.read().unwrap();
    *nonce
}
pub fn update_nonce() -> usize {
    let mut nonce = GLOBAL_NONCE.write().unwrap();
    *nonce += 1;
    *nonce
}
