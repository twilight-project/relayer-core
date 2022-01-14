#![allow(dead_code)]
#![allow(unused_variables)]
extern crate uuid;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::time::SystemTime;
use tpf::config::POSTGRESQL_POOL_CONNECTION;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub enum TXType {
    ORDERTX, //TraderOrder
    LENDTX,  //LendOrder
}
#[derive(Serialize, Deserialize, Debug)]
pub enum OrderType {
    LIMIT,
    MARKET,
    DARK,
    LEND,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum PositionType {
    LONG,
    SHORT,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum OrderStatus {
    SETTLED,
    LEND,
    LIQUIDATE,
    CANCELLED,
    PENDING,
}

#[derive(Serialize, Deserialize, Debug)]
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
}
#[derive(Serialize, Deserialize, Debug)]
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
pub fn unRealizedPNL(position_type: &PositionType,positionsize: f64, entryprice: f64,settleprice: f64) -> f64 {
    match position_type {
        &PositionType::LONG => positionsize * (1.0/entryprice - 1.0/settleprice),
        &PositionType::SHORT => positionsize * (1.0/settleprice - 1.0/entryprice),
        
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
pub fn maintenancemargin(entryvalue: f64, bankruptcyvalue: f64, fee: f64, funding: f64) -> f64 {
    (0.4 * entryvalue + fee * bankruptcyvalue + funding * bankruptcyvalue) /100.0
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
        entryprice * positionsize
            / ((positionside as f64) * entryprice * (mm - im) + positionsize)
    // }
}
pub fn positionside(position_type: &PositionType) -> i32 {
    match position_type {
        &PositionType::LONG => -1,
        &PositionType::SHORT => 1,
    }
}


// need to create new order **done
// need to create recalculate order
// need to create settle order initial_margin
// impl for order -> new, recaculate, liquidate etc
// create function bankruptcy price, bankruptcy rate, liquidation price 
// remove position side from traderorder stuct
// create_ts timestamp DEFAULT CURRENT_TIMESTAMP ,
// update_ts timestamp DEFAULT CURRENT_TIMESTAMP

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
        let entryvalue = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entryvalue, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fee = 0.002; //.2%
        let funding = 0.025; //2.5%
        let maintenance_margin = maintenancemargin(entryvalue, bankruptcy_value, fee, funding);
        let liquidation_price=liquidationprice(entryprice, positionsize,position_side, maintenance_margin, initial_margin);
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
            },
            Err(e) => panic!("Could not generate new order: {}", e),
        }
    }

    pub fn newtraderorderinsert(self) ->TraderOrder {
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

        let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},{},{},{},{},{});",
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
        &self.liquidation_price 
        );
        client.execute(&query, &[]).unwrap();
        // let rt = self.clone();
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
}
