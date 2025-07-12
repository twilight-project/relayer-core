#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;

// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct OrderDB<T> {
    pub ordertable: HashMap<Uuid, Arc<RwLock<T>>>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}

pub trait LocalDB<T> {
    fn new() -> Self;
    fn add(&mut self, order: T, cmd: RpcCommand) -> T;
    fn get_nonce(&mut self) -> usize;
    fn update_nonce(&mut self) -> usize;
    fn get(&mut self, id: Uuid) -> Result<T, std::io::Error>;
    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<T>>, std::io::Error>;
    fn getall_mut(&mut self) -> Vec<Arc<RwLock<T>>>;
    fn update(&mut self, order: T, cmd: RelayerCommand) -> Result<T, std::io::Error>;
    fn remove(&mut self, order: T, cmd: RpcCommand) -> Result<T, std::io::Error>;
    fn aggrigate_log_sequence(&mut self) -> usize;
    fn set_order_check(&mut self, account_id: String) -> bool;
    fn remove_order_check(&mut self, account_id: String) -> bool;
    fn get_zkos_string(&mut self, order_id: Uuid) -> Option<String>;
    fn set_zkos_string_on_limit_update(
        &mut self,
        order_id: Uuid,
        zkos_msg: String,
    ) -> Option<String>;
    // fn load_data() -> (bool, OrderDB<T>);
    // fn check_backup() -> Self;
    fn liquidate(&mut self, order: T, cmd: RelayerCommand) -> Result<T, std::io::Error>;
}

impl LocalDB<TraderOrder> for OrderDB<TraderOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            // event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            zkos_msg: HashMap::new(),
            hash: HashSet::new(),
        }
    }

    fn add(&mut self, mut order: TraderOrder, cmd: RpcCommand) -> TraderOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        // order.entry_nonce = get_nonce();
        order.entry_nonce = 0;
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.aggrigate_log_sequence += 1;
        self.zkos_msg.insert(order.uuid, cmd.zkos_msg());
        Event::new(
            Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            String::from(format!("add_order-{}", order.uuid)),
            CORE_EVENT_LOG.clone().to_string(),
        );

        order.clone()
    }

    fn update(
        &mut self,
        mut order: TraderOrder,
        cmd: RelayerCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        if self.ordertable.contains_key(&order.uuid) {
            self.ordertable
                .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
            self.aggrigate_log_sequence += 1;
            order.timestamp = systemtime_to_utc();
            Event::new(
                Event::TraderOrderUpdate(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                format!("update_order-{}", order.uuid),
                CORE_EVENT_LOG.clone().to_string(),
            );
            // if order.order_status == OrderStatus::FILLED {
            //     let zkos_msg_hex = match self.zkos_msg.get(&order.uuid) {
            //         Some(hex_string) => Some(hex_string.clone()),
            //         None => None,
            //     };

            //     zkos_order_handler(ZkosTxCommand::CreateTraderOrderLIMITTX(
            //         order.clone(),
            //         zkos_msg_hex,
            //     ));
            // }

            Ok(order.clone())
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Order not found",
            ));
        }
    }

    fn remove(
        &mut self,
        mut order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        match self.ordertable.remove(&order.uuid) {
            Some(_) => {
                self.aggrigate_log_sequence += 1;
                // order.exit_nonce = get_nonce();
                order.timestamp = systemtime_to_utc();
                let _removed_zkos_msg = self.zkos_msg.remove(&order.uuid);
                Event::new(
                    Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                    format!("remove_order-{}", order.uuid),
                    CORE_EVENT_LOG.clone().to_string(),
                );
                let _ = self.remove_order_check(order.account_id.clone());
                Ok(order)
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Order not found",
                ));
            }
        }
    }

    fn liquidate(
        &mut self,
        mut order: TraderOrder,
        cmd: RelayerCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        if self.ordertable.contains_key(&order.uuid) {
            match self.ordertable.remove(&order.uuid) {
                Some(_) => {
                    self.aggrigate_log_sequence += 1;
                    // order.exit_nonce = get_nonce();
                    order.timestamp = systemtime_to_utc();
                    Event::new(
                        Event::TraderOrderLiquidation(
                            order.clone(),
                            cmd.clone(),
                            self.aggrigate_log_sequence,
                        ),
                        format!("remove_order-{}", order.uuid),
                        CORE_EVENT_LOG.clone().to_string(),
                    );
                    Ok(order)
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Order not found",
                    ));
                }
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Order not found",
            ));
        }
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
                    std::io::ErrorKind::NotFound,
                    "Order not found",
                ));
            }
        }
    }

    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<TraderOrder>>, std::io::Error> {
        match self.ordertable.get_mut(&id) {
            Some(order) => Ok(Arc::clone(order)),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Order {} not found", &id),
                ));
            }
        }
    }
    fn getall_mut(&mut self) -> Vec<Arc<RwLock<TraderOrder>>> {
        let mut orderdetails_array: Vec<Arc<RwLock<TraderOrder>>> = Vec::new();

        for (_, order) in self.ordertable.iter_mut() {
            orderdetails_array.push(Arc::clone(order));
        }

        orderdetails_array
    }
    fn aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }

    fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
    fn get_zkos_string(&mut self, order_id: Uuid) -> Option<String> {
        let zkos_msg_hex = match self.zkos_msg.get(&order_id) {
            Some(hex_string) => Some(hex_string.clone()),
            None => None,
        };
        zkos_msg_hex
    }
    fn set_zkos_string_on_limit_update(
        &mut self,
        order_id: Uuid,
        zkos_msg: String,
    ) -> Option<String> {
        self.zkos_msg.insert(order_id, zkos_msg)
    }
}

impl LocalDB<LendOrder> for OrderDB<LendOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            zkos_msg: HashMap::new(),
            hash: HashSet::new(),
        }
    }

    fn add(&mut self, mut order: LendOrder, cmd: RpcCommand) -> LendOrder {
        // let mut lendpool = LEND_POOL_DB.lock().unwrap();
        // lendpool.sequence;
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.aggrigate_log_sequence += 1;
        self.zkos_msg.insert(order.uuid, cmd.zkos_msg());
        Event::new(
            Event::LendOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            format!("add_order-{}", order.uuid),
            CORE_EVENT_LOG.clone().to_string(),
        );

        order.clone()
    }

    fn get_nonce(&mut self) -> usize {
        get_nonce()
    }

    fn update_nonce(&mut self) -> usize {
        update_nonce()
    }

    fn get(&mut self, id: Uuid) -> Result<LendOrder, std::io::Error> {
        match self.ordertable.get(&id) {
            Some(order) => {
                let orx = order.read().unwrap();
                Ok(orx.clone())
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ));
            }
        }
    }

    fn get_mut(&mut self, id: Uuid) -> Result<Arc<RwLock<LendOrder>>, std::io::Error> {
        match self.ordertable.get_mut(&id) {
            Some(order) => Ok(Arc::clone(order)),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ));
            }
        }
    }

    fn getall_mut(&mut self) -> Vec<Arc<RwLock<LendOrder>>> {
        let mut orderdetails_array: Vec<Arc<RwLock<LendOrder>>> = Vec::new();

        for (_, order) in self.ordertable.iter_mut() {
            orderdetails_array.push(Arc::clone(order));
        }

        orderdetails_array
    }

    fn update(
        &mut self,
        order: LendOrder,
        _cmd: RelayerCommand,
    ) -> Result<LendOrder, std::io::Error> {
        Ok(order.clone())
    }

    fn remove(
        &mut self,
        mut order: LendOrder,
        cmd: RpcCommand,
    ) -> Result<LendOrder, std::io::Error> {
        match self.ordertable.remove(&order.uuid) {
            Some(_) => {
                self.aggrigate_log_sequence += 1;
                order.timestamp = systemtime_to_utc();
                let _removed_zkos_msg = self.zkos_msg.remove(&order.uuid);
                Event::new(
                    Event::LendOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                    format!("remove_order-{}", order.uuid),
                    CORE_EVENT_LOG.clone().to_string(),
                );
                let _ = self.remove_order_check(order.account_id.clone());
                Ok(order)
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Order not found",
                ));
            }
        }
    }
    fn liquidate(
        &mut self,
        order: LendOrder,
        _cmd: RelayerCommand,
    ) -> Result<LendOrder, std::io::Error> {
        Ok(order)
    }
    fn aggrigate_log_sequence(&mut self) -> usize {
        self.aggrigate_log_sequence
    }

    fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
    fn get_zkos_string(&mut self, order_id: Uuid) -> Option<String> {
        let zkos_msg_hex = match self.zkos_msg.get(&order_id) {
            Some(hex_string) => Some(hex_string.clone()),
            None => None,
        };
        zkos_msg_hex
    }
    fn set_zkos_string_on_limit_update(
        &mut self,
        order_id: Uuid,
        zkos_msg: String,
    ) -> Option<String> {
        self.zkos_msg.insert(order_id, zkos_msg)
    }
}

pub fn get_nonce() -> usize {
    let mut lend_pool = LEND_POOL_DB.lock().unwrap();
    lend_pool.get_nonce()
}
pub fn update_nonce() -> usize {
    let mut lend_pool = LEND_POOL_DB.lock().unwrap();
    lend_pool.next_nonce()
}
