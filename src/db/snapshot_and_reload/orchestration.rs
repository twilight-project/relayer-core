use crc32fast::Hasher as Crc32Hasher;
use kafka::consumer::FetchOffset;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::config::*;
use crate::db::*;
use crate::relayer::*;

use super::codec::{decode_legacy_snapshot, decode_versioned_snapshot, write_snapshot_atomically};
use super::current::{SnapshotBuilder, SnapshotDB, SNAPSHOT_FORMAT_VERSION};
use super::lock_or_err;

pub fn snapshot() -> Result<SnapshotDB, String> {
    crate::log_heartbeat!(info, "started taking snapshot");
    let read_snapshot = fs::read(format!(
        "{}-{}",
        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
    ));
    let decoded_snapshot: SnapshotDB;
    let fetchoffset: FetchOffset;
    match read_snapshot {
        Ok(snapshot_data_from_file) => {
            decoded_snapshot = match decode_versioned_snapshot(&snapshot_data_from_file) {
                Ok(Some(snap)) => {
                    crate::log_heartbeat!(
                        info,
                        "Loaded versioned:{} snapshot successfully",
                        SNAPSHOT_FORMAT_VERSION
                    );
                    fetchoffset = FetchOffset::Earliest;
                    snap
                }
                Ok(None) => {
                    crate::log_heartbeat!(
                        info,
                        "No version header detected, trying legacy deserialization"
                    );
                    match decode_legacy_snapshot(&snapshot_data_from_file) {
                        Ok(snap) => {
                            fetchoffset = FetchOffset::Earliest;
                            snap
                        }
                        Err(e) => {
                            if *ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE {
                                crate::log_heartbeat!(
                                    warn,
                                    "All snapshot decode attempts failed. \
                                     ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true, \
                                     creating new empty snapshot. Error: {} \n path: {:?}",
                                    e,
                                    format!(
                                        "{}-{}",
                                        *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                                    )
                                );
                                fetchoffset = FetchOffset::Earliest;
                                SnapshotDB::new()
                            } else {
                                return Err(format!(
                                    "Snapshot file exists but could not be decoded by any version (V7-V1). \
                                     Set ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true to discard and start fresh. \
                                     Error: {}, path: {}-{}",
                                    e, *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    if *ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE {
                        crate::log_heartbeat!(
                            warn,
                            "Versioned snapshot decode failed: {}. \
                             ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true, creating new empty snapshot.",
                            e
                        );
                        fetchoffset = FetchOffset::Earliest;
                        SnapshotDB::new()
                    } else {
                        return Err(format!(
                            "Versioned snapshot decode failed: {}. \
                             Set ALLOW_NEW_SNAPSHOT_ON_DECODE_FAILURE=true to discard and start fresh. \
                             path: {}-{}",
                            e, *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
                        ));
                    }
                }
            };
        }
        Err(arg) => {
            crate::log_heartbeat!(
                info,
                "No previous Snapshot Found- Error:{:#?} \n path: {:?} \n Creating new Snapshot",
                arg,
                format!("{}-{}", *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION)
            );
            decoded_snapshot = SnapshotDB::new();
            fetchoffset = FetchOffset::Earliest;
        }
    }

    let (snapshot_db_updated, tx_consumed) =
        match create_snapshot_data(fetchoffset, decoded_snapshot) {
            Ok(snap_data) => snap_data,
            Err(arg) => {
                return Err(arg.to_string());
            }
        };

    // Serialize with version header + CRC32 + zstd compression
    let raw_payload = match bincode::serialize(&snapshot_db_updated) {
        Ok(v) => v,
        Err(arg) => {
            crate::log_heartbeat!(error, "Could not encode snapshot data - Error:{:#?}", arg);
            return Err(arg.to_string());
        }
    };

    let compressed_payload = match zstd::encode_all(Cursor::new(&raw_payload), 3) {
        Ok(v) => v,
        Err(arg) => {
            crate::log_heartbeat!(error, "zstd compression failed: {:?}", arg);
            return Err(arg.to_string());
        }
    };

    crate::log_heartbeat!(
        info,
        "Snapshot size: raw={} bytes, compressed={} bytes ({:.1}x)",
        raw_payload.len(),
        compressed_payload.len(),
        raw_payload.len() as f64 / compressed_payload.len().max(1) as f64
    );

    let mut hasher = Crc32Hasher::new();
    hasher.update(&compressed_payload);
    let crc = hasher.finalize();

    let mut encoded_v = Vec::with_capacity(8 + compressed_payload.len());
    encoded_v.extend_from_slice(&SNAPSHOT_FORMAT_VERSION.to_le_bytes());
    encoded_v.extend_from_slice(&crc.to_le_bytes());
    encoded_v.extend_from_slice(&compressed_payload);

    write_snapshot_atomically(
        &encoded_v,
        &PathBuf::from(format!(
            "{}-{}",
            *RELAYER_SNAPSHOT_FILE_LOCATION, *SNAPSHOT_VERSION
        )),
        &tx_consumed,
        snapshot_db_updated.event_offset_partition,
    )
    .map_err(|e| {
        crate::log_heartbeat!(error, "snapshot write failed: {:?}", e);
        e.to_string()
    })?;

    crate::log_heartbeat!(info, "Snapshot Done");
    snapshot_db_updated.log_metadata();
    Ok(snapshot_db_updated)
}

pub fn create_snapshot_data(
    fetchoffset: FetchOffset,
    snapshot_db: SnapshotDB,
) -> Result<(SnapshotDB, crossbeam_channel::Sender<(i32, i64)>), String> {
    let time = ServerTime::now().epoch;
    let event_timestamp = time.clone();
    let event_stoper_string = format!("snapsot-start-{}", time);
    let eventstop: Event = Event::Stop(event_stoper_string.clone());
    let _ = Event::send_event_to_kafka_queue(
        eventstop.clone(),
        CORE_EVENT_LOG.clone().to_string(),
        String::from("StopLoadMSG"),
    );

    let mut builder = SnapshotBuilder::from_snapshot(snapshot_db, event_timestamp);

    let saved_offset = builder.event_offset_partition.1;

    let mut stop_signal: bool = true;
    let mut retry_attempt = 0;
    while retry_attempt < 10 {
        match Event::receive_event_for_snapshot_from_kafka_queue(
            CORE_EVENT_LOG.clone().to_string(),
            builder.group_name.clone(),
            fetchoffset,
            "snapshot handle",
        ) {
            Ok((receiver_lock, tx_consumed)) => {
                let recever1 = match receiver_lock.lock() {
                    Ok(rec_lock) => rec_lock,
                    Err(arg) => {
                        retry_attempt += 1;
                        crate::log_heartbeat!(
                            error,
                            "unable to lock the kafka log receiver :{:?}",
                            arg
                        );
                        continue;
                    }
                };
                let mut last_offset = 0;
                let mut kafka_reconnect_attempt = 0;
                while stop_signal {
                    match recever1.recv() {
                        Ok(data) => {
                            if let Event::Stop(ref timex) = data.value {
                                if *timex == event_stoper_string {
                                    stop_signal = false;
                                    builder.event_offset_partition = (data.partition, data.offset);
                                    continue;
                                }
                            }

                            if saved_offset > 0 && data.offset <= saved_offset {
                                continue;
                            }

                            if let Event::PoolUpdate(_, _, _) = &data.value {
                                if data.offset > last_offset {
                                    last_offset = data.offset;
                                    builder.handle_event(&data);
                                }
                            } else {
                                builder.handle_event(&data);
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(error, "Error at kafka log receiver : {:?}", arg);
                            kafka_reconnect_attempt += 1;
                            if kafka_reconnect_attempt > 20 {
                                return Err(arg.to_string());
                            }
                            thread::sleep(std::time::Duration::from_millis(1000));
                        }
                    }
                }

                return Ok((builder.build(), tx_consumed));
            }
            Err(arg) => {
                crate::log_heartbeat!(
                    error,
                    "Failed to connect to kafka with error :{:?}\n attempt:{}",
                    arg,
                    retry_attempt
                );
                if retry_attempt == 10 {
                    return Err(arg.to_string());
                }
                retry_attempt += 1;
                thread::sleep(std::time::Duration::from_millis(1000));
            }
        };
    }

    return Err("Unable to connect to kafka".to_string());
}

pub fn load_from_snapshot() -> Result<QueueState, String> {
    match snapshot() {
        Ok(snapshot_data) => {
            let mut liquidation_long_sortedset_db = lock_or_err!(TRADER_LP_LONG, "TRADER_LP_LONG");
            let mut liquidation_short_sortedset_db =
                lock_or_err!(TRADER_LP_SHORT, "TRADER_LP_SHORT");
            let mut open_long_sortedset_db =
                lock_or_err!(TRADER_LIMIT_OPEN_LONG, "TRADER_LIMIT_OPEN_LONG");
            let mut open_short_sortedset_db =
                lock_or_err!(TRADER_LIMIT_OPEN_SHORT, "TRADER_LIMIT_OPEN_SHORT");
            let mut close_long_sortedset_db =
                lock_or_err!(TRADER_LIMIT_CLOSE_LONG, "TRADER_LIMIT_CLOSE_LONG");
            let mut close_short_sortedset_db =
                lock_or_err!(TRADER_LIMIT_CLOSE_SHORT, "TRADER_LIMIT_CLOSE_SHORT");
            let mut sl_close_long_sortedset_db =
                lock_or_err!(TRADER_SL_CLOSE_LONG, "TRADER_SL_CLOSE_LONG");
            let mut sl_close_short_sortedset_db =
                lock_or_err!(TRADER_SL_CLOSE_SHORT, "TRADER_SL_CLOSE_SHORT");
            let mut tp_close_long_sortedset_db =
                lock_or_err!(TRADER_TP_CLOSE_LONG, "TRADER_TP_CLOSE_LONG");
            let mut tp_close_short_sortedset_db =
                lock_or_err!(TRADER_TP_CLOSE_SHORT, "TRADER_TP_CLOSE_SHORT");
            let mut position_size_log = lock_or_err!(POSITION_SIZE_LOG, "POSITION_SIZE_LOG");
            let mut load_trader_data = lock_or_err!(TRADER_ORDER_DB, "TRADER_ORDER_DB");
            let mut load_lend_data = lock_or_err!(LEND_ORDER_DB, "LEND_ORDER_DB");
            let mut load_pool_data = lock_or_err!(LEND_POOL_DB, "LEND_POOL_DB");
            let mut output_hex_storage = lock_or_err!(OUTPUT_STORAGE, "OUTPUT_STORAGE");

            snapshot_data.print_status();

            *output_hex_storage = snapshot_data.output_hex_storage;
            drop(output_hex_storage);

            load_trader_data.sequence = snapshot_data.orderdb_traderorder.sequence;
            load_trader_data.nonce = snapshot_data.orderdb_traderorder.nonce;
            load_trader_data.aggrigate_log_sequence =
                snapshot_data.orderdb_traderorder.aggrigate_log_sequence;
            load_trader_data.last_snapshot_id = snapshot_data.orderdb_traderorder.last_snapshot_id;
            load_trader_data.zkos_msg = snapshot_data.orderdb_traderorder.zkos_msg.clone();
            load_trader_data.hash = snapshot_data.orderdb_traderorder.hash.clone();

            load_lend_data.sequence = snapshot_data.orderdb_lendorder.sequence;
            load_lend_data.nonce = snapshot_data.orderdb_lendorder.nonce;
            load_lend_data.aggrigate_log_sequence =
                snapshot_data.orderdb_lendorder.aggrigate_log_sequence;
            load_lend_data.last_snapshot_id = snapshot_data.orderdb_lendorder.last_snapshot_id;
            load_lend_data.zkos_msg = snapshot_data.orderdb_lendorder.zkos_msg.clone();
            load_lend_data.hash = snapshot_data.orderdb_lendorder.hash.clone();
            drop(load_trader_data);
            drop(load_lend_data);
            let traderorder_hashmap = snapshot_data.orderdb_traderorder.ordertable.clone();
            let lendorder_hashmap = snapshot_data.orderdb_lendorder.ordertable.clone();

            let trader_order_handle = match thread::Builder::new()
                .name(String::from("trader_order_handle"))
                .spawn(move || {
                    match TRADER_ORDER_DB.lock() {
                        Ok(mut load_trader_data) => {
                            for (key, val) in traderorder_hashmap.iter() {
                                load_trader_data
                                    .ordertable
                                    .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(
                                error,
                                "Error locking TRADER_ORDER_DB : {:?}",
                                arg
                            );
                        }
                    };
                }) {
                Ok(handle) => handle,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error creating trader_order_handle : {:?}", arg);
                    return Err(arg.to_string());
                }
            };
            let lend_order_handle = match thread::Builder::new()
                .name(String::from("lend_order_handle"))
                .spawn(move || {
                    match LEND_ORDER_DB.lock() {
                        Ok(mut load_lend_data) => {
                            for (key, val) in lendorder_hashmap.iter() {
                                load_lend_data
                                    .ordertable
                                    .insert(key.clone(), Arc::new(RwLock::new(val.clone())));
                            }
                        }
                        Err(arg) => {
                            crate::log_heartbeat!(error, "Error locking LEND_ORDER_DB : {:?}", arg);
                        }
                    };
                }) {
                Ok(handle) => handle,
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error creating lend_order_handle : {:?}", arg);
                    return Err(arg.to_string());
                }
            };

            *liquidation_long_sortedset_db = snapshot_data.liquidation_long_sortedset_db.clone();
            *liquidation_short_sortedset_db = snapshot_data.liquidation_short_sortedset_db.clone();
            *open_long_sortedset_db = snapshot_data.open_long_sortedset_db.clone();
            *open_short_sortedset_db = snapshot_data.open_short_sortedset_db.clone();
            *close_long_sortedset_db = snapshot_data.close_long_sortedset_db.clone();
            *close_short_sortedset_db = snapshot_data.close_short_sortedset_db.clone();
            *sl_close_long_sortedset_db = snapshot_data.sl_close_long_sortedset_db.clone();
            *sl_close_short_sortedset_db = snapshot_data.sl_close_short_sortedset_db.clone();
            *tp_close_long_sortedset_db = snapshot_data.tp_close_long_sortedset_db.clone();
            *tp_close_short_sortedset_db = snapshot_data.tp_close_short_sortedset_db.clone();
            *position_size_log = snapshot_data.position_size_log.clone();
            {
                let mut risk_engine_state = RISK_ENGINE_STATE.lock().unwrap();
                *risk_engine_state = snapshot_data.risk_state.clone();
                drop(risk_engine_state);
            }
            if let Some(ref params) = snapshot_data.risk_params {
                let mut risk_params = RISK_PARAMS.lock().unwrap();
                *risk_params = params.clone();
                drop(risk_params);
                crate::log_heartbeat!(info, "RISK_ENGINE: Restored risk params from snapshot");
            }
            *load_pool_data = snapshot_data.lendpool_database.clone();
            let current_price = snapshot_data.localdb_hashmap.get("CurrentPrice");
            set_localdb(
                "CurrentPrice",
                match current_price {
                    Some(value) => *value,
                    None => 60000.0,
                },
            );
            let funding_rate = snapshot_data.localdb_hashmap.get("FundingRate");
            set_localdb(
                "FundingRate",
                match funding_rate {
                    Some(value) => *value,
                    None => 0.0,
                },
            );
            let order_filled_on_market = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnMarket.into())
                .unwrap_or(&0.0);
            let order_filled_on_limit = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::FilledOnLimit.into())
                .unwrap_or(&0.0);
            let order_settled_on_market = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnMarket.into())
                .unwrap_or(&0.0);
            let order_settled_on_limit = *snapshot_data
                .localdb_hashmap
                .get::<String>(&FeeType::SettledOnLimit.into())
                .unwrap_or(&0.0);
            set_fee(FeeType::FilledOnMarket, order_filled_on_market);
            set_fee(FeeType::FilledOnLimit, order_filled_on_limit);
            set_fee(FeeType::SettledOnMarket, order_settled_on_market);
            set_fee(FeeType::SettledOnLimit, order_settled_on_limit);

            match trader_order_handle.join() {
                Ok(_) => {}
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error at trader_order_handle : {:?}", arg);
                }
            }

            match lend_order_handle.join() {
                Ok(_) => {}
                Err(arg) => {
                    crate::log_heartbeat!(error, "Error at lend_order_handle : {:?}", arg);
                }
            }
            return Ok(snapshot_data.queue_manager);
        }

        Err(arg) => {
            crate::log_heartbeat!(
                error,
                "unable to load data from snapshot \n error: {:?}",
                arg
            );
            return Err(arg.to_string());
        }
    }
}
