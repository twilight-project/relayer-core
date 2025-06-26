#![allow(dead_code)]
#![allow(unused_imports)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafkacmd::KAFKA_PRODUCER;
use crate::relayer::*;
// use bincode::{config, Decode, Encode};
use bincode;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::Error as KafkaError;
use kafka::producer::Record;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;
// lazy_static! {
//     // pub static ref SNAPSHOT_DATA: Arc<Mutex<SnapshotDB>> = Arc::new(Mutex::new(SnapshotDB::new()));
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShotTO {
    pub ordertable: HashMap<Uuid, TraderOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}
impl OrderDBSnapShotTO {
    fn new() -> Self {
        OrderDBSnapShotTO {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            zkos_msg: HashMap::new(),
            hash: HashSet::new(),
        }
    }
    fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDBSnapShotLO {
    pub ordertable: HashMap<Uuid, LendOrder>,
    pub sequence: usize,
    pub nonce: usize,
    pub aggrigate_log_sequence: usize,
    pub last_snapshot_id: usize,
    pub zkos_msg: HashMap<Uuid, ZkosHexString>,
    pub hash: HashSet<String>,
}
impl OrderDBSnapShotLO {
    fn new() -> Self {
        OrderDBSnapShotLO {
            ordertable: HashMap::new(),
            sequence: 0,
            nonce: 0,
            aggrigate_log_sequence: 0,
            last_snapshot_id: 0,
            zkos_msg: HashMap::new(),
            hash: HashSet::new(),
        }
    }
    fn remove_order_check(&mut self, account_id: String) -> bool {
        self.hash.remove(&account_id)
    }
    fn set_order_check(&mut self, account_id: String) -> bool {
        self.hash.insert(account_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDB {
    pub orderdb_traderorder: OrderDBSnapShotTO,
    pub orderdb_lendorder: OrderDBSnapShotLO,
    pub lendpool_database: LendPool,
    pub liquidation_long_sortedset_db: SortedSet,
    pub liquidation_short_sortedset_db: SortedSet,
    pub open_long_sortedset_db: SortedSet,
    pub open_short_sortedset_db: SortedSet,
    pub close_long_sortedset_db: SortedSet,
    pub close_short_sortedset_db: SortedSet,
    pub position_size_log: PositionSizeLog,
    pub localdb_hashmap: HashMap<String, f64>,
    // pub output_memo_hashmap: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    pub event_offset_partition: (i32,i64),
    pub event_timestamp: String,
    pub output_hex_storage: utxo_in_memory::db::LocalStorage<Option<zkvm::zkos_types::Output>>,
    queue_manager:QueueState
}
impl SnapshotDB {
    fn new() -> Self {
        SnapshotDB {
            orderdb_traderorder: OrderDBSnapShotTO::new(),
            orderdb_lendorder: OrderDBSnapShotLO::new(),
            lendpool_database: LendPool::default(),
            liquidation_long_sortedset_db: SortedSet::new(),
            liquidation_short_sortedset_db: SortedSet::new(),
            open_long_sortedset_db: SortedSet::new(),
            open_short_sortedset_db: SortedSet::new(),
            close_long_sortedset_db: SortedSet::new(),
            close_short_sortedset_db: SortedSet::new(),
            position_size_log: PositionSizeLog::new(),
            localdb_hashmap: {
                let mut hashmap = HashMap::new();

                let filled_on_market = std::env::var("FILLED_ON_MARKET").unwrap_or("0.04".to_string()).parse::<f64>().unwrap_or(0.04);
                let filled_on_limit = std::env::var("FILLED_ON_LIMIT").unwrap_or("0.02".to_string()).parse::<f64>().unwrap_or(0.02);
                let settled_on_market = std::env::var("SETTLED_ON_MARKET").unwrap_or("0.04".to_string()).parse::<f64>().unwrap_or(0.04);
                let settled_on_limit = std::env::var("SETTLED_ON_LIMIT").unwrap_or("0.02".to_string()).parse::<f64>().unwrap_or(0.02);



                hashmap.insert(FeeType::FilledOnMarket.into(), filled_on_market);
                hashmap.insert(FeeType::FilledOnLimit.into(), filled_on_limit);
                hashmap.insert(FeeType::SettledOnMarket.into(), settled_on_market);
                hashmap.insert(FeeType::SettledOnLimit.into(), settled_on_limit);
                let event_time=systemtime_to_utc();
                Event::send_event_to_kafka_queue(
                    Event::FeeUpdate(RelayerCommand::UpdateFees(filled_on_market, filled_on_limit, settled_on_market, settled_on_limit), event_time.clone()),
                    CORE_EVENT_LOG.clone().to_string(),
                    format!("FeeUpdate-{}", event_time),
                );
                hashmap
            },
            event_offset_partition: (0,0),
            event_timestamp: ServerTime::now().epoch,
            output_hex_storage:
                utxo_in_memory::db::LocalStorage::<Option<zkvm::zkos_types::Output>>::new(1),
                queue_manager:QueueState::new()
        }
    }
    fn print_status(&self) {
        if self.orderdb_traderorder.sequence > 0 {
            println!("TraderOrder Database Loaded ....");
        } else {
            println!("No old TraderOrder Database found ....\nCreating new TraderOrder_database");
        }
        if self.orderdb_lendorder.sequence > 0 {
            println!("LendOrder Database Loaded ....");
        } else {
            println!("No old LendOrder Database found ....\nCreating new LendOrder_database");
        }
        if self.lendpool_database.aggrigate_log_sequence > 0 {
            println!("LendPool Database Loaded ....");
        } else {
            println!("No old LendPool Database found ....\nCreating new LendPool_database");
        }
    }
}

pub fn snapshot() -> Result<SnapshotDB, String> {
    // let read_snapshot = fs::read("snapshot").expect("Could not read file");
    // snapshot renaming on success
    // encryption on snapshot data
    // snapshot version
    // delete old snapshot data deleted by cron job
    println!("started taking snapshot");
    let read_snapshot = fs::read(format!(
        "{}-{}",
        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
    ));
    let decoded_snapshot: SnapshotDB;
    let mut is_file_exist = false;
    // let last_snapshot_time: String;
    let fetchoffset: FetchOffset;
    match read_snapshot {
        Ok(snapshot_data_from_file) => {
            decoded_snapshot = match bincode::deserialize(&snapshot_data_from_file) {
                Ok(snap_data) => {
                    is_file_exist = true;
                    // last_snapshot_time = decoded_snapshot.event_timestamp.clone();
                    fetchoffset = FetchOffset::Earliest;
                    snap_data
                }
                Err(arg) => {
                    // .expect("Could not decode vector");
                    println!(
                        "No previous Snapshot Found- Error:{:#?} \n path: {:?}",
                        arg,
                        format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
                    );
                    // last_snapshot_time = decoded_snapshot.event_timestamp.clone();
                    fetchoffset = FetchOffset::Earliest;
                    SnapshotDB::new()
                }
            }
        }
        Err(arg) => {
            println!(
                "No previous Snapshot Found- Error:{:#?} \n path: {:?}",
                arg,
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
            );
            decoded_snapshot = SnapshotDB::new();
            // last_snapshot_time = decoded_snapshot.event_timestamp.clone();
            fetchoffset = FetchOffset::Earliest;
        }
    }

    let (snapshot_db_updated,tx_consumed) = match create_snapshot_data(fetchoffset, decoded_snapshot) {
        Ok(snap_data) => snap_data,
        Err(arg) => {
            return Err(arg.to_string());
        }
    };

    let encoded_v = bincode::serialize(&snapshot_db_updated).expect("Could not encode vector");
    match fs::write(
        format!(
            "{}-{}-new",
            *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
        ),
        encoded_v,
    ) {
        Ok(_) => {
            if is_file_exist {
                if let Err(e) = fs::remove_file(format!(
                    "{}-{}",
                    *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                )) {
                    println!("Can not delete snapshot file, \n Error: {:?}", e);
                }
            }
            fs::rename(
                format!(
                    "{}-{}-new",
                    *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                ),
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
            )
            .unwrap();
        match tx_consumed.send(snapshot_db_updated.event_offset_partition){
            Ok(_)=>{}
            Err(_)=>{}
        }
        }
        Err(arg) => {
            println!("Could not write snapshot file - Error:{:#?}", arg);
        }
    }
    println!("Snapshot Done");

    Ok(snapshot_db_updated)
}

pub fn create_snapshot_data(
    fetchoffset: FetchOffset,
    snapshot_db: SnapshotDB,
) -> Result<(SnapshotDB,crossbeam_channel::Sender<(i32, i64)>), String> {
    let SnapshotDB {
        mut orderdb_traderorder,
        mut orderdb_lendorder,
        mut lendpool_database,
        mut liquidation_long_sortedset_db,
        mut liquidation_short_sortedset_db,
        mut open_long_sortedset_db,
        mut open_short_sortedset_db,
        mut close_long_sortedset_db,
        mut close_short_sortedset_db,
        mut position_size_log,
        mut localdb_hashmap,
        mut event_offset_partition,
        event_timestamp: _,
        mut output_hex_storage,
        mut queue_manager
    } = snapshot_db;

    let time = ServerTime::now().epoch;
    let event_timestamp = time.clone();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut stop_signal: bool = true;
    let mut retry_attempt = 0;
    while retry_attempt < 5 {
        match Event::receive_event_for_snapshot_from_kafka_queue(
            CORE_EVENT_LOG.clone().to_string(),
            format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION),
            fetchoffset,"snapshot handle"
        ) {
            Ok((receiver_lock,tx_consumed)) => {
                let recever1 = 
                // receiver_lock.lock().unwrap();
               match receiver_lock.lock(){
                Ok(rec_lock)=>{rec_lock}
                Err(arg)=>{
                    retry_attempt+=1;
                    println!("unable to lock the kafka log receiver :{:?}",arg);
                    continue;
                }
               };
               let mut last_offset = 0;
                while stop_signal {

                    // let data = recever1.recv().unwrap();
                    // match data.value {
                    //     Event::CurrentPriceUpdate(..) => {}
                    //     _ => {
                    //         println!("Envent log: {:#?}", data);
                    //     }
                    // }

                    match recever1.recv(){
                        Ok(data)=>{

                            match data.value.clone() {
                                Event::TraderOrder(order, cmd, seq) => match cmd {
                                    RpcCommand::CreateTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // let order_clone = order.clone();
                                        orderdb_traderorder
                                            .ordertable
                                            .insert(order.uuid, order.clone());
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                            orderdb_traderorder.sequence = order.entry_sequence.clone();
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }

                                        match order.order_status {
                                            OrderStatus::FILLED => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ = liquidation_long_sortedset_db.add(
                                                        order.uuid,
                                                        (order.liquidation_price * 10000.0) as i64,
                                                    );
                                                }
                                                PositionType::SHORT => {
                                                    let _ = liquidation_short_sortedset_db.add(
                                                        order.uuid,
                                                        (order.liquidation_price * 10000.0) as i64,
                                                    );
                                                }
                                            },
                                            OrderStatus::PENDING => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ = open_long_sortedset_db.add(
                                                        order.uuid,
                                                        (order.entryprice * 10000.0) as i64,
                                                    );
                                                }
                                                PositionType::SHORT => {
                                                    let _ = open_short_sortedset_db.add(
                                                        order.uuid,
                                                        (order.entryprice * 10000.0) as i64,
                                                    );
                                                }
                                            },
                                            _ => {}
                                        }
                                        orderdb_traderorder
                                            .zkos_msg
                                            .insert(order.uuid, zkos_hex_string);
                                        let _ = orderdb_traderorder.set_order_check(order.account_id);
                                    }
                                    RpcCommand::CancelTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        let order_clone = order.clone();
                                        if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                                            orderdb_traderorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ =
                                                orderdb_traderorder.remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order_clone.entry_sequence {
                                            orderdb_traderorder.sequence = order_clone.entry_sequence;
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        queue_manager.remove_fill(&order.uuid);
                                    }
                                    RpcCommand::ExecuteTraderOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // let order_clone = order.clone();
                                        if orderdb_traderorder
                                            .ordertable
                                            .contains_key(&order.uuid.clone())
                                        {
                                            orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ =
                                                orderdb_traderorder.remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                            orderdb_traderorder.sequence = order.entry_sequence.clone();
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        match order.order_status {
                                            OrderStatus::SETTLED => match order.position_type {
                                                PositionType::LONG => {
                                                    let _ =
                                                        liquidation_long_sortedset_db.remove(order.uuid);
                                                }
                                                PositionType::SHORT => {
                                                    let _ =
                                                        liquidation_short_sortedset_db.remove(order.uuid);
                                                }
                                            },
                                            _ => {}
                                        }
                                        queue_manager.remove_settle(&order.uuid);
                                    }
                                    RpcCommand::RelayerCommandTraderOrderSettleOnLimit(
                                        _rpc_request,
                                        _metadata,
                                        _payment,
                                    ) => {
                                        let order_clone = order.clone();
                                        if orderdb_traderorder.ordertable.contains_key(&order.uuid) {
                                            orderdb_traderorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                            let _ =
                                                orderdb_traderorder.remove_order_check(order.account_id);
                                        }
                                        // orderdb_traderorder.event.push(data.value);
                                        if orderdb_traderorder.sequence < order_clone.entry_sequence {
                                            orderdb_traderorder.sequence = order_clone.entry_sequence;
                                        }
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                        queue_manager.remove_fill(&order.uuid);
                                        queue_manager.remove_settle(&order.uuid);
                                    }
                                    RpcCommand::CreateLendOrder(_, _, _, _)=>{}
                                    RpcCommand::ExecuteLendOrder(_, _, _, _)=>{}
                                    
                                },
                                Event::TraderOrderLimitUpdate(order, cmd, seq) => {
                                    match cmd {
                                        RpcCommand::ExecuteTraderOrder(  _rpc_request,
                                            _metadata,
                                            zkos_hex_string,
                                            _request_id,)=>{
                                            orderdb_traderorder
                                            .zkos_msg
                                            .insert(order.uuid, zkos_hex_string);
                                        if orderdb_traderorder.aggrigate_log_sequence < seq {
                                            orderdb_traderorder.aggrigate_log_sequence = seq;
                                        }
                                      
                                        }
                                        _=>{}
                                    }
                                }
                                Event::TraderOrderUpdate(order, _cmd, seq) => {
                                    // let order_clone = order.clone();
                                    orderdb_traderorder
                                        .ordertable
                                        .insert(order.uuid, order.clone());
                                    // orderdb_traderorder.event.push(data.value);
                                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                                    }
                                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                                        orderdb_traderorder.aggrigate_log_sequence = seq;
                                    }

                                    match order.order_status {
                                        OrderStatus::FILLED => match order.position_type {
                                            PositionType::LONG => {
                                                let _ = liquidation_long_sortedset_db.add(
                                                    order.uuid,
                                                    (order.liquidation_price * 10000.0) as i64,
                                                );
                                            }
                                            PositionType::SHORT => {
                                                let _ = liquidation_short_sortedset_db.add(
                                                    order.uuid,
                                                    (order.liquidation_price * 10000.0) as i64,
                                                );
                                            }
                                        },
                                        _ => {}
                                    }
                                    queue_manager.remove_fill(&order.uuid);
                                }
                                Event::TraderOrderFundingUpdate(order, _cmd) => {
                                    orderdb_traderorder
                                        .ordertable
                                        .insert(order.uuid, order.clone());

                                    match order.position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db.update(
                                                order.uuid,
                                                (order.liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db.update(
                                                order.uuid,
                                                (order.liquidation_price * 10000.0) as i64,
                                            );
                                        }
                                    }
                                }
                                Event::TraderOrderLiquidation(order, _cmd, seq) => {
                                    // let order_clone = order.clone();
                                    if orderdb_traderorder
                                        .ordertable
                                        .contains_key(&order.uuid.clone())
                                    {
                                        orderdb_traderorder.ordertable.remove(&order.uuid.clone());
                                        let _removed_zkos_msg =
                                            orderdb_traderorder.zkos_msg.remove(&order.uuid);
                                        let _ = orderdb_traderorder.remove_order_check(order.account_id);
                                    }
                                    // orderdb_traderorder.event.push(data.value);
                                    if orderdb_traderorder.sequence < order.entry_sequence.clone() {
                                        orderdb_traderorder.sequence = order.entry_sequence.clone();
                                    }
                                    if orderdb_traderorder.aggrigate_log_sequence < seq {
                                        orderdb_traderorder.aggrigate_log_sequence = seq;
                                    }
                                    queue_manager.remove_liquidate(&order.uuid);
                                }
                                Event::Stop(timex) => {
                                    if timex == event_stoper_string {
                                        stop_signal = false;
                                        event_offset_partition = (data.partition,data.offset);
                                    }
                                }
                                Event::LendOrder(order, cmd, seq) => match cmd {
                                    RpcCommand::CreateLendOrder(
                                        _rpc_request,
                                        _metadata,
                                        zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        let order_clone = order.clone();
                                        orderdb_lendorder.ordertable.insert(order.uuid, order);
                                        // orderdb_lendorder.event.push(data.value);
                                        if orderdb_lendorder.sequence < order_clone.entry_sequence {
                                            orderdb_lendorder.sequence = order_clone.entry_sequence;
                                        }
                                        if orderdb_lendorder.aggrigate_log_sequence < seq {
                                            orderdb_lendorder.aggrigate_log_sequence = seq;
                                        }
                                        orderdb_lendorder
                                            .zkos_msg
                                            .insert(order_clone.uuid, zkos_hex_string);
                                        let _ = orderdb_traderorder.set_order_check(order_clone.account_id);
                                    }
                                    RpcCommand::ExecuteLendOrder(
                                        _rpc_request,
                                        _metadata,
                                        _zkos_hex_string,
                                        _request_id,
                                    ) => {
                                        // orderdb_lendorder.event.push(data.value);

                                        if orderdb_lendorder.ordertable.contains_key(&order.uuid) {
                                            orderdb_lendorder.ordertable.remove(&order.uuid);
                                            let _removed_zkos_msg =
                                                orderdb_lendorder.zkos_msg.remove(&order.uuid);
                                            let _ =
                                                orderdb_traderorder.remove_order_check(order.account_id);
                                        }
                                        if orderdb_lendorder.aggrigate_log_sequence < seq {
                                            orderdb_lendorder.aggrigate_log_sequence = seq;
                                        }
                                        if orderdb_lendorder.sequence < order.entry_sequence {
                                            orderdb_lendorder.sequence = order.entry_sequence;
                                        }
                                    }
                                    _ => {}
                                },
                                Event::PoolUpdate(_cmd, lend_pool, _seq) => {
                                  
                                    if data.offset > last_offset {
                                        lendpool_database = lend_pool;
                                        last_offset = data.offset;
                                    }
                                }
                                Event::FundingRateUpdate(funding_rate, _current_price, _time) => {
                                    // set_localdb("FundingRate", funding_rate);
                                    localdb_hashmap.insert("FundingRate".to_string(), funding_rate);
                                }
                                Event::FeeUpdate(cmd, _time) => {
                                    match cmd {
                                        RelayerCommand::UpdateFees(order_filled_on_market, order_filled_on_limit, order_settled_on_market, order_settled_on_limit) => {
                                            localdb_hashmap.insert(FeeType::FilledOnMarket.into(), order_filled_on_market);
                                            localdb_hashmap.insert(FeeType::FilledOnLimit.into(), order_filled_on_limit);
                                            localdb_hashmap.insert(FeeType::SettledOnMarket.into(), order_settled_on_market);
                                            localdb_hashmap.insert(FeeType::SettledOnLimit.into(), order_settled_on_limit);
                                        }
                                        _ => {}
                                    }
                                }
                                Event::CurrentPriceUpdate(current_price, _time) => {
                                    // set_localdb("CurrentPrice", current_price);
                                    localdb_hashmap.insert("CurrentPrice".to_string(), current_price);

                                    queue_manager.bulk_insert_to_fill(&mut open_short_sortedset_db, &mut open_long_sortedset_db,  current_price);
                                    queue_manager.bulk_insert_to_liquidate(&mut liquidation_short_sortedset_db, &mut liquidation_long_sortedset_db, current_price);
                                    queue_manager.bulk_insert_to_settle(&mut close_short_sortedset_db, &mut close_long_sortedset_db, current_price);

                                }
                                Event::SortedSetDBUpdate(cmd) => match cmd {
                                    SortedSetCommand::AddOpenLimitPrice(
                                        order_id,
                                        entry_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db
                                                .add(order_id, (entry_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db
                                                .add(order_id, (entry_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::AddLiquidationPrice(
                                        order_id,
                                        liquidation_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _sortedset_ = liquidation_long_sortedset_db
                                                .add(order_id, (liquidation_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db
                                                .add(order_id, (liquidation_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::AddCloseLimitPrice(
                                        order_id,
                                        execution_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db
                                                .add(order_id, (execution_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db
                                                .add(order_id, (execution_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::RemoveOpenLimitPrice(order_id, position_type) => {
                                        match position_type {
                                            PositionType::LONG => {
                                                let _ = open_long_sortedset_db.remove(order_id);
                                            }
                                            PositionType::SHORT => {
                                                let _ = open_short_sortedset_db.remove(order_id);
                                            }
                                        }
                                    }
                                    SortedSetCommand::RemoveLiquidationPrice(order_id, position_type) => {
                                        match position_type {
                                            PositionType::LONG => {
                                                let _ = liquidation_long_sortedset_db.remove(order_id);
                                            }
                                            PositionType::SHORT => {
                                                let _ = liquidation_short_sortedset_db.remove(order_id);
                                            }
                                        }
                                    }
                                    SortedSetCommand::RemoveCloseLimitPrice(order_id, position_type) => {
                                        match position_type {
                                            PositionType::LONG => {
                                                let _ = close_long_sortedset_db.remove(order_id);
                                            }
                                            PositionType::SHORT => {
                                                let _ = close_short_sortedset_db.remove(order_id);
                                            }
                                        }
                                    }
                                    SortedSetCommand::UpdateOpenLimitPrice(
                                        order_id,
                                        entry_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = open_long_sortedset_db
                                                .update(order_id, (entry_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = open_short_sortedset_db
                                                .update(order_id, (entry_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::UpdateLiquidationPrice(
                                        order_id,
                                        liquidation_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db
                                                .update(order_id, (liquidation_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db
                                                .update(order_id, (liquidation_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::UpdateCloseLimitPrice(
                                        order_id,
                                        execution_price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = close_long_sortedset_db
                                                .update(order_id, (execution_price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db
                                                .update(order_id, (execution_price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveOpenLimitPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ =
                                                open_long_sortedset_db.search_gt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ =
                                                open_short_sortedset_db.search_lt((price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveCloseLimitPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ =
                                                close_long_sortedset_db.search_lt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = close_short_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                    },
                                    SortedSetCommand::BulkSearchRemoveLiquidationPrice(
                                        price,
                                        position_type,
                                    ) => match position_type {
                                        PositionType::LONG => {
                                            let _ = liquidation_long_sortedset_db
                                                .search_gt((price * 10000.0) as i64);
                                        }
                                        PositionType::SHORT => {
                                            let _ = liquidation_short_sortedset_db
                                                .search_lt((price * 10000.0) as i64);
                                        }
                                    },
                                },
                                Event::PositionSizeLogDBUpdate(_cmd, event) => {
                                    
                                    position_size_log = event;
                                }
                                Event::TxHash(
                                    orderid,
                                    _account_id,
                                    _tx_hash,
                                    order_type,
                                    order_status,
                                    _timestamp,
                                    option_output,
                                    _request_id,
                                ) => match order_type {
                                    OrderType::LIMIT | OrderType::MARKET => match order_status {
                                        OrderStatus::FILLED => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let output_memo_option = match option_output {
                                                Some(output_hex_string) => {
                                                    match hex::decode(output_hex_string) {
                                                        Ok(output_byte) => {
                                                            match bincode::deserialize(&output_byte) {
                                                                Ok(output_memo) => Some(output_memo),
                                                                Err(_) => None,
                                                            }
                                                        }
                                                        Err(_) => None,
                                                    }
                                                }
                                                None => None,
                                            };
                                            match output_memo_option {
                                                Some(output_memo) => {
                                                    let _ = output_hex_storage.add(
                                                        uuid_to_byte,
                                                        Some(output_memo),
                                                        0,
                                                    );
                                                }
                                                None => {}
                                            }
                                        }
                                        OrderStatus::SETTLED
                                        | OrderStatus::CANCELLED
                                        | OrderStatus::LIQUIDATE => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let _ = output_hex_storage.remove(uuid_to_byte, 0);
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                },
                                Event::TxHashUpdate(
                                    orderid,
                                    _account_id,
                                    _tx_hash,
                                    order_type,
                                    order_status,
                                    _timestamp,
                                    option_output,
                                ) => match order_type {
                                    OrderType::LIMIT | OrderType::MARKET => match order_status {
                                        OrderStatus::FILLED => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let output_memo_option = match option_output {
                                                Some(output_hex_string) => {
                                                    match hex::decode(output_hex_string) {
                                                        Ok(output_byte) => {
                                                            match bincode::deserialize(&output_byte) {
                                                                Ok(output_memo) => Some(output_memo),
                                                                Err(_) => None,
                                                            }
                                                        }
                                                        Err(_) => None,
                                                    }
                                                }
                                                None => None,
                                            };
                                            match output_memo_option {
                                                Some(output_memo) => {
                                                    let _ = output_hex_storage.add(
                                                        uuid_to_byte,
                                                        Some(output_memo),
                                                        0,
                                                    );
                                                }
                                                None => {}
                                            }
                                        }
                                        OrderStatus::SETTLED
                                        | OrderStatus::CANCELLED
                                        | OrderStatus::LIQUIDATE => {
                                            let uuid_to_byte = match bincode::serialize(&orderid) {
                                                Ok(uuid_v_u8) => uuid_v_u8,
                                                Err(_) => Vec::new(),
                                            };
                                            let _ = output_hex_storage.remove(uuid_to_byte, 0);
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                },
                                Event::AdvanceStateQueue(_,_ )=>{}
                            }
                        
                        }
                        Err(arg)=>println!("Error at kafka log receiver : {:?}",arg)
                    }
              
                }

                if lendpool_database.aggrigate_log_sequence > 0 {
                } else {
                    lendpool_database = LendPool::new();
                }
                
                queue_manager.bulk_remove_queue();

                return Ok((SnapshotDB {
                    orderdb_traderorder: orderdb_traderorder.clone(),
                    orderdb_lendorder: orderdb_lendorder.clone(),
                    lendpool_database: lendpool_database.clone(),
                    liquidation_long_sortedset_db: liquidation_long_sortedset_db.clone(),
                    liquidation_short_sortedset_db: liquidation_short_sortedset_db.clone(),
                    open_long_sortedset_db: open_long_sortedset_db.clone(),
                    open_short_sortedset_db: open_short_sortedset_db.clone(),
                    close_long_sortedset_db: close_long_sortedset_db.clone(),
                    close_short_sortedset_db: close_short_sortedset_db.clone(),
                    position_size_log: position_size_log.clone(),
                    localdb_hashmap: localdb_hashmap,
                    event_offset_partition: event_offset_partition,
                    event_timestamp: event_timestamp,
                    output_hex_storage: output_hex_storage,
                    queue_manager
                },tx_consumed));
            }
            Err(arg) => {
                println!(
                    "Failed to connect to kafka with error :{:?}\n attempt:{}",
                    arg, retry_attempt
                );
                retry_attempt += 1;
                if retry_attempt == 5 {
                    return Err(arg.to_string());
                }
                thread::sleep(std::time::Duration::from_millis(500));
            }
        };
    }

    return Err("Unable to connect to kafka".to_string());
}

pub fn load_from_snapshot()->Result<QueueState,String> {
    match snapshot() {
        Ok(snapshot_data) => {
            // let snapshot_data = SNAPSHOT_DATA.lock().unwrap();
            let mut liquidation_long_sortedset_db = TRADER_LP_LONG.lock().unwrap();
            let mut liquidation_short_sortedset_db = TRADER_LP_SHORT.lock().unwrap();
            let mut open_long_sortedset_db = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
            let mut open_short_sortedset_db = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
            let mut close_long_sortedset_db = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
            let mut close_short_sortedset_db = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
            let mut position_size_log = POSITION_SIZE_LOG.lock().unwrap();
            let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
            let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
            let mut load_pool_data = LEND_POOL_DB.lock().unwrap();
            snapshot_data.print_status();
            let mut output_hex_storage = OUTPUT_STORAGE.lock().unwrap();
            *output_hex_storage = snapshot_data.output_hex_storage;
            drop(output_hex_storage);
            // add field of Trader order db
            load_trader_data.sequence = snapshot_data.orderdb_traderorder.sequence.clone();
            load_trader_data.nonce = snapshot_data.orderdb_traderorder.nonce.clone();
            load_trader_data.aggrigate_log_sequence = snapshot_data
                .orderdb_traderorder
                .aggrigate_log_sequence
                .clone();
            load_trader_data.last_snapshot_id =
                snapshot_data.orderdb_traderorder.last_snapshot_id.clone();
            load_trader_data.zkos_msg = snapshot_data.orderdb_traderorder.zkos_msg.clone();
            load_trader_data.hash = snapshot_data.orderdb_traderorder.hash.clone();
            //end

            // add field of Lend order db
            load_lend_data.sequence = snapshot_data.orderdb_lendorder.sequence.clone();
            load_lend_data.nonce = snapshot_data.orderdb_lendorder.nonce.clone();
            load_lend_data.aggrigate_log_sequence = snapshot_data
                .orderdb_lendorder
                .aggrigate_log_sequence
                .clone();
            load_lend_data.last_snapshot_id =
                snapshot_data.orderdb_lendorder.last_snapshot_id.clone();
            load_lend_data.zkos_msg = snapshot_data.orderdb_lendorder.zkos_msg.clone();
            load_lend_data.hash = snapshot_data.orderdb_lendorder.hash.clone();
            // end
            drop(load_trader_data);
            drop(load_lend_data);
            let traderorder_hashmap = snapshot_data.orderdb_traderorder.ordertable.clone();
            let lendorder_hashmap = snapshot_data.orderdb_lendorder.ordertable.clone();

            let trader_order_handle = thread::Builder::new()
                .name(String::from("trader_order_handle"))
                .spawn(move || {
                    let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
                    for (key, val) in traderorder_hashmap.iter() {
                        load_trader_data
                            .ordertable
                            .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                    }
                })
                .unwrap();
            let lend_order_handle = thread::Builder::new()
                .name(String::from("lend_order_handle"))
                .spawn(move || {
                    let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
                    for (key, val) in lendorder_hashmap.iter() {
                        load_lend_data
                            .ordertable
                            .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                    }
                })
                .unwrap();

            *liquidation_long_sortedset_db = snapshot_data.liquidation_long_sortedset_db.clone();
            *liquidation_short_sortedset_db = snapshot_data.liquidation_short_sortedset_db.clone();
            *open_long_sortedset_db = snapshot_data.open_long_sortedset_db.clone();
            *open_short_sortedset_db = snapshot_data.open_short_sortedset_db.clone();
            *close_long_sortedset_db = snapshot_data.close_long_sortedset_db.clone();
            *close_short_sortedset_db = snapshot_data.close_short_sortedset_db.clone();
            *position_size_log = snapshot_data.position_size_log.clone();
            *load_pool_data = snapshot_data.lendpool_database.clone();
            let current_price = snapshot_data.localdb_hashmap.get("CurrentPrice").clone();
            set_localdb(
                "CurrentPrice",
                match current_price {
                    Some(value) => value.clone(),
                    None => 60000.0,
                },
            );
            let funding_rate = snapshot_data.localdb_hashmap.get("FundingRate").clone();
            set_localdb(
                "FundingRate",
                match funding_rate {
                    Some(value) => value.clone(),
                    None => 0.0,
                },
            );
            let order_filled_on_market = snapshot_data.localdb_hashmap.get::<String>(&FeeType::FilledOnMarket.into()).unwrap_or(&0.0).clone();
            let order_filled_on_limit = snapshot_data.localdb_hashmap.get::<String>(&FeeType::FilledOnLimit.into()).unwrap_or(&0.0).clone();
            let order_settled_on_market = snapshot_data.localdb_hashmap.get::<String>(&FeeType::SettledOnMarket.into()).unwrap_or(&0.0).clone();
            let order_settled_on_limit = snapshot_data.localdb_hashmap.get::<String>(&FeeType::SettledOnLimit.into()).unwrap_or(&0.0).clone();
            set_fee(FeeType::FilledOnMarket, order_filled_on_market);
            set_fee(FeeType::FilledOnLimit, order_filled_on_limit);
            set_fee(FeeType::SettledOnMarket, order_settled_on_market);
            set_fee(FeeType::SettledOnLimit, order_settled_on_limit);

            trader_order_handle.join().unwrap();
            lend_order_handle.join().unwrap();
            // println!("queue_manager:{:?}",snapshot_data.queue_manager);
            return Ok(snapshot_data.queue_manager);
        }

        Err(arg) => {
            println!("unable to load data from snapshot \n error: {:?}", arg);
            // let mut load_trader_data = TRADER_ORDER_DB.lock().unwrap();
            // let mut load_lend_data = LEND_ORDER_DB.lock().unwrap();
            // let mut load_pool_data = LEND_POOL_DB.lock().unwrap();
            // let (data1, data2, data3): (OrderDB<TraderOrder>, OrderDB<LendOrder>, LendPool) =
            //     load_backup_data();
            // *load_trader_data = data1;
            // *load_lend_data = data2;
            // *load_pool_data = data3;
            // drop(load_trader_data);
            // drop(load_lend_data);
            // drop(load_pool_data);
return Err(arg.to_string());

        }
    }
}

