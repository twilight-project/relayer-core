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
    LEND,
    LIQUIDATE,
    CANCELLED,
    PENDING,
    FILLED,
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
    pub order_type: OrderType,     // LEND
    pub order_status: OrderStatus, //lend or settle
    pub nonce: i32,
    pub lend_amount: f64,
    pub timestamp: u128,
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
        &PositionType::LONG => positionsize * (1.0 / entryprice - 1.0 / settleprice),
        &PositionType::SHORT => positionsize * (1.0 / settleprice - 1.0 / entryprice),
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
    let totalshort = redis_db::get("TotalShortPositionSize")
        .parse::<f64>()
        .unwrap();
    let totallong = redis_db::get("TotalLongPositionSize")
        .parse::<f64>()
        .unwrap();
    let allpositionsize = redis_db::get("TotalPoolPositionSize")
        .parse::<f64>()
        .unwrap();

    //powi is faster then powf
    //psi = 8.0 or  ψ = 8.0
    let mut fundingrate = f64::powi((totallong - totalshort) / allpositionsize, 2) / (psi * 8.0);

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
                    - (fundingratechange * (ordertx.positionsize / current_price))
            } else {
                ordertx.available_margin = ordertx.available_margin
                    + (fundingratechange * (ordertx.positionsize / current_price))
            }
        }
        PositionType::SHORT => {
            if fundingratechange > 0.0 {
                ordertx.available_margin = ordertx.available_margin
                    + (fundingratechange * (ordertx.positionsize / current_price))
            } else {
                ordertx.available_margin = ordertx.available_margin
                    - (fundingratechange * (ordertx.positionsize / current_price))
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
        updatechangesineachordertxonfundingratechange(orderid, fundingrate, current_price, fee);
    }
}

////********* operation on each funding cycle end **** //////

// need to create new order **done
// need to create recalculate order
// need to create settle order initial_margin
// impl for order -> new, recaculate, liquidate etc
// create function bankruptcy price, bankruptcy rate, liquidation price
// remove position side from traderorder stuct
// create_ts timestamp DEFAULT CURRENT_TIMESTAMP ,
// update_ts timestamp DEFAULT CURRENT_TIMESTAMP
// need to create parameter table in psql and then update funding rate in psql, same for all pool size n all

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
        let fee = 0.002; //.2%
        let funding = 0.025; //2.5%
        let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, funding);
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
            SET order_status={:#?},   available_margin={},  maintenance_margin={}, liquidation_price={},settlement_price={}
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
}
