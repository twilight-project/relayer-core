use crate::config::{POSTGRESQL_POOL_CONNECTION, THREADPOOL};
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::sync::mpsc;
use transaction::verify_relayer::{
    verify_query_order, verify_settle_requests, verify_trade_lend_order,
};

use zkschnorr::Signature;
use zkvm::zkos_types::{Input, Output};
// use uuid::{uuid, Uuid};
use uuid::Uuid;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosQueryMsg {
    pub public_key: String, //This is Account hex address identified as public_key. Do not mistake it for public key of input
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryTraderOrderZkos {
    pub query_trader_order: QueryTraderOrder,
    pub msg: ZkosQueryMsg,
}

impl QueryTraderOrderZkos {
    pub fn verify_query(&mut self) -> Result<(), &'static str> {
        verify_query_order(
            serde_json::from_str(&self.msg.public_key.clone()).unwrap(),
            self.msg.signature.clone(),
            &bincode::serialize(&self.query_trader_order).unwrap(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryLendOrderZkos {
    pub query_lend_order: QueryLendOrder,
    pub msg: ZkosQueryMsg,
}

impl QueryLendOrderZkos {
    pub fn verify_query(&mut self) -> Result<(), &'static str> {
        verify_query_order(
            serde_json::from_str(&self.msg.public_key.clone()).unwrap(),
            self.msg.signature.clone(),
            &bincode::serialize(&self.query_lend_order).unwrap(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct QueryTraderOrder {
    pub account_id: String,
    pub order_status: OrderStatus,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct QueryLendOrder {
    pub account_id: String,
    pub order_status: OrderStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ByteRec {
    pub data: String,
}

pub fn get_traderorder_details_by_account_id(
    account: String,
) -> Result<TraderOrder, std::io::Error> {
    let threadpool = THREADPOOL.lock().unwrap();
    let account_id = account.clone();
    let (sender, receiver): (
        mpsc::Sender<Result<TraderOrder, std::io::Error>>,
        mpsc::Receiver<Result<TraderOrder, std::io::Error>>,
    ) = mpsc::channel();
    threadpool.execute(move || {
        let query = format!(
            " SELECT  uuid
	FROM public.trader_order where account_id='{}' Order By  timestamp desc Limit 1 ;",
            account
        );
        println!("query:{}", query);
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
        let mut is_raw = true;
        for row in client.query(&query, &[]).unwrap() {
            println!("is it coming here");
            let uuid_string: String = row.get("uuid");
            let uuid = Uuid::parse_str(&uuid_string).unwrap();
            println!("raw data:{:#?}", uuid);
            let mut trader_order_db = TRADER_ORDER_DB.lock().unwrap();
            let trader_order = trader_order_db.get(uuid);
            drop(trader_order_db);
            sender.send(trader_order).unwrap();
            is_raw = false;
        }
        if is_raw {
            sender.send(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("order not found id:{}", account),
            )));
        }
    });

    match receiver.recv().unwrap() {
        Ok(value) => {
            println!("is it coming here3");
            return Ok(value);
        }
        Err(arg) => {
            println!("is it coming here4");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("order not found id:{}", account_id),
            ));
        }
    };
}
pub fn get_lendorder_details_by_account_id(account: String) -> Result<LendOrder, std::io::Error> {
    let threadpool = THREADPOOL.lock().unwrap();

    let (sender, receiver): (
        mpsc::Sender<Result<LendOrder, std::io::Error>>,
        mpsc::Receiver<Result<LendOrder, std::io::Error>>,
    ) = mpsc::channel();
    threadpool.execute(move || {
        let query = format!(
            " SELECT  uuid
	FROM  public.lend_order where account_id='{}' Order By  timestamp desc Limit 1 ;",
            account
        );
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
        let mut is_raw = true;
        for row in client.query(&query, &[]).unwrap() {
            let uuid_string: String = row.get("uuid");
            let uuid = Uuid::parse_str(&uuid_string).unwrap();
            println!("raw data:{:#?}", uuid);
            let mut lend_order_db = LEND_ORDER_DB.lock().unwrap();
            let lend_order = lend_order_db.get(uuid);
            drop(lend_order_db);
            sender.send(lend_order).unwrap();
            is_raw = false;
        }
        if is_raw {
            sender.send(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "order not found",
            )));
        }
    });

    match receiver.recv().unwrap() {
        Ok(value) => {
            println!("is it coming here3");
            return Ok(value);
        }
        Err(arg) => {
            println!("is it coming here4");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "order not found",
            ));
        }
    };
}
