use crate::db::Event;
use crate::config::*;
use std::thread::sleep;
use std::time::Duration;
use twilight_relayer_sdk::transaction::Transaction;
use twilight_relayer_sdk::zkvm::IOType;
use twilight_relayer_sdk::zkvm::Output;

pub fn transaction_queue_to_confirm_relayer_latest_state(
    last_output: Output,
    tx: Transaction,
    next_output: Output
) -> Result<String, String> {
    let nonce = match last_output.as_out_state() {
        Some(state) => state.nonce.clone(),
        None => 1,
    };

    let account_id = last_output.as_output_data().get_owner_address().clone().unwrap().clone();

    let mut flag_chain_update = true;
    let mut latest_nonce: u32 = 0;
    let mut chain_attempt: i32 = 0;
    while flag_chain_update {
        // let updated_output_on_chain =
        match
            twilight_relayer_sdk::twilight_client_sdk::chain::get_utxo_details_by_address(
                account_id.clone(),
                IOType::State
            )
        {
            Ok(utxo_detail) => {
                match utxo_detail.output.as_out_state() {
                    Some(state) => {
                        latest_nonce = state.nonce.clone();
                        // flag_chain_update = false;
                    }
                    None => {}
                }
            }
            Err(arg) => {
                chain_attempt += 1;
                sleep(Duration::from_millis(200));
                if chain_attempt == 100 {
                    // flag_chain_update = false;
                    return Err("Tx Failed due to missing latest state update".to_string());
                }
            }
        }
        if nonce == latest_nonce {
            // flag_chain_update = false;
            crate::log_zkos_tx!(debug, "tx: {:?}", tx);
            match
                twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(
                    tx.clone()
                )
            {
                Ok(tx_hash) => {
                    Event::new(
                        Event::AdvanceStateQueue((nonce + 1) as usize, next_output),
                        format!("Nonce : {:?}", nonce + 1),
                        RELAYER_STATE_QUEUE.clone().to_string()
                    );
                    crate::log_zkos_tx!(
                        debug,
                        "transaction broadcasted successfully, tx_hash: {:?}",
                        tx_hash
                    );
                    return Ok(tx_hash);
                }
                Err(arg) => {
                    if arg.contains("Failed to verify the Input Utxo") {
                        crate::log_zkos_tx!(debug, "Error: Failed to verify the Input Utxo");
                        crate::log_zkos_tx!(debug, "retrying tx after 2 seconds");
                        sleep(Duration::from_secs(2));
                        let check_verify = tx.verify();
                        if check_verify.is_ok() {
                            crate::log_zkos_tx!(debug, "tx verified successfully");
                        } else {
                            crate::log_zkos_tx!(
                                debug,
                                "tx not verified, error: {:?}",
                                check_verify
                            );
                        }
                        match
                            twilight_relayer_sdk::twilight_client_sdk::chain::tx_commit_broadcast_transaction(
                                tx
                            )
                        {
                            Ok(tx_hash) => {
                                Event::new(
                                    Event::AdvanceStateQueue((nonce + 1) as usize, next_output),
                                    format!("Nonce : {:?}", nonce + 1),
                                    RELAYER_STATE_QUEUE.clone().to_string()
                                );
                                crate::log_zkos_tx!(
                                    debug,
                                    "transaction broadcasted successfully after 2 seconds, tx_hash: {:?}",
                                    tx_hash
                                );
                                // sleep(Duration::from_secs(1));
                                return Ok(tx_hash);
                            }
                            Err(arg) => {
                                crate::log_zkos_tx!(
                                    debug,
                                    "Failed to broadcast retried tx after 2 seconds, error: {:?}",
                                    arg
                                );
                                return Err(arg);
                            }
                        }
                    }
                    crate::log_zkos_tx!(debug, "Failed to broadcast tx, error: {:?}", arg);
                    return Err(arg);
                }
            }
        } else {
            flag_chain_update = true;
            chain_attempt += 1;
            sleep(Duration::from_millis(200));
            if chain_attempt == 100 {
                // flag_chain_update = false;
                return Err("Tx Failed due to missing latest state update".to_string());
            }
        }
    }
    return Err("Tx Failed due to missing latest state update".to_string());
}

pub fn is_state_updated(last_output: &Output) -> bool {
    let nonce = match last_output.as_out_state() {
        Some(state) => state.nonce,
        None => 1,
    };

    let account_id = match last_output.as_output_data().get_owner_address() {
        Some(owner) => owner.clone(),
        None => {
            return false;
        }
    };

    let mut latest_nonce: u32 = 0;
    let mut chain_attempt: i32 = 0;
    loop {
        // let updated_output_on_chain =
        match
            twilight_relayer_sdk::twilight_client_sdk::chain::get_utxo_details_by_address(
                account_id.clone(),
                IOType::State
            )
        {
            Ok(utxo_detail) => {
                match utxo_detail.output.as_out_state() {
                    Some(state) => {
                        latest_nonce = state.nonce.clone();
                        // flag_chain_update = false;
                    }
                    None => {}
                }
            }
            Err(arg) => {
                chain_attempt += 1;

                if chain_attempt == 150 {
                    // flag_chain_update = false;
                    return false;
                }
                sleep(Duration::from_millis(200));
            }
        }
        if nonce == latest_nonce {
            return true;
        } else {
            chain_attempt += 1;

            if chain_attempt == 150 {
                // flag_chain_update = false;
                return false;
            }
            sleep(Duration::from_millis(200));
        }
    }
}
