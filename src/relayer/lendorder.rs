#![allow(dead_code)]
#![allow(unused_imports)]
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
extern crate uuid;
use std::thread;
use std::time::SystemTime;
use stopwatch::Stopwatch;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LendOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub balance: f64,
    pub order_status: OrderStatus, //lend or settle
    pub order_type: OrderType,     // LEND
    pub entry_nonce: usize,        // change it to u256
    pub exit_nonce: usize,         // change it to u256
    pub deposit: f64,
    pub new_lend_state_amount: f64,
    pub timestamp: String,
    pub npoolshare: f64,
    pub nwithdraw: f64,
    pub payment: f64,
    pub tlv0: f64, //total locked value before lend tx
    pub tps0: f64, // total poolshare before lend tx
    pub tlv1: f64, // total locked value after lend tx
    pub tps1: f64, // total poolshre value after lend tx
    pub tlv2: f64, // total locked value before lend payment/settlement
    pub tps2: f64, // total poolshare before lend payment/settlement
    pub tlv3: f64, // total locked value after lend payment/settlement
    pub tps3: f64, // total poolshare after lend payment/settlement
    pub entry_sequence: usize,
}

impl LendOrder {
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }
    pub fn deserialize(json: &String) -> Self {
        let deserialized: LendOrder = serde_json::from_str(json).unwrap();
        deserialized
    }

    pub fn new_order(rpc_command: CreateLendOrder, tlv0: f64, tps0: f64) -> Self {
        let account_id = rpc_command.account_id;
        let balance = rpc_command.balance;
        let order_type = rpc_command.order_type;
        let order_status = rpc_command.order_status;
        let deposit = rpc_command.deposit;
        let ndeposit = deposit;
        let npoolshare = tps0 * deposit * 10000.0 / tlv0;
        let poolshare = tps0 * deposit / tlv0;
        let tps1 = tps0.round() + poolshare.round();
        let tlv1 = tlv0.round() + ndeposit.round();
        LendOrder {
            uuid: Uuid::new_v4(),
            account_id: String::from(account_id),
            balance,
            order_status,
            order_type,
            entry_nonce: 0,
            exit_nonce: 0,
            deposit,
            new_lend_state_amount: ndeposit,
            timestamp: systemtime_to_utc(),
            npoolshare,
            nwithdraw: 0.0,
            payment: 0.0,
            tlv0,
            tps0,
            tlv1: tlv1,
            tps1: tps1,
            tlv2: 0.0,
            tps2: 0.0,
            tlv3: 0.0,
            tps3: 0.0,
            entry_sequence: 0,
        }
    }
    pub fn calculatepayment_localdb(&mut self, tlv2: f64, tps2: f64) -> Result<(), std::io::Error> {
        let nwithdraw = (tlv2 * (self.npoolshare / 10000.0).round() / tps2) * 10000.0;
        let withdraw = nwithdraw / 10000.0;
        if tlv2 < withdraw {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "insufficient pool fund!",
            ));
        }
        let payment = withdraw - self.new_lend_state_amount;
        self.nwithdraw = nwithdraw;
        let tlv3 = tlv2 - (nwithdraw / 10000.0).round();
        let tps3 = tps2 - (self.npoolshare / 10000.0).round();
        self.payment = payment;
        self.tlv2 = tlv2;
        self.tps2 = tps2;
        self.tlv3 = tlv3;
        self.tps3 = tps3;
        self.order_status = OrderStatus::SETTLED;
        self.new_lend_state_amount = withdraw;
        Ok(())
    }
}
