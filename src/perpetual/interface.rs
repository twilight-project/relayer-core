#![allow(dead_code)]
#![allow(unused_variables)]
extern crate uuid;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::thread;
use std::time::SystemTime;
use tpf::config::POSTGRESQL_POOL_CONNECTION;
use tpf::redislib::redis_db;

use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TXType {
    ORDERTX, //TraderOrder
    LENDTX,  //LendOrder
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderType {
    LIMIT,
    MARKET,
    DARK,
    LEND,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PositionType {
    LONG,
    SHORT,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderStatus {
    SETTLED,
    LENDED,
    LIQUIDATE,
    CANCELLED,
    PENDING, // change it to New
    FILLED,  //executed on price ticker
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraderOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub position_type: PositionType,
    pub order_status: OrderStatus, //SETTLED or LIQUIDATE or PENDING or FILLED
    pub order_type: OrderType,
    pub entryprice: f64,
    pub execution_price: f64,
    pub positionsize: f64,
    pub leverage: f64,
    pub initial_margin: f64,
    pub available_margin: f64,
    pub timestamp: u128,
    pub bankruptcy_price: f64,
    pub bankruptcy_value: f64,
    pub maintenance_margin: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
    pub settlement_price: f64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LendOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub balance: f64,
    pub order_status: OrderStatus, //lend or settle
    pub order_type: OrderType,     // LEND
    pub nonce: i32,
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

pub fn entryvalue(initial_margin: f64, leverage: f64) -> f64 {
    initial_margin * leverage
}
pub fn positionsize(entryvalue: f64, entryprice: f64) -> f64 {
    entryvalue * entryprice
}

// execution_price = settle price
pub fn unrealizedpnl(
    position_type: &PositionType,
    positionsize: f64,
    entryprice: f64,
    settleprice: f64,
) -> f64 {
    match position_type {
        // &PositionType::LONG => positionsize * (1.0 / entryprice - 1.0 / settleprice),
        &PositionType::LONG => {
            (positionsize * (settleprice - entryprice)) / (entryprice * settleprice)
        }
        // &PositionType::SHORT => positionsize * (1.0 / settleprice - 1.0 / entryprice),
        &PositionType::SHORT => {
            (positionsize * (entryprice - settleprice)) / (entryprice * settleprice)
        }
    }
}

pub fn bankruptcyprice(position_type: &PositionType, entryprice: f64, leverage: f64) -> f64 {
    match position_type {
        &PositionType::LONG => entryprice * leverage / (leverage + 1.0),
        &PositionType::SHORT => {
            if leverage > 1.0 {
                entryprice * leverage / (leverage - 1.0)
            } else {
                0.0
            }
        }
    }
}
pub fn bankruptcyvalue(positionsize: f64, bankruptcyprice: f64) -> f64 {
    if bankruptcyprice > 0.0 {
        positionsize / bankruptcyprice
    } else {
        0.0
    }
}
pub fn maintenancemargin(entry_value: f64, bankruptcyvalue: f64, fee: f64, funding: f64) -> f64 {
    (0.4 * entry_value + fee * bankruptcyvalue + funding * bankruptcyvalue) / 100.0
}

pub fn liquidationprice(
    entryprice: f64,
    positionsize: f64,
    positionside: i32,
    mm: f64,
    im: f64,
) -> f64 {
    // if positionside > 0 {
    //     entryprice * positionsize
    //         / ((positionside as f64) * entryprice * (im - mm) + positionsize)
    // } else {
    entryprice * positionsize / ((positionside as f64) * entryprice * (mm - im) + positionsize)
    // }
}
pub fn positionside(position_type: &PositionType) -> i32 {
    match position_type {
        &PositionType::LONG => -1,
        &PositionType::SHORT => 1,
    }
}

pub fn updatefundingrate(psi: f64) {
    let totalshort = redis_db::get_type_f64("TotalShortPositionSize");
    let totallong = redis_db::get_type_f64("TotalLongPositionSize");
    let allpositionsize = redis_db::get_type_f64("TotalPoolPositionSize");
    //powi is faster then powf
    //psi = 8.0 or  ψ = 8.0
    let mut fundingrate;
    if allpositionsize == 0.0 {
        fundingrate = 0.0;
    } else {
        fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);
    }

    //positive funding if totallong > totalshort else negative funding
    if totallong > totalshort {
    } else {
        fundingrate = fundingrate * (-1.0);
    }
    updatefundingrateindb(fundingrate);
}

pub fn updatefundingrateindb(fundingrate: f64) {
    redis_db::set("FundingRate", &fundingrate.to_string());

    // need to create parameter table in psql and then update funding rate in psql, same for all pool size n all
}
////********* operation on each funding cycle **** //////

pub fn updatechangesineachordertxonfundingratechange(
    orderid: String,
    fundingratechange: f64,
    current_price: f64,
    fee: f64,
) -> TraderOrder {
    //get order from redis by orderid
    let mut ordertx: TraderOrder = TraderOrder::deserialize(&redis_db::get(&orderid));
    // to check liquidated orders
    let mut isliquidated = false;

    //update available margin
    match ordertx.position_type {
        PositionType::LONG => {
            if fundingratechange > 0.0 {
                ordertx.available_margin = ordertx.available_margin
                    - ((fundingratechange * ordertx.positionsize) / current_price)
            } else {
                ordertx.available_margin = ordertx.available_margin
                    - ((fundingratechange * ordertx.positionsize) / current_price)
            }
        }
        PositionType::SHORT => {
            if fundingratechange > 0.0 {
                ordertx.available_margin = ordertx.available_margin
                    + ((fundingratechange * ordertx.positionsize) / current_price)
            } else {
                ordertx.available_margin = ordertx.available_margin
                    + ((fundingratechange * ordertx.positionsize) / current_price)
            }
        }
    }
    // update maintenancemargin
    ordertx.maintenance_margin = maintenancemargin(
        entryvalue(ordertx.initial_margin, ordertx.leverage),
        ordertx.bankruptcy_value,
        fee,
        fundingratechange,
    );

    // check if AM <= MM if true then call liquidate position else update liquidation price
    if ordertx.available_margin <= ordertx.maintenance_margin {
        //call liquidation
        ordertx = liquidateposition(ordertx, current_price);
        ordertx.order_status = OrderStatus::LIQUIDATE;
        isliquidated = true;
    } else {
        ordertx.liquidation_price = liquidationprice(
            ordertx.entryprice,
            ordertx.positionsize,
            positionside(&ordertx.position_type),
            ordertx.maintenance_margin,
            ordertx.initial_margin,
        );
        // idliquidate = false;
    }
    let ordertxclone = ordertx.clone();
    ordertxclone.updatetraderordertableintodb(orderid, isliquidated);
    return ordertx;
}
pub fn liquidateposition(mut ordertx: TraderOrder, current_price: f64) -> TraderOrder {
    ordertx.available_margin = 0.0;
    ordertx.settlement_price = current_price;
    ordertx.liquidation_price = current_price;
    ordertx
}

pub fn getandupdateallordersonfundingcycle() {
    let orderid_list = redis_db::zrangeallopenorders();
    let current_price = redis_db::get("CurrentPrice").parse::<f64>().unwrap();
    let fundingrate = redis_db::get("FundingRate").parse::<f64>().unwrap();
    let fee = redis_db::get("Fee").parse::<f64>().unwrap();
    for orderid in orderid_list {
        let state =
            updatechangesineachordertxonfundingratechange(orderid, fundingrate, current_price, fee);
        println!("order of {} is {:#?}", state.uuid, state.order_status);
    }
}

////********* operation on each funding cycle end **** //////
/// also add ammount in tlv **  update nonce also

pub fn updatelendaccountontraderordersettlement(payment: f64) {
    let best_lend_account_order_id = redis_db::getbestlender();
    let mut best_lend_account: LendOrder =
        LendOrder::deserialize(&redis_db::get(&best_lend_account_order_id[0]));
    best_lend_account.new_lend_state_amount =
        redis_db::zdecr_lend_pool_account(&best_lend_account.uuid.to_string(), payment);
    //update best lender tx for newlendstate
    redis_db::set(
        &best_lend_account_order_id[0],
        &best_lend_account.serialize(),
    );
    redis_db::decrbyfloat_type_f64("tlv", payment);
    println!(
        "lend account {} changed by {}",
        &best_lend_account_order_id[0], payment
    );
}

// need to create new order **done
// need to create recalculate order **done
// need to create settle order initial_margin **done
// impl for order -> new, recaculate, liquidate etc  **done
// create function bankruptcy price, bankruptcy rate, liquidation price  **done
// remove position side from traderorder stuct  **done
// create_ts timestamp DEFAULT CURRENT_TIMESTAMP ,
// update_ts timestamp DEFAULT CURRENT_TIMESTAMP
// need to create parameter table in psql and then update funding rate in psql, same for all pool size n all
// tlv tps mutex check
// limit order execution
// add entry nonce and exit nonce for traderorder as well as lendorder

impl TraderOrder {
    pub fn new(
        account_id: &str,
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        available_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
        execution_price: f64,
    ) -> Self {
        let position_side = positionside(&position_type);
        let entry_value = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entry_value, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fee = redis_db::get_type_f64("Fee");
        let fundingrate = redis_db::get_type_f64("FundingRate");
        let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, fundingrate);
        let liquidation_price = liquidationprice(
            entryprice,
            positionsize,
            position_side,
            maintenance_margin,
            initial_margin,
        );
        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => TraderOrder {
                uuid: Uuid::new_v4(),
                account_id: String::from(account_id),
                position_type,
                order_status,
                order_type,
                entryprice,
                execution_price,
                positionsize,
                leverage,
                initial_margin,
                available_margin,
                timestamp: n.as_millis(),
                bankruptcy_price,
                bankruptcy_value,
                maintenance_margin,
                liquidation_price,
                unrealized_pnl: 0.0,
                settlement_price: 0.0,
            },
            Err(e) => panic!("Could not generate new order: {}", e),
        }
    }

    pub fn newtraderorderinsert(self) -> Self {
        let rt = self.clone();

        let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},{},{},{},{},{},{},{});",
            &self.uuid,
            &self.account_id ,
            &self.position_type ,
            &self.order_status ,
            &self.order_type ,
            &self.entryprice ,
            &self.execution_price ,
            &self.positionsize ,
            &self.leverage ,
            &self.initial_margin ,
            &self.available_margin ,
            &self.timestamp ,
            &self.bankruptcy_price ,
            &self.bankruptcy_value ,
            &self.maintenance_margin ,
            &self.liquidation_price ,
            &self.unrealized_pnl,
            &self.settlement_price
        );

        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        thread::spawn(move || {
            // trader order saved in redis, orderid as key
            redis_db::set(&rt.uuid.to_string(), &rt.serialize());
            // trader order set by timestamp
            redis_db::zadd(
                &"TraderOrder",
                &rt.uuid.to_string(),      //value
                &rt.timestamp.to_string(), //score
            );

            // update pool size when new order get inserted
            match rt.position_type {
                PositionType::LONG => {
                    redis_db::incrbyfloat(&"TotalLongPositionSize", &rt.positionsize.to_string());
                    // trader order set by liquidation_price for long
                    redis_db::zadd(
                        &"TraderOrderbyLiquidationPriceFORLong",
                        &rt.uuid.to_string(),
                        &rt.liquidation_price.to_string(),
                    );
                }
                PositionType::SHORT => {
                    redis_db::incrbyfloat(&"TotalShortPositionSize", &rt.positionsize.to_string());
                    // trader order set by liquidation_price for short
                    redis_db::zadd(
                        &"TraderOrderbyLiquidationPriceFORShort",
                        &rt.uuid.to_string(),
                        &rt.liquidation_price.to_string(),
                    );
                }
            }
            redis_db::incrbyfloat(&"TotalPoolPositionSize", &rt.positionsize.to_string());

            match rt.order_type {
                OrderType::LIMIT => match rt.position_type {
                    PositionType::LONG => {
                        redis_db::zadd(
                            &"TraderOrderbyLONGLimit",
                            &rt.uuid.to_string(),
                            &rt.execution_price.to_string(),
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::zadd(
                            &"TraderOrderbySHORTLimit",
                            &rt.uuid.to_string(),
                            &rt.execution_price.to_string(),
                        );
                    }
                },
                _ => {}
            }
        });
        // thread to store trader order data in postgreSQL
        let handle = thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        // handle.join().unwrap();
        return self;
    }

    pub fn removeorderfromredis(self) -> Self {
        let rt = self.clone();

        thread::spawn(move || {
            // trader order saved in redis, orderid as key
            redis_db::del(&rt.uuid.to_string());
            // trader order set by timestamp
            redis_db::zdel(
                &"TraderOrder",
                &rt.uuid.to_string(), //value
            );
            // trader order set by liquidation_price
            redis_db::zdel(&"TraderOrderbyLiquidationPrice", &rt.uuid.to_string());

            match rt.order_type {
                OrderType::LIMIT => match rt.position_type {
                    PositionType::LONG => {
                        redis_db::zdel(&"TraderOrderbyLONGLimit", &rt.uuid.to_string());
                    }
                    PositionType::SHORT => {
                        redis_db::zdel(&"TraderOrderbySHORTLimit", &rt.uuid.to_string());
                    }
                },
                _ => {}
            }

            // update pool size when  order get settled
            match rt.position_type {
                PositionType::LONG => {
                    redis_db::decrbyfloat(&"TotalLongPositionSize", &rt.positionsize.to_string());
                    // trader order set by liquidation_price for long
                    redis_db::zdel(
                        &"TraderOrderbyLiquidationPriceFORLong",
                        &rt.uuid.to_string(),
                    );
                }
                PositionType::SHORT => {
                    redis_db::decrbyfloat(&"TotalShortPositionSize", &rt.positionsize.to_string());
                    // trader order set by liquidation_price for short
                    redis_db::zdel(
                        &"TraderOrderbyLiquidationPriceFORShort",
                        &rt.uuid.to_string(),
                    );
                }
            }
            redis_db::decrbyfloat(&"TotalPoolPositionSize", &rt.positionsize.to_string());
        });
        return self;
    }
    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }
    pub fn deserialize(json: &String) -> Self {
        let deserialized: TraderOrder = serde_json::from_str(json).unwrap();
        deserialized
    }

    pub fn updatetraderordertableintodb(self, orderid: String, isliquidated: bool) {
        let ordertx = self.clone();

        if isliquidated {
            ordertx.removeorderfromredis().updatepsqlonliquidation();
        } else {
            redis_db::set(&orderid, &ordertx.serialize());
            ordertx.updatepsqlonfundingcycle();
        }
    }

    pub fn updatepsqlonfundingcycle(self) -> Self {
        // ordertx.available_margin
        // ordertx.maintenance_margin
        // ordertx.liquidation_price

        let query = format!(
            "UPDATE public.newtraderorder
        SET available_margin={},  maintenance_margin={}, liquidation_price={}
        WHERE uuid='{}';",
            &self.available_margin, &self.maintenance_margin, &self.liquidation_price, &self.uuid
        );
        thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        return self;
    }

    pub fn updatepsqlonliquidation(self) -> Self {
        // ordertx.order_status
        // ordertx.available_margin
        // ordertx.settlement_price
        // ordertx.liquidation_price
        // ordertx.maintenance_margin

        let query = format!("UPDATE public.newtraderorder
            SET order_status='{:#?}',   available_margin={},  maintenance_margin={}, liquidation_price={},settlement_price={}
            WHERE uuid='{}';",
            &self.order_status ,
            &self.available_margin ,
            &self.maintenance_margin ,
            &self.liquidation_price ,
            &self.settlement_price,
            &self.uuid
            );
        thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        return self;
    }
    pub fn updatepsqlonsettlement(self) -> Self {
        // ordertx.order_status
        // ordertx.available_margin
        // ordertx.settlement_price
        // ordertx.liquidation_price
        // ordertx.maintenance_margin

        let query = format!(
            "UPDATE public.newtraderorder
            SET order_status='{:#?}',   available_margin={},settlement_price={}
            WHERE uuid='{}';",
            &self.order_status, &self.available_margin, &self.settlement_price, &self.uuid
        );
        thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

            client.execute(&query, &[]).unwrap();
            // let rt = self.clone();
        });
        return self;
    }

    pub fn calculatepayment(self) -> Self {
        let mut ordertx = self.clone();
        let margindifference = ordertx.available_margin - ordertx.initial_margin;
        let current_price = redis_db::get_type_f64("CurrentPrice");
        // let current_price = redis_db::get(&"CurrentPrice").parse::<f64>().unwrap();
        let u_pnl = unrealizedpnl(
            &ordertx.position_type,
            ordertx.positionsize,
            ordertx.entryprice,
            current_price,
        );
        let payment = u_pnl + margindifference;
        ordertx.order_status = OrderStatus::SETTLED;
        ordertx.available_margin = ordertx.available_margin + payment;
        ordertx.settlement_price = current_price;
        updatelendaccountontraderordersettlement(payment);
        ordertx = ordertx.removeorderfromredis().updatepsqlonsettlement();
        return ordertx;
    }
}

//////****** Lending fn ***************/
///
/// undate this function to return both poolshare and npoolshare
pub fn normalize_pool_share(tlv: f64, tps: f64, deposit: f64) -> f64 {
    let npoolshare = tps * deposit * 10000.0 / tlv;

    return npoolshare;
}

/// undate this function to return both nwithdraw and withdraw
pub fn normalize_withdraw(tlv: f64, tps: f64, npoolshare: f64) -> f64 {
    let nwithdraw = tlv * npoolshare / tps;
    return nwithdraw;
}

pub fn initialize_lend_pool(tlv: f64, tps: f64) -> f64 {
    redis_db::set("tlv", &tlv.to_string());
    redis_db::set("tps", &tps.to_string());
    tlv
}

pub fn lend_mutex_lock(lock: bool) {}

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
