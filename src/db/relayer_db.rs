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

#[derive(Debug, Clone)]
pub struct OrderDB<T> {
    pub ordertable: HashMap<Uuid, Arc<RwLock<T>>>,
    pub sequence: usize,
    pub nonce: usize,
    pub event: Vec<Event>,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
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
    fn load_data() -> (bool, OrderDB<T>);
    fn check_backup() -> Self;
    fn liquidate(&mut self, order: T, cmd: RelayerCommand) -> Result<T, std::io::Error>;
}

impl LocalDB<TraderOrder> for OrderDB<TraderOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
        }
    }

    fn add(&mut self, mut order: TraderOrder, cmd: RpcCommand) -> TraderOrder {
        self.sequence += 1;
        order.entry_sequence = self.sequence;
        order.entry_nonce = get_nonce();
        self.ordertable
            .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
        self.aggrigate_log_sequence += 1;
        self.event.push(Event::new(
            Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            String::from("add_order"),
            TRADERORDER_EVENT_LOG.clone().to_string(),
        ));
        order.clone()
    }

    fn update(
        &mut self,
        order: TraderOrder,
        cmd: RelayerCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        if self.ordertable.contains_key(&order.uuid) {
            self.ordertable
                .insert(order.uuid, Arc::new(RwLock::new(order.clone())));
            self.aggrigate_log_sequence += 1;
            self.event.push(Event::new(
                Event::TraderOrderUpdate(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                String::from("update_order"),
                TRADERORDER_EVENT_LOG.clone().to_string(),
            ));
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
        order: TraderOrder,
        cmd: RpcCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        if self.ordertable.contains_key(&order.uuid) {
            match self.ordertable.remove(&order.uuid) {
                Some(_) => {
                    self.aggrigate_log_sequence += 1;
                    // order.exit_nonce = get_nonce();
                    self.event.push(Event::new(
                        Event::TraderOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                        String::from("remove_order"),
                        TRADERORDER_EVENT_LOG.clone().to_string(),
                    ));
                    Ok(order)
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Order not found",
                    ))
                }
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Order not found",
            ));
        }
    }

    fn liquidate(
        &mut self,
        order: TraderOrder,
        cmd: RelayerCommand,
    ) -> Result<TraderOrder, std::io::Error> {
        if self.ordertable.contains_key(&order.uuid) {
            match self.ordertable.remove(&order.uuid) {
                Some(_) => {
                    self.aggrigate_log_sequence += 1;
                    // order.exit_nonce = get_nonce();
                    self.event.push(Event::new(
                        Event::TraderOrderLiquidation(
                            order.clone(),
                            cmd.clone(),
                            self.aggrigate_log_sequence,
                        ),
                        String::from("remove_order"),
                        TRADERORDER_EVENT_LOG.clone().to_string(),
                    ));
                    Ok(order)
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Order not found",
                    ))
                }
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Order not found",
            ));
        }
    }

    fn load_data() -> (bool, OrderDB<TraderOrder>) {
        // fn load_data() -> (bool, Self) {
        let mut database: OrderDB<TraderOrder> = LocalDB::<TraderOrder>::new();
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let eventstop: Event = Event::Stop(time.clone());
        Event::send_event_to_kafka_queue(
            eventstop.clone(),
            TRADERORDER_EVENT_LOG.clone().to_string(),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = Event::receive_event_from_kafka_queue(
            TRADERORDER_EVENT_LOG.clone().to_string(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros()
                .to_string(),
        )
        .unwrap();
        let recever1 = recever.lock().unwrap();
        while stop_signal {
            let data = recever1.recv().unwrap();
            println!("Event logs:{:#?}", data.value);
            match data.value.clone() {
                Event::TraderOrder(order, cmd, seq) => match cmd {
                    RpcCommand::ExecuteTraderOrder(_rpc_request, _metadata) => {
                        let order_clone = order.clone();
                        if database.ordertable.contains_key(&order.uuid) {
                            database.ordertable.remove(&order.uuid);
                        }
                        // database.event.push(data.value);
                        if database.sequence < order_clone.entry_sequence {
                            database.sequence = order_clone.entry_sequence;
                        }
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                    RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                        _rpc_request,
                        _metadata,
                        _payment,
                    ) => {
                        let order_clone = order.clone();
                        if database.ordertable.contains_key(&order.uuid) {
                            database.ordertable.remove(&order.uuid);
                        }
                        // database.event.push(data.value);
                        if database.sequence < order_clone.entry_sequence {
                            database.sequence = order_clone.entry_sequence;
                        }
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                    _ => {
                        let order_clone = order.clone();
                        database
                            .ordertable
                            .insert(order.uuid, Arc::new(RwLock::new(order)));
                        // database.event.push(data.value);
                        if database.sequence < order_clone.entry_sequence {
                            database.sequence = order_clone.entry_sequence;
                        }
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                },
                Event::TraderOrderUpdate(order, _cmd, seq) => {
                    let order_clone = order.clone();
                    database
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                    // database.event.push(data.value);
                    if database.sequence < order_clone.entry_sequence {
                        database.sequence = order_clone.entry_sequence;
                    }
                    if database.aggrigate_log_sequence < seq {
                        database.aggrigate_log_sequence = seq;
                    }
                }
                Event::TraderOrderFundingUpdate(order, _cmd) => {
                    database
                        .ordertable
                        .insert(order.uuid, Arc::new(RwLock::new(order)));
                }
                Event::TraderOrderLiquidation(order, _cmd, seq) => {
                    let order_clone = order.clone();
                    if database.ordertable.contains_key(&order.uuid) {
                        database.ordertable.remove(&order.uuid);
                    }
                    // database.event.push(data.value);
                    if database.sequence < order_clone.entry_sequence {
                        database.sequence = order_clone.entry_sequence;
                    }
                    if database.aggrigate_log_sequence < seq {
                        database.aggrigate_log_sequence = seq;
                    }
                }
                Event::Stop(timex) => {
                    if timex == time {
                        stop_signal = false;
                    }
                }
                Event::LendOrder { .. } => {
                    println!("LendOrder Im here");
                }
                Event::PoolUpdate { .. } => {
                    println!("PoolUpdate Im here");
                }
                Event::FundingRateUpdate(funding_rate, _time) => {
                    set_localdb("FundingRate", funding_rate);
                }
                Event::CurrentPriceUpdate(current_price, _time) => {
                    set_localdb("CurrentPrice", current_price);
                }
            }
        }
        if database.sequence > 0 {
            (true, database.clone())
        } else {
            (false, database)
        }
    }

    fn check_backup() -> Self {
        println!("Loading TraderOrder Database ....");
        let (redis_data, database): (bool, OrderDB<TraderOrder>) =
            OrderDB::<TraderOrder>::load_data();
        if redis_data {
            // println!("uploading db:{:?}", database);
            println!("TraderOrder Database Loaded ....");
            database
        } else {
            println!("No old TraderOrder Database found ....\nCreating new database");
            LocalDB::<TraderOrder>::new()
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
                ))
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
                ))
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
}

impl LocalDB<LendOrder> for OrderDB<LendOrder> {
    fn new() -> Self {
        OrderDB {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            event: Vec::new(),
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
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
        self.event.push(Event::new(
            Event::LendOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
            String::from("add_order"),
            LENDORDER_EVENT_LOG.clone().to_string(),
        ));
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
                ))
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
                ))
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

    fn remove(&mut self, order: LendOrder, cmd: RpcCommand) -> Result<LendOrder, std::io::Error> {
        match self.ordertable.remove(&order.uuid) {
            Some(_) => {
                self.aggrigate_log_sequence += 1;
                self.event.push(Event::new(
                    Event::LendOrder(order.clone(), cmd.clone(), self.aggrigate_log_sequence),
                    String::from("remove_order"),
                    LENDORDER_EVENT_LOG.clone().to_string(),
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

    fn load_data() -> (bool, OrderDB<LendOrder>) {
        // fn load_data() -> (bool, Self) {
        let mut database: OrderDB<LendOrder> = LocalDB::<LendOrder>::new();
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let eventstop: Event = Event::Stop(time.clone());
        Event::send_event_to_kafka_queue(
            eventstop.clone(),
            LENDORDER_EVENT_LOG.clone().to_string(),
            String::from("StopLoadMSG"),
        );
        let mut stop_signal: bool = true;

        let recever = Event::receive_event_from_kafka_queue(
            LENDORDER_EVENT_LOG.clone().to_string(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros()
                .to_string(),
        )
        .unwrap();
        let recever1 = recever.lock().unwrap();
        while stop_signal {
            let data = recever1.recv().unwrap();
            match data.value.clone() {
                Event::LendOrder(order, cmd, seq) => match cmd {
                    RpcCommand::CreateLendOrder(..) => {
                        let order_clone = order.clone();
                        database
                            .ordertable
                            .insert(order.uuid, Arc::new(RwLock::new(order)));
                        // database.event.push(data.value);
                        if database.sequence < order_clone.entry_sequence {
                            database.sequence = order_clone.entry_sequence;
                        }
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                    }
                    RpcCommand::ExecuteLendOrder(..) => {
                        // database.event.push(data.value);

                        if database.ordertable.contains_key(&order.uuid) {
                            database.ordertable.remove(&order.uuid);
                        }
                        if database.aggrigate_log_sequence < seq {
                            database.aggrigate_log_sequence = seq;
                        }
                        if database.sequence < order.entry_sequence {
                            database.sequence = order.entry_sequence;
                        }
                    }
                    _ => {}
                },
                Event::Stop(timex) => {
                    if timex == time {
                        stop_signal = false;
                    }
                }
                Event::TraderOrder { .. } => {}
                Event::TraderOrderUpdate { .. } => {}
                Event::TraderOrderLiquidation { .. } => {}
                Event::TraderOrderFundingUpdate { .. } => {}
                Event::FundingRateUpdate { .. } => {}
                Event::PoolUpdate { .. } => {}
                Event::CurrentPriceUpdate { .. } => {}
            }
        }
        if database.sequence > 0 {
            (true, database.clone())
        } else {
            (false, database)
        }
    }

    fn check_backup() -> Self {
        println!("Loading LendOrder Database ....");
        let (redis_data, database): (bool, OrderDB<LendOrder>) = OrderDB::<LendOrder>::load_data();
        if redis_data {
            // println!("uploading db:{:?}", database);
            println!("LendOrder Database Loaded ....");
            database
        } else {
            println!("No old LendOrder Database found ....\nCreating new database");
            LocalDB::<LendOrder>::new()
        }
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
