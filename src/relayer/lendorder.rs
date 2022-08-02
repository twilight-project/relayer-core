#![allow(dead_code)]
#![allow(unused_imports)]
use crate::relayer::types::*;
use crate::relayer::utils::*;
use crate::relayer::ServerTime;
use serde_derive::{Deserialize, Serialize};
extern crate uuid;
use crate::config::QUERYSTATUS;
use crate::config::{POSTGRESQL_POOL_CONNECTION, THREADPOOL_PSQL_ORDER_INSERT_QUEUE};
use crate::redislib::redis_db;
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
    pub timestamp: SystemTime,
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
    pub fn new(
        account_id: &str,
        balance: f64,
        order_type: OrderType,
        order_status: OrderStatus,
        deposit: f64,
    ) -> Self {
        let ndeposit = deposit * 10000.0;
        // // let nonce = redis_db::incr_lend_nonce_by_one();
        // // println!("Nonce : {}", nonce);

        // let mut tlv = redis_db::get_type_f64("tlv");
        // if tlv == 0.0 {
        //     tlv = initialize_lend_pool(100000.0, 10.0);
        // }
        // let tps = redis_db::get_type_f64("tps");
        // // println!("tps check {:#?}", tps);
        // let npoolshare = normalize_pool_share(tlv, tps, ndeposit);
        // let tps1 = redis_db::incrbyfloat_type_f64("tps", npoolshare / 10000.0);
        // let tlv1 = redis_db::incrbyfloat_type_f64("tlv", ndeposit);
        // let entry_nonce = redis_db::incr_lend_nonce_by_one();
        // let entry_sequence = redis_db::incr_entry_sequence_by_one_lend_order();

        let (tlv0, tps0, tlv1, tps1, poolshare, npoolshare, entry_nonce, entry_sequence): (
            f64,
            f64,
            f64,
            f64,
            f64,
            f64,
            usize,
            usize,
        ) = getset_new_lend_order_tlv_tps_poolshare(deposit);

        LendOrder {
            uuid: Uuid::new_v4(),
            account_id: String::from(account_id),
            balance,
            order_status,
            order_type,
            entry_nonce,
            exit_nonce: 0,
            deposit,
            new_lend_state_amount: ndeposit,
            timestamp: SystemTime::now(),
            npoolshare,
            nwithdraw: 0.0,
            payment: 0.0,
            tlv0,
            tps0,
            tlv1,
            tps1,
            tlv2: 0.0,
            tps2: 0.0,
            tlv3: 0.0,
            tps3: 0.0,
            entry_sequence,
        }
    }
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }
    pub fn deserialize(json: &String) -> Self {
        let deserialized: LendOrder = serde_json::from_str(json).unwrap();
        deserialized
    }

    pub fn newlendorderinsert(self) -> LendOrder {
        let lendtx = self.clone();

        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        let return_self = lendtx.clone();
        let lendtx_psql = lendtx.clone();
        thread::spawn(move || {
            // Lend order saved in redis, orderid as key
            redis_db::set(&lendtx.uuid.to_string(), &lendtx.serialize());
            // Lend order set by nonce
            redis_db::zadd(
                &"LendOrder",
                &lendtx.uuid.to_string(),           //value
                &lendtx.entry_sequence.to_string(), //score
            );

            // Lend order by there deposit amount as score

            redis_db::zadd(
                &"LendOrderbyDepositLendState",
                &lendtx.uuid.to_string(),                  //value
                &lendtx.new_lend_state_amount.to_string(), //score
            );

            redis_db::incrbyfloat(
                &"TotalLendPoolSize",
                &lendtx.new_lend_state_amount.to_string(),
            );
        });
        // let sw = Stopwatch::start_new();

        // thread to store Lend order data in postgreSQL
        let pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
        pool.execute(move || {
            let query = format!("INSERT INTO public.newlendorder(
                uuid, account_id, balance, order_status, order_type, entry_nonce,exit_nonce, deposit, new_lend_state_amount, timestamp, npoolshare, nwithdraw, payment, tlv0, tps0, tlv1, tps1, tlv2, tps2, tlv3, tps3, entry_sequence)
                VALUES ('{}','{}',{},'{:#?}','{:#?}',{},{},{},{},'{}',{},{},{},{},{},{},{},{},{},{},{},{});",
                &lendtx_psql.uuid,
                &lendtx_psql.account_id ,
                &lendtx_psql.balance ,
                &lendtx_psql.order_status ,
                &lendtx_psql.order_type ,
                &lendtx_psql.entry_nonce ,
                &lendtx_psql.exit_nonce ,
                &lendtx_psql.deposit ,
                &lendtx_psql.new_lend_state_amount ,
                ServerTime::new(lendtx_psql.timestamp).iso,
                &lendtx_psql.npoolshare ,
                &lendtx_psql.nwithdraw ,
                &lendtx_psql.payment ,
                &lendtx_psql.tlv0 ,
                &lendtx_psql.tps0 ,
                &lendtx_psql.tlv1 ,
                &lendtx_psql.tps1 ,
                &lendtx_psql.tlv2 ,
                &lendtx_psql.tps2 ,
                &lendtx_psql.tlv3 ,
                &lendtx_psql.tps3 ,
                &lendtx_psql.entry_sequence ,
            );
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            println!("psql done");
        });
        drop(pool);
        // println!("pool took {:#?}", sw.elapsed());
        // let handle = thread::spawn(move || {
        //     let query = format!("INSERT INTO public.newlendorder(
        //         uuid, account_id, balance, order_status, order_type, entry_nonce,exit_nonce, deposit, new_lend_state_amount, timestamp, npoolshare, nwithdraw, payment, tlv0, tps0, tlv1, tps1, tlv2, tps2, tlv3, tps3, entry_sequence)
        //         VALUES ('{}','{}',{},'{:#?}','{:#?}',{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{});",
        //         &lendtx_psql.uuid,
        //         &lendtx_psql.account_id ,
        //         &lendtx_psql.balance ,
        //         &lendtx_psql.order_status ,
        //         &lendtx_psql.order_type ,
        //         &lendtx_psql.entry_nonce ,
        //         &lendtx_psql.exit_nonce ,
        //         &lendtx_psql.deposit ,
        //         &lendtx_psql.new_lend_state_amount ,
        //         &lendtx_psql.timestamp ,
        //         &lendtx_psql.npoolshare ,
        //         &lendtx_psql.nwithdraw ,
        //         &lendtx_psql.payment ,
        //         &lendtx_psql.tlv0 ,
        //         &lendtx_psql.tps0 ,
        //         &lendtx_psql.tlv1 ,
        //         &lendtx_psql.tps1 ,
        //         &lendtx_psql.tlv2 ,
        //         &lendtx_psql.tps2 ,
        //         &lendtx_psql.tlv3 ,
        //         &lendtx_psql.tps3 ,
        //         &lendtx_psql.entry_sequence ,
        //     );
        //     let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        //     // client.execute(&query, &[]).unwrap();
        //     // let rt = self.clone();
        // });

        // let rt = self.clone();
        // handle.join().unwrap();
        return return_self;
    }

    pub fn calculatepayment(self) -> Result<Self, std::io::Error> {
        let mut lendtx = self.clone();
        // // let current_price = get_localdb("CurrentPrice");
        // let tps = redis_db::get_type_f64("tps");
        // let tlv = redis_db::get_type_f64("tlv");
        // let nwithdraw = normalize_withdraw(tlv, tps, self.npoolshare);
        match getset_settle_lend_order_tlv_tps_poolshare(self.npoolshare) {
            Ok((tlv2, tps2, tlv3, tps3, withdraw, nwithdraw, exit_nonce)) => {
                let payment;
                if self.new_lend_state_amount > withdraw {
                    payment = self.new_lend_state_amount - withdraw;
                } else {
                    payment = withdraw - self.new_lend_state_amount;
                }
                lendtx.order_status = OrderStatus::SETTLED;
                lendtx.nwithdraw = nwithdraw;
                lendtx.payment = payment;
                lendtx.tlv2 = tlv2;
                lendtx.tps2 = tps2;
                lendtx.tlv3 = tlv3;
                lendtx.tps3 = tps3;
                lendtx.exit_nonce = exit_nonce;
                lendtx = lendtx
                    .remove_lend_order_from_redis()
                    .update_psql_on_lend_settlement()
                    .update_lend_account_on_lendtx_order_settlement();

                Ok(lendtx)
            }
            // Err(arg) => println!("order not found !!, {:#?}", arg),
            Err(arg) => Err(std::io::Error::new(std::io::ErrorKind::Other, arg)),
        }
    }

    pub fn get_order_by_order_id(
        account_id: String,
        uuid: Uuid,
    ) -> Result<LendOrder, std::io::Error> {
        let ordertx_string = redis_db::get_no_error(&uuid.to_string());
        match ordertx_string {
            Ok(order) => Ok(LendOrder::deserialize(&order)),
            Err(arg) => Err(std::io::Error::new(std::io::ErrorKind::Other, arg)),
        }
    }

    pub fn remove_lend_order_from_redis(self) -> Self {
        let lendtx = self.clone();

        // thread::spawn(move || {
        // Lend order removed from redis, orderid as key
        redis_db::del(&lendtx.uuid.to_string());
        // trader order set by timestamp
        redis_db::zdel(
            &"LendOrder",
            &lendtx.uuid.to_string(), //value
        );
        // Lend order set by deposit
        redis_db::zdel(&"LendOrderbyDepositLendState", &lendtx.uuid.to_string());
        // });
        return self;
    }

    pub fn update_psql_on_lend_settlement(self) -> Self {
        let rt = self.clone();

        let query = format!(
            "UPDATE public.newlendorder
        SET  order_status='{:#?}',  nwithdraw={}, payment={},  tlv2={}, tps2={},tlv3={}, tps3={}, exit_nonce={}
        WHERE uuid='{}';",
            &self.order_status, &self.nwithdraw, &self.payment, &self.tlv2, &self.tps2,self.tlv3, &self.tps3, &self.exit_nonce, &self.uuid,
        );

        // thread to store Lend order data in postgreSQL
        let handle = thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        // handle.join().unwrap();
        return self;
    }

    pub fn update_lend_account_on_lendtx_order_settlement(self) -> Self {
        // negative payment // need to add next lender account
        let lendtx_for_settlement = self.clone();
        let best_lend_account_order_id = redis_db::getbestlender();
        let mut best_lend_account: LendOrder =
            LendOrder::deserialize(&redis_db::get(&best_lend_account_order_id[0]));
        println!("best lend account {:#?}", best_lend_account);
        if lendtx_for_settlement.new_lend_state_amount > lendtx_for_settlement.nwithdraw {
            best_lend_account.new_lend_state_amount = redis_db::zincr_lend_pool_account(
                &best_lend_account.uuid.to_string(),
                lendtx_for_settlement.payment,
            );
        }
        // positive payment need to get amount from next lender account and make payment
        else {
            best_lend_account.new_lend_state_amount = redis_db::zdecr_lend_pool_account(
                &best_lend_account.uuid.to_string(),
                lendtx_for_settlement.payment,
            );
        }
        //update best lender tx for newlendstate
        redis_db::set(
            &best_lend_account_order_id[0],
            &best_lend_account.serialize(),
        );
        return self;
    }

    // pub fn new_order(prc_command: ExecuteTraderOrder) -> Self {
    //     let ndeposit = deposit * 10000.0;
    //     let (tlv0, tps0, tlv1, tps1, poolshare, npoolshare, entry_nonce, entry_sequence): (
    //         f64,
    //         f64,
    //         f64,
    //         f64,
    //         f64,
    //         f64,
    //         usize,
    //         usize,
    //     ) = getset_new_lend_order_tlv_tps_poolshare(deposit);

    //     LendOrder {
    //         uuid: Uuid::new_v4(),
    //         account_id: String::from(account_id),
    //         balance,
    //         order_status,
    //         order_type,
    //         entry_nonce,
    //         exit_nonce: 0,
    //         deposit,
    //         new_lend_state_amount: ndeposit,
    //         timestamp: SystemTime::now(),
    //         npoolshare,
    //         nwithdraw: 0.0,
    //         payment: 0.0,
    //         tlv0,
    //         tps0,
    //         tlv1,
    //         tps1,
    //         tlv2: 0.0,
    //         tps2: 0.0,
    //         tlv3: 0.0,
    //         tps3: 0.0,
    //         entry_sequence,
    //     }
    // }
}
