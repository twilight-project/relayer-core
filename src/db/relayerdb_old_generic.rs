#![allow(dead_code)]
#![allow(unused_imports)]

use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use uuid::Uuid;

lazy_static! {
    pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TraderOrderDB {
    ordertable: HashMap<Uuid, TraderOrder>,
    sequence: usize,
    nonce: usize,
    cmd: Vec<RpcCommand>,
    event: Vec<Event<TraderOrder>>,
    aggrigate_log_sequence: usize,
    last_snapshot_id: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Event<TraderOrder> {
    TraderOrder(TraderOrder, RpcCommand, usize),
    LendOrder(TraderOrder, RpcCommand, usize),
    RelayerUpdate(TraderOrder, RelayerCommand, usize),
}

impl TraderOrderDB {
    pub fn new() -> Self {
        TraderOrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            cmd: Vec::new(),
            event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }
    pub fn add(&mut self, mut order: TraderOrder, cmd: RpcCommand) -> TraderOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        order.entry_nonce = self.nonce;
        self.ordertable.insert(order.uuid, order.clone());
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::TraderOrder(
            order.clone(),
            cmd.clone(),
            self.aggrigate_log_sequence,
        ));
        order.clone()
    }

    pub fn get_nonce(&mut self) -> usize {
        get_nonce()
    }

    pub fn update_nonce(&mut self) -> usize {
        update_nonce()
    }

    pub fn get(&mut self, id: Uuid) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.get(&id) {
            Some(order) => {
                let orx = order;
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

    // pub fn get_mut(&mut self, id: Uuid) -> Result<Arc<Mutex<&TraderOrder>>, std::io::Error> {
    //     match self.ordertable.get_mut(&id) {
    //         Some(order) => {
    //             let mut order_mut = Arc::new(Mutex::new(order));

    //             Ok(Arc::clone(&order_mut))
    //         }
    //         None => {
    //             return Err(std::io::Error::new(
    //                 std::io::ErrorKind::Other,
    //                 "Order not found",
    //             ))
    //         }
    //     }
    // }

    pub fn update(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        self.ordertable.insert(order.uuid, order.clone());
        self.cmd.push(cmd.clone());
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::TraderOrder(
            order.clone(),
            cmd.clone(),
            self.aggrigate_log_sequence,
        ));
        Ok(order.clone())
    }

    pub fn remove(
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

    pub fn aggrigate_log_sequence(&mut self) -> usize {
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
