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
use relayerwalletlib::zkoswalletlib::util::get_state_info_from_output_hex;
use serde::Deserialize as DeserializeAs;
use serde::Serialize as SerializeAs;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use utxo_in_memory::db::LocalDBtrait;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerState {
    pub map: HashMap<Nonce, zkvm::Output>,
    pub vec_deque: VecDeque<Nonce>,
}
impl RelayerState {
    pub fn new(size: usize) -> Self {
        RelayerState {
            map: HashMap::with_capacity(size),
            vec_deque: VecDeque::with_capacity(size),
        }
    }
    pub fn insert(&mut self, nonce: Nonce, output: zkvm::Output) -> Result<(), String> {
        let _ = self.map.insert(nonce, output);

        // Track insertion order
        self.vec_deque.push_back(nonce);

        // If HashMap exceeds capacity
        if self.map.len() > self.map.capacity() {
            // Remove the oldest element
            if let Some(oldest_key) = self.vec_deque.pop_front() {
                self.map.remove(&oldest_key);
            }
        }
        Ok(())
    }
    pub fn get_nonce(&mut self, nonce: Nonce) -> Option<&zkvm::Output> {
        self.map.get(&nonce)
    }
    pub fn update_relayer_state_in_lendpool(&mut self) {
        let mut lendpool = LEND_POOL_DB.lock().unwrap();

        let account_id = lendpool
            .last_output_state
            .clone()
            .as_output_data()
            .get_owner_address()
            .clone()
            .unwrap()
            .clone();

        let nonce = lendpool.nonce;
        let mut latest_nonce = 0;

        match relayerwalletlib::zkoswalletlib::chain::get_utxo_details_by_address(
            account_id.clone(),
            zkvm::IOType::State,
        ) {
            Ok(utxo_detail) => {
                match utxo_detail.output.as_out_state() {
                    Some(state) => {
                        latest_nonce = state.nonce.clone() as usize;
                        // flag_chain_update = false;
                    }
                    None => {}
                }
            }
            Err(arg) => {
                // flag_chain_update = false;
                crate::log_heartbeat!(
                    error,
                    "Failed to get latest state from chain \n Error:{:?}",
                    arg
                );
            }
        }
        if nonce == latest_nonce {
            // flag_chain_update = false;
            return;
        } else {
            lendpool.last_output_state = match self.get_nonce(latest_nonce) {
                Some(output) => output.clone(),
                None => return,
            };

            match get_state_info_from_output_hex(hex::encode(
                bincode::serialize(&lendpool.last_output_state).unwrap(),
            )) {
                Ok((nonce, tlv_witness, _tlv_blinding, tps_witness, _tps_blinding)) => {
                    lendpool.total_locked_value = tlv_witness as f64;
                    lendpool.total_pool_share = tps_witness as f64;
                    lendpool.nonce = nonce as usize;
                }
                Err(_arg) => return,
            };
        }
    }
}

pub fn create_relayer_state_data() -> Result<
    (
        RelayerState,
        crossbeam_channel::Sender<(i32, i64)>,
        OffsetCompletion,
    ),
    String,
> {
    let mut event_offset_partition: OffsetCompletion = (0, 0);
    let mut relayer_state_db = RelayerState::new(10);
    let time = ServerTime::now().epoch;
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    Event::send_event_to_kafka_queue(
        eventstop.clone(),
        RELAYER_STATE_QUEUE.clone().to_string(),
        String::from("StopLoadMSG"),
    );
    let mut retry_attempt = 0;
    while retry_attempt < 5 {
        match Event::receive_event_for_snapshot_from_kafka_queue(
            RELAYER_STATE_QUEUE.clone().to_string(),
            format!(
                "{}-{}-state",
                *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
            ),
            FetchOffset::Earliest,
            "relayer state handle",
        ) {
            Ok((receiver_lock, tx_consumed)) => {
                let recever1 =
                // receiver_lock.lock().unwrap();
               match receiver_lock.lock(){
                Ok(rec_lock)=>{rec_lock}
                Err(arg)=>{
                    retry_attempt+=1;
                    crate::log_heartbeat!(error, "unable to lock the kafka log receiver :{:?}",arg);
                    continue;
                }
               };
                loop {
                    match recever1.recv() {
                        Ok(data) => match data.value.clone() {
                            Event::Stop(timex) => {
                                if timex == event_stoper_string {
                                    crate::log_heartbeat!(
                                        debug,
                                        "exiting from consumer as received stop cmd"
                                    );
                                    break;
                                }
                            }
                            Event::AdvanceStateQueue(_nonce, output) => {
                                let _ = relayer_state_db.insert(
                                    match output.as_out_state() {
                                        Some(state) => state.nonce.clone() as usize,
                                        None => 1,
                                    },
                                    output,
                                );
                                event_offset_partition = (data.partition, data.offset);
                            }
                            _ => {}
                        },
                        Err(arg) => {
                            crate::log_heartbeat!(error, "Error at kafka log receiver : {:?}", arg)
                        }
                    }
                }

                return Ok((relayer_state_db, tx_consumed, event_offset_partition));
            }
            Err(arg) => {
                crate::log_heartbeat!(
                    error,
                    "Failed to connect to kafka with error :{:?}\n attempt:{}",
                    arg,
                    retry_attempt
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

pub fn load_relayer_latest_state() {
    match create_relayer_state_data() {
        Ok((mut relayer_state, tx_consumed, (partition, mut offset))) => {
            relayer_state.update_relayer_state_in_lendpool();
            if offset > 10 {
                offset -= 10;
            }
            match tx_consumed.send((partition, offset)) {
                Ok(_) => {}
                Err(arg) => {
                    crate::log_heartbeat!(error, "unable to send completion offset :{:?}", arg);
                }
            }
        }
        Err(arg) => crate::log_heartbeat!(
            error,
            "unable to update relayer latest update due to Error:{:?}",
            arg
        ),
    }
}
