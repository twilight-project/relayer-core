use crate::relayer::types::*;
use crate::relayer::utils::*;
use serde_derive::{Deserialize, Serialize};
extern crate uuid;
use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::redislib::redis_db;
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LendOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub balance: f64,
    pub order_status: OrderStatus, //lend or settle
    pub order_type: OrderType,     // LEND
    pub nonce: u128,               // change it to u256
    pub deposit: f64,
    pub new_lend_state_amount: f64,
    pub timestamp: u128,
    pub npoolshare: f64,
    pub nwithdraw: f64,
    pub payment: f64,
    pub tlv0: f64, //total locked value before lend tx
    pub tps0: f64, // total poolshare before lend tx
    pub tlv1: f64, // total locked value after lend tx
    pub tps1: f64, // total poolshre value after lend tx
    pub tlv2: f64, // total locked value after lend payment/settlement
    pub tps2: f64, // total poolshare after lend payment/settlement
}

impl LendOrder {
    pub fn new(
        account_id: &str,
        balance: f64,
        order_type: OrderType,
        order_status: OrderStatus,
        deposit: f64,
    ) -> Self {
        lend_mutex_lock(true);
        let ndeposit = deposit * 10000.0;
        let nonce = redis_db::incr_lend_nonce_by_one();
        // println!("Nonce : {}", nonce);
        let mut tlv = redis_db::get_type_f64("tlv");
        if tlv == 0.0 {
            tlv = initialize_lend_pool(100000.0, 10.0);
        }
        let tps = redis_db::get_type_f64("tps");
        let npoolshare = normalize_pool_share(tlv, tps, ndeposit);
        let tps1 = redis_db::incrbyfloat_type_f64("tps", npoolshare / 10000.0);
        let tlv1 = redis_db::incrbyfloat_type_f64("tlv", ndeposit);

        lend_mutex_lock(false);

        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => LendOrder {
                uuid: Uuid::new_v4(),
                account_id: String::from(account_id),
                balance,
                order_status,
                order_type,
                nonce,
                deposit: ndeposit,
                new_lend_state_amount: ndeposit,
                timestamp: n.as_millis(),
                npoolshare,
                nwithdraw: 0.0,
                payment: 0.0,
                tlv0: tlv,
                tps0: tps,
                tlv1,
                tps1,
                tlv2: 0.0,
                tps2: 0.0,
            },
            Err(e) => panic!("Could not generate new order: {}", e),
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

    pub fn newtraderorderinsert(self) -> Self {
        let rt = self.clone();

        let query = format!("INSERT INTO public.newlendorder(
            uuid, account_id, balance, order_status, order_type, nonce, deposit, new_lend_state_amount, timestamp, npoolshare, nwithdraw, payment, tlv0, tps0, tlv1, tps1, tlv2, tps2)
            VALUES ('{}','{}',{},'{:#?}','{:#?}',{},{},{},{},{},{},{},{},{},{},{},{},{});",
            &self.uuid,
            &self.account_id ,
            &self.balance ,
            &self.order_status ,
            &self.order_type ,
            &self.nonce ,
            &self.deposit ,
            &self.new_lend_state_amount ,
            &self.timestamp ,
            &self.npoolshare ,
            &self.nwithdraw ,
            &self.payment ,
            &self.tlv0 ,
            &self.tps0 ,
            &self.tlv1 ,
            &self.tps1 ,
            &self.tlv2 ,
            &self.tps2 ,
        );

        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        thread::spawn(move || {
            // Lend order saved in redis, orderid as key
            redis_db::set(&rt.uuid.to_string(), &rt.serialize());
            // Lend order set by nonce
            redis_db::zadd(
                &"LendOrder",
                &rt.uuid.to_string(),  //value
                &rt.nonce.to_string(), //score
            );

            // Lend order by there deposit amount as score

            redis_db::zadd(
                &"LendOrderbyDepositLendState",
                &rt.uuid.to_string(),    //value
                &rt.deposit.to_string(), //score
            );

            redis_db::incrbyfloat(&"TotalLendPoolSize", &rt.deposit.to_string());
        });
        // thread to store Lend order data in postgreSQL
        let handle = thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        // handle.join().unwrap();
        return self;
    }

    pub fn calculatepayment(self) -> Self {
        lend_mutex_lock(true);

        let mut lendtx = self.clone();
        // let current_price = redis_db::get_type_f64("CurrentPrice");
        let tps = redis_db::get_type_f64("tps");
        let tlv = redis_db::get_type_f64("tlv");
        let nwithdraw = normalize_withdraw(tlv, tps, self.npoolshare);
        let payment;
        if self.new_lend_state_amount > nwithdraw {
            payment = self.new_lend_state_amount - nwithdraw;
        } else {
            payment = nwithdraw - self.new_lend_state_amount;
        }
        lendtx.order_status = OrderStatus::SETTLED;
        lendtx.nwithdraw = nwithdraw;
        lendtx.payment = payment;
        lendtx.tlv2 = redis_db::decrbyfloat_type_f64("tlv", nwithdraw);
        lendtx.tps2 = redis_db::decrbyfloat_type_f64("tps", self.npoolshare / 10000.0);
        lendtx = lendtx
            .remove_lend_order_from_redis()
            .update_psql_on_lend_settlement()
            .update_lend_account_on_lendtx_order_settlement();

        lend_mutex_lock(false);

        return lendtx;
    }

    pub fn remove_lend_order_from_redis(self) -> Self {
        let rt = self.clone();

        // thread::spawn(move || {
        // Lend order removed from redis, orderid as key
        redis_db::del(&rt.uuid.to_string());
        // trader order set by timestamp
        redis_db::zdel(
            &"LendOrder",
            &rt.uuid.to_string(), //value
        );
        // Lend order set by deposit
        redis_db::zdel(&"LendOrderbyDepositLendState", &rt.uuid.to_string());
        // });
        return self;
    }
    pub fn update_psql_on_lend_settlement(self) -> Self {
        let rt = self.clone();

        let query = format!(
            "UPDATE public.newlendorder
        SET  order_status={:#?},  nwithdraw={}, payment={},  tlv2={}, tps2={}
        WHERE uuid='{}';",
            &self.order_status, &self.nwithdraw, &self.payment, &self.tlv2, &self.tps2, &self.uuid,
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
}
