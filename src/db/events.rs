#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::BROKERS;

pub const EVENTLOG_VERSION: &str = "v0.1.3";
use crate::db::*;
use crate::kafkalib::kafka_health::{self, ResilientProducer};
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::kafkalib::persistent_queue;
use crate::relayer::*;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use jsonrpc::Request;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use std::time::{self, Duration};
use std::{fmt, thread};
use uuid::serde::compact::serialize;
use uuid::Uuid;
pub type OffsetCompletion = (i32, i64);
pub type Nonce = usize;
lazy_static! {
    // pub static ref GLOBAL_NONCE: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    pub static ref KAFKA_EVENT_LOG_THREADPOOL1: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(1, String::from("KAFKA_EVENT_LOG_THREADPOOL1"))
    );
    pub static ref KAFKA_EVENT_LOG_THREADPOOL2: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(1, String::from("KAFKA_EVENT_LOG_THREADPOOL2"))
    );
    pub static ref KAFKA_PRODUCER_EVENT: ResilientProducer = ResilientProducer::new(3);
}

#[derive(Debug)]
pub struct EventLog {
    pub offset: i64,
    pub key: String,
    pub value: Event,
    pub partition: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EventKey {
    pub agg_id: String,
    pub event_type: String,
    pub event_version: String,
    pub metadata: Option<HashMap<String, String>>,
}
impl EventKey {
    pub fn default() -> Self {
        EventKey {
            agg_id: "".to_string(),
            event_type: "".to_string(),
            event_version: EVENTLOG_VERSION.to_string(),
            metadata: None,
        }
    }
    pub fn new(agg_id: String, event_type: String) -> Self {
        EventKey {
            agg_id,
            event_type,
            event_version: EVENTLOG_VERSION.to_string(),
            metadata: None,
        }
    }
    pub fn new_with_version(agg_id: String, event_type: String, event_version: String) -> Self {
        EventKey {
            agg_id,
            event_type,
            event_version,
            metadata: None,
        }
    }

    pub fn add_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
    pub fn add_in_metadata(&mut self, key: String, value: String) -> bool {
        match &mut self.metadata {
            Some(metadata) => metadata.insert(key, value).is_none(),
            None => {
                let mut metadata = HashMap::new();
                let bool = metadata.insert(key, value).is_none();
                self.metadata = Some(metadata);
                bool
            }
        }
    }

    pub fn to_string_or_default(&self) -> String {
        match serde_json::to_string(self) {
            Ok(value) => value,
            Err(_arg) => "".to_string(),
        }
    }
    pub fn from_string_or_default(serialize_event_key: String) -> Self {
        match serde_json::from_str(&serialize_event_key) {
            Ok(event_key) => event_key,
            Err(arg) => {
                let mut key = EventKey::default();
                key.event_type = serialize_event_key;
                key.event_version = "0.0.0".to_string();
                let _ = key.add_in_metadata("Error".to_string(), arg.to_string());
                key
            }
        }
    }
    pub fn is_upcast(&self) -> bool {
        self.event_version != EVENTLOG_VERSION
    }
    // event type casting for load any order kafka logs for different version of event log
    pub fn event_log_upcast(&mut self, log: String) -> String {
        match &*self.event_version {
            // v0.1.0 is the current version of the event log code example to upgrade the event log version for any struct change in commands
            // v0.0.0 → v0.1.0
            "v0.0.0" => match &*self.event_type {
                _ => {
                    self.event_version = "v0.1.1".to_string();
                }
            },
            // v0.1.0 → v0.1.1
            "v0.1.0" => match &*self.event_type {
                "SortedSetDBUpdate" => {
                    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                    pub enum EventOld {
                        SortedSetDBUpdate(SortedSetCommand),
                    }

                    let log_der: EventOld = match serde_json::from_str(&log) {
                        Ok(v) => v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error upcasting SortedSetDBUpdate from v0.1.0: {:?}",
                                e
                            );
                            self.event_version = "v0.1.1".to_string();
                            return log;
                        }
                    };
                    let EventOld::SortedSetDBUpdate(cmd) = log_der;
                    let new_log = Event::SortedSetDBUpdate(
                        cmd,
                        crate::relayer::iso8601(&std::time::SystemTime::now()),
                    );
                    self.event_version = "v0.1.1".to_string();
                    match serde_json::to_string(&new_log) {
                        Ok(v) => return v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error serializing upcasted SortedSetDBUpdate: {:?}",
                                e
                            );
                            return log;
                        }
                    }
                }
                _ => {
                    self.event_version = "v0.1.1".to_string();
                }
            },
            // v0.1.1 → v0.1.2
            "v0.1.1" => match &*self.event_type {
                "TxHash" => {
                    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                    pub enum EventOldTxHash {
                        TxHash(
                            Uuid,
                            String,
                            String,
                            OrderType,
                            OrderStatus,
                            String,
                            Option<String>,
                            RequestID,
                        ),
                    }

                    let log_der: EventOldTxHash = match serde_json::from_str(&log) {
                        Ok(v) => v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error upcasting TxHash from v0.1.1: {:?}",
                                e
                            );
                            self.event_version = "v0.1.2".to_string();
                            return log;
                        }
                    };
                    let EventOldTxHash::TxHash(
                        order_id,
                        account_id,
                        tx_hash,
                        order_type,
                        order_status,
                        datetime,
                        output,
                        request_id,
                    ) = log_der;
                    let new_log = Event::TxHash(TxHashData {
                        order_id,
                        account_id,
                        tx_hash,
                        order_type,
                        order_status,
                        datetime,
                        output,
                        request_id,
                        reason: None,
                        old_price: None,
                        new_price: None,
                    });
                    self.event_version = "v0.1.2".to_string();
                    match serde_json::to_string(&new_log) {
                        Ok(v) => return v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error serializing upcasted TxHash: {:?}",
                                e
                            );
                            return log;
                        }
                    }
                }
                "TxHashUpdate" => {
                    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                    pub enum EventOldTxHashUpdate {
                        TxHashUpdate(
                            Uuid,
                            String,
                            String,
                            OrderType,
                            OrderStatus,
                            String,
                            Option<String>,
                        ),
                    }

                    let log_der: EventOldTxHashUpdate = match serde_json::from_str(&log) {
                        Ok(v) => v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error upcasting TxHashUpdate from v0.1.1: {:?}",
                                e
                            );
                            self.event_version = "v0.1.2".to_string();
                            return log;
                        }
                    };
                    let EventOldTxHashUpdate::TxHashUpdate(
                        order_id,
                        account_id,
                        tx_hash,
                        order_type,
                        order_status,
                        datetime,
                        output,
                    ) = log_der;
                    let new_log = Event::TxHashUpdate(TxHashData {
                        order_id,
                        account_id,
                        tx_hash,
                        order_type,
                        order_status,
                        datetime,
                        output,
                        request_id: String::new(),
                        reason: None,
                        old_price: None,
                        new_price: None,
                    });
                    self.event_version = "v0.1.2".to_string();
                    match serde_json::to_string(&new_log) {
                        Ok(v) => return v,
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "Error serializing upcasted TxHashUpdate: {:?}",
                                e
                            );
                            return log;
                        }
                    }
                }
                _ => {
                    self.event_version = "v0.1.2".to_string();
                }
            },
            "v0.1.2" => match &*self.event_type {
                "TraderOrderUpdate"
                | "TraderOrderFundingUpdate"
                | "TraderOrderLiquidation"
                | "FeeUpdate" => {
                    // v0.1.2 -> v0.1.3: PriceTickerOrderSettle changed from 3 fields to 4 (added OrderType).
                    // Old: PriceTickerOrderSettle(Vec<Uuid>, Meta, f64)
                    // New: PriceTickerOrderSettle(Vec<Uuid>, Meta, f64, OrderType)
                    // Default to OrderType::LIMIT since that was the only settle type before.
                    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
                    pub enum RelayerCommandOld {
                        FundingCycle(PoolBatchOrder, Meta, f64),
                        FundingOrderEventUpdate(TraderOrder, Meta),
                        PriceTickerLiquidation(Vec<Uuid>, Meta, f64),
                        PriceTickerOrderFill(Vec<Uuid>, Meta, f64),
                        PriceTickerOrderSettle(Vec<Uuid>, Meta, f64),
                        FundingCycleLiquidation(Vec<Uuid>, Meta, f64),
                        RpcCommandPoolupdate(),
                        UpdateFees(f64, f64, f64, f64),
                    }

                    fn upcast_relayer_command(old: RelayerCommandOld) -> RelayerCommand {
                        match old {
                            RelayerCommandOld::PriceTickerOrderSettle(ids, meta, price) => {
                                RelayerCommand::PriceTickerOrderSettle(
                                    ids,
                                    meta,
                                    price,
                                    OrderType::LIMIT,
                                )
                            }
                            RelayerCommandOld::FundingCycle(a, b, c) => {
                                RelayerCommand::FundingCycle(a, b, c)
                            }
                            RelayerCommandOld::FundingOrderEventUpdate(a, b) => {
                                RelayerCommand::FundingOrderEventUpdate(a, b)
                            }
                            RelayerCommandOld::PriceTickerLiquidation(a, b, c) => {
                                RelayerCommand::PriceTickerLiquidation(a, b, c)
                            }
                            RelayerCommandOld::PriceTickerOrderFill(a, b, c) => {
                                RelayerCommand::PriceTickerOrderFill(a, b, c)
                            }
                            RelayerCommandOld::FundingCycleLiquidation(a, b, c) => {
                                RelayerCommand::FundingCycleLiquidation(a, b, c)
                            }
                            RelayerCommandOld::RpcCommandPoolupdate() => {
                                RelayerCommand::RpcCommandPoolupdate()
                            }
                            RelayerCommandOld::UpdateFees(a, b, c, d) => {
                                RelayerCommand::UpdateFees(a, b, c, d)
                            }
                        }
                    }

                    let result: Option<String> = match &*self.event_type {
                        "TraderOrderUpdate" => {
                            #[derive(Serialize, Deserialize)]
                            pub enum EventOld {
                                TraderOrderUpdate(TraderOrder, RelayerCommandOld, usize),
                            }
                            serde_json::from_str::<EventOld>(&log).ok().map(|e| {
                                let EventOld::TraderOrderUpdate(order, cmd, seq) = e;
                                let new_cmd = upcast_relayer_command(cmd);
                                serde_json::to_string(&Event::TraderOrderUpdate(
                                    order, new_cmd, seq,
                                ))
                                .unwrap()
                            })
                        }
                        "TraderOrderFundingUpdate" => {
                            #[derive(Serialize, Deserialize)]
                            pub enum EventOld {
                                TraderOrderFundingUpdate(TraderOrder, RelayerCommandOld),
                            }
                            serde_json::from_str::<EventOld>(&log).ok().map(|e| {
                                let EventOld::TraderOrderFundingUpdate(order, cmd) = e;
                                let new_cmd = upcast_relayer_command(cmd);
                                serde_json::to_string(&Event::TraderOrderFundingUpdate(
                                    order, new_cmd,
                                ))
                                .unwrap()
                            })
                        }
                        "TraderOrderLiquidation" => {
                            #[derive(Serialize, Deserialize)]
                            pub enum EventOld {
                                TraderOrderLiquidation(TraderOrder, RelayerCommandOld, usize),
                            }
                            serde_json::from_str::<EventOld>(&log).ok().map(|e| {
                                let EventOld::TraderOrderLiquidation(order, cmd, seq) = e;
                                let new_cmd = upcast_relayer_command(cmd);
                                serde_json::to_string(&Event::TraderOrderLiquidation(
                                    order, new_cmd, seq,
                                ))
                                .unwrap()
                            })
                        }
                        "FeeUpdate" => {
                            #[derive(Serialize, Deserialize)]
                            pub enum EventOld {
                                FeeUpdate(RelayerCommandOld, String),
                            }
                            serde_json::from_str::<EventOld>(&log).ok().map(|e| {
                                let EventOld::FeeUpdate(cmd, time) = e;
                                let new_cmd = upcast_relayer_command(cmd);
                                serde_json::to_string(&Event::FeeUpdate(new_cmd, time)).unwrap()
                            })
                        }
                        _ => None,
                    };

                    if let Some(new_log) = result {
                        self.event_version = EVENTLOG_VERSION.to_string();
                        return new_log;
                    }
                    // Either no PriceTickerOrderSettle in this event (deserializes fine with new format),
                    // or deserialization failed — just bump version and let the main deserializer handle it.
                    self.event_version = EVENTLOG_VERSION.to_string();
                }
                _ => {
                    self.event_version = "v0.1.3".to_string();
                }
            },
            _ => {}
        }
        log
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TxHashData {
    pub order_id: Uuid,
    pub account_id: String,
    pub tx_hash: String,
    pub order_type: OrderType,
    pub order_status: OrderStatus,
    pub datetime: String,
    pub output: Option<String>,
    pub request_id: RequestID,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub old_price: Option<f64>,
    #[serde(default)]
    pub new_price: Option<f64>,
}

impl TxHashData {
    pub fn new(
        order_id: Uuid,
        account_id: String,
        tx_hash: String,
        order_type: OrderType,
        order_status: OrderStatus,
        request_id: RequestID,
    ) -> Self {
        TxHashData {
            order_id,
            account_id,
            tx_hash,
            order_type,
            order_status,
            datetime: crate::relayer::iso8601(&std::time::SystemTime::now()),
            output: None,
            request_id,
            reason: None,
            old_price: None,
            new_price: None,
        }
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn with_output(mut self, output: Option<String>) -> Self {
        self.output = output;
        self
    }

    pub fn with_old_price(mut self, price: f64) -> Self {
        self.old_price = Some(price);
        self
    }

    pub fn with_new_price(mut self, price: f64) -> Self {
        self.new_price = Some(price);
        self
    }

    pub fn with_datetime(mut self, datetime: String) -> Self {
        self.datetime = datetime;
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    TraderOrder(TraderOrder, RpcCommand, usize),
    TraderOrderUpdate(TraderOrder, RelayerCommand, usize),
    TraderOrderLimitUpdate(TraderOrder, RpcCommand, usize),
    TraderOrderFundingUpdate(TraderOrder, RelayerCommand),
    TraderOrderLiquidation(TraderOrder, RelayerCommand, usize),
    LendOrder(LendOrder, RpcCommand, usize),
    PoolUpdate(LendPoolCommand, LendPool, usize),
    FundingRateUpdate(f64, f64, String), //funding rate, btc price, time
    CurrentPriceUpdate(f64, String),
    SortedSetDBUpdate(SortedSetCommand, String), //command, time
    PositionSizeLogDBUpdate(PositionSizeLogCommand, PositionSizeLog),
    TxHash(TxHashData),
    TxHashUpdate(TxHashData),
    Stop(String),
    AdvanceStateQueue(Nonce, twilight_relayer_sdk::zkvm::Output),
    FeeUpdate(RelayerCommand, String), //fee data and time
    RiskEngineUpdate(RiskEngineCommand, RiskState),
    RiskParamsUpdate(RiskParams),
}

impl Event {
    pub fn new(event: Event, key: String, topic: String) {
        match KAFKA_EVENT_LOG_THREADPOOL1.lock() {
            Ok(pool) => {
                pool.execute(move || {
                    let mut attempt: u64 = 0;
                    loop {
                        match Event::send_event_to_kafka_queue(
                            event.clone(),
                            topic.clone(),
                            key.clone(),
                        ) {
                            Ok(_) => break,
                            Err(e) => {
                                attempt += 1;
                                crate::log_heartbeat!(
                                    error,
                                    "Error dispatching event to Kafka (attempt {}), retrying: {:?}",
                                    attempt,
                                    e
                                );
                                kafka_health::backoff_sleep(attempt);
                            }
                        }
                    }
                });
                drop(pool);
            }
            Err(e) => {
                crate::log_heartbeat!(error, "Error locking Kafka event log thread pool: {:?}", e);
            }
        }
    }
    pub fn send_event_to_kafka_queue(
        event: Event,
        topic: String,
        key: String,
    ) -> Result<(), String> {
        // updating the event key and data to send to kafka queue
        let key = EventKey::new(key, event.get_event_type()).to_string_or_default();
        let data = match serde_json::to_vec(&event) {
            Ok(data) => data,
            Err(e) => {
                crate::log_heartbeat!(error, "Error serializing event: {:?}", e);
                return Err(e.to_string());
            }
        };

        // 1. Persist to disk FIRST (crash-safe)
        let queue_id = persistent_queue::EVENT_QUEUE
            .push(topic.clone(), key.clone(), data.clone())
            .map_err(|e| format!("Failed to persist event: {}", e))?;

        // 2. Then dispatch to thread pool for async Kafka send
        match KAFKA_EVENT_LOG_THREADPOOL2.lock() {
            Ok(pool) => {
                pool.execute(move || {
                    let mut attempt: u64 = 0;
                    loop {
                        match KAFKA_PRODUCER_EVENT.send(&Record::from_key_value(
                            &topic,
                            key.clone(),
                            data.clone(),
                        )) {
                            Ok(_) => {
                                // 3. Remove from persistent queue only after successful send
                                if let Err(e) = persistent_queue::EVENT_QUEUE.remove(queue_id) {
                                    crate::log_heartbeat!(
                                        error,
                                        "Failed to remove event {} from persistent queue: {}",
                                        queue_id,
                                        e
                                    );
                                }
                                break;
                            }
                            Err(e) => {
                                attempt += 1;
                                crate::log_heartbeat!(
                                    error,
                                    "Event send to Kafka failed (attempt {}), retrying: {:?}",
                                    attempt,
                                    e
                                );
                                kafka_health::backoff_sleep(attempt);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                crate::log_heartbeat!(
                    error,
                    "Error locking Kafka producer event log thread pool: {:?}",
                    e
                );
                return Err(e.to_string());
            }
        }

        Ok(())
    }

    pub fn receive_event_for_snapshot_from_kafka_queue(
        topic: String,
        group: String,
        fetchoffset: FetchOffset,
        thread_name: &str,
    ) -> Result<(Arc<Mutex<Receiver<EventLog>>>, Sender<OffsetCompletion>), KafkaError> {
        let (sender, receiver) = bounded::<EventLog>(10_000);
        let (tx_consumed, rx_consumed) = unbounded::<OffsetCompletion>();
        // let _topic_clone = topic.clone();
        let _handle = match
            thread::Builder
                ::new()
                .name(String::from(thread_name))
                .spawn(move || {
                    match
                        Consumer::from_hosts(BROKERS.clone())
                            // .with_topic(topic)
                            .with_group(group.clone())
                            .with_topic_partitions(topic.clone(), &[0])
                            .with_fallback_offset(fetchoffset)
                            .with_offset_storage(GroupOffsetStorage::Kafka)
                            .create()
                    {
                        Ok(mut con) => {
                            let mut connection_status = true;
                            let _partition: i32 = 0;
                            let mut snapshot_poll_failures: u64 = 0;
                            while connection_status {
                                let sender_clone = sender.clone();
                                match con.poll() {
                                    Ok(mss) => {
                                        snapshot_poll_failures = 0;
                                        if mss.is_empty() {
                                            // println!("No messages available right now.");
                                            // return Ok(());
                                        } else {
                                            for ms in mss.iter() {
                                                for m in ms.messages() {
                                                    let mut eventkey =
                                                        EventKey::from_string_or_default(
                                                            String::from_utf8_lossy(
                                                                &m.key
                                                            ).to_string()
                                                        );
                                                    let mut value = String::from_utf8_lossy(
                                                        &m.value
                                                    ).to_string();
                                                    while eventkey.is_upcast() {
                                                        value = eventkey.event_log_upcast(value);
                                                    }
                                                    let value = match serde_json::from_str(&value) {
                                                        Ok(ser_value) => ser_value,
                                                        Err(arg) => {
                                                            crate::log_heartbeat!(
                                                                error,
                                                                "Error in event log snapshot for upcasting event: {:?}",
                                                                arg
                                                            );
                                                            crate::log_heartbeat!(
                                                                warn,
                                                                "skipping corrupted log"
                                                            );
                                                            continue;
                                                        }
                                                    };
                                                    let message = EventLog {
                                                        offset: m.offset,
                                                        key: String::from_utf8_lossy(
                                                            &m.key
                                                        ).to_string(),
                                                        value: value,
                                                        partition: ms.partition(),
                                                    };
                                                    match sender_clone.send(message) {
                                                        Ok(_) => {

                                                        }
                                                        Err(_arg) => {
                                                            crate::log_heartbeat!(
                                                                warn,
                                                                "Sender Dropped from Snapshot : received last updated event"
                                                            );
                                                            connection_status = false;
                                                            break;
                                                        }
                                                    }
                                                }
                                                if connection_status == false {
                                                    break;
                                                }
                                            }
                                        }
                                        if !connection_status {
                                            match rx_consumed.recv() {
                                                Ok((partition, offset)) => {
                                                    let e = con.consume_message(
                                                        &topic,
                                                        partition,
                                                        offset
                                                    );

                                                    if e.is_err() {
                                                        crate::log_heartbeat!(
                                                            error,
                                                            "Kafka connection failed {:?}",
                                                            e
                                                        );
                                                        // connection_status = false;
                                                        break;
                                                    }

                                                    let e = con.commit_consumed();
                                                    if e.is_err() {
                                                        crate::log_heartbeat!(
                                                            error,
                                                            "Kafka connection failed {:?}",
                                                            e
                                                        );
                                                        // connection_status = false;
                                                        break;
                                                    }
                                                    // connection_status = false;
                                                    break;
                                                }
                                                Err(_e) => {
                                                    // connection_status = false;
                                                    crate::log_heartbeat!(
                                                        error,
                                                        "The consumed channel is closed: {:?}",
                                                        thread::current().name()
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        snapshot_poll_failures += 1;
                                        kafka_health::record_kafka_failure();
                                        crate::log_heartbeat!(warn, "Kafka poll error event.rs (group: {}. attempt {}): {}", group, snapshot_poll_failures, e);
                                        if snapshot_poll_failures > 3 {
                                            crate::log_heartbeat!(warn, "Snapshot consumer: too many poll failures, closing consumer thread");
                                            // connection_status = false;
                                            break;
                                        }
                                        kafka_health::backoff_sleep(snapshot_poll_failures);
                                    }
                                }
                            }
                            thread::sleep(time::Duration::from_millis(3000));
                        }
                        Err(e) => {
                            kafka_health::record_kafka_failure();
                            crate::log_heartbeat!(error, "Kafka connection failed: {}", e);
                            drop(sender);
                            return;
                        }
                    }
                })
        {
            Ok(handle) => handle,
            Err(e) => {
                crate::log_heartbeat!(error, "Kafka connection failed {:?}", e);
                return Err(e.into());
            }
        };
        Ok((Arc::new(Mutex::new(receiver)), tx_consumed))
    }

    pub fn get_event_type(&self) -> String {
        match self {
            Event::TraderOrder(..) => "TraderOrder".to_string(),
            Event::TraderOrderUpdate(..) => "TraderOrderUpdate".to_string(),
            Event::TraderOrderFundingUpdate(..) => "TraderOrderFundingUpdate".to_string(),
            Event::TraderOrderLiquidation(..) => "TraderOrderLiquidation".to_string(),
            Event::LendOrder(..) => "LendOrder".to_string(),
            Event::PoolUpdate(..) => "PoolUpdate".to_string(),
            Event::FundingRateUpdate(..) => "FundingRateUpdate".to_string(),
            Event::CurrentPriceUpdate(..) => "CurrentPriceUpdate".to_string(),
            Event::SortedSetDBUpdate(..) => "SortedSetDBUpdate".to_string(),
            Event::PositionSizeLogDBUpdate(..) => "PositionSizeLogDBUpdate".to_string(),
            Event::TxHash(..) => "TxHash".to_string(),
            Event::TxHashUpdate(..) => "TxHashUpdate".to_string(),
            Event::Stop(..) => "Stop".to_string(),
            Event::AdvanceStateQueue(..) => "AdvanceStateQueue".to_string(),
            Event::TraderOrderLimitUpdate(..) => "TraderOrderLimitUpdate".to_string(),
            Event::FeeUpdate(..) => "FeeUpdate".to_string(),
            Event::RiskEngineUpdate(..) => "RiskEngineUpdate".to_string(),
            Event::RiskParamsUpdate(..) => "RiskParamsUpdate".to_string(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::EventKey;

    #[test]
    fn test_eventlog_key() {
        let mut metadata = HashMap::new();
        metadata.insert("key1", "value1");
        let mut event_key = EventKey::new("agg_id".to_string(), "price_update".to_string());

        println!("event_key:{:?}", event_key);
        event_key.add_in_metadata("key".to_string(), "value".to_string());
        println!("event_key_meta:{:?}", event_key);
        event_key.add_in_metadata("key2".to_string(), "value2".to_string());
        println!("event_key_meta_new:{:?}", event_key);
    }
}
