use crate::relayer::types::*;
use serde_derive::{Deserialize, Serialize};
extern crate uuid;
use crate::relayer::utils::*;
use std::thread;
use std::time::SystemTime;

use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::redislib::redis_db;
use uuid::Uuid;

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
        let mut rt = self.clone();
        let current_price = redis_db::get("CurrentPrice").parse::<f64>().unwrap();

        let mut order_entry_status: bool = false;

        match rt.order_type {
            OrderType::LIMIT => match rt.position_type {
                PositionType::LONG => {
                    if rt.entryprice >= current_price {
                        order_entry_status = true;
                        rt.order_status = OrderStatus::FILLED;
                    } else {
                        rt.order_status = OrderStatus::PENDING;
                    }
                }
                PositionType::SHORT => {
                    if rt.entryprice <= current_price {
                        order_entry_status = true;
                        rt.order_status = OrderStatus::FILLED;
                    } else {
                        rt.order_status = OrderStatus::PENDING;
                    }
                }
            },
            OrderType::MARKET => {
                rt.order_status = OrderStatus::FILLED;
                rt.entryprice = current_price;
                order_entry_status = true;
            }
            _ => {}
        }

        let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},{},{},{},{},{},{},{});",
            &rt.uuid,
            &rt.account_id ,
            &rt.position_type ,
            &rt.order_status ,
            &rt.order_type ,
            &rt.entryprice ,
            &rt.execution_price ,
            &rt.positionsize ,
            &rt.leverage ,
            &rt.initial_margin ,
            &rt.available_margin ,
            &rt.timestamp ,
            &rt.bankruptcy_price ,
            &rt.bankruptcy_value ,
            &rt.maintenance_margin ,
            &rt.liquidation_price ,
            &rt.unrealized_pnl,
            &rt.settlement_price
        );

        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        thread::spawn(move || {
            // trader order saved in redis, orderid as key
            redis_db::set(&rt.uuid.to_string(), &rt.serialize());

            if order_entry_status {
                // trader order set by timestamp
                redis_db::zadd(
                    &"TraderOrder",
                    &rt.uuid.to_string(),      //value
                    &rt.timestamp.to_string(), //score
                );

                // update pool size when new order get inserted

                match rt.position_type {
                    PositionType::LONG => {
                        redis_db::incrbyfloat(
                            &"TotalLongPositionSize",
                            &rt.positionsize.to_string(),
                        );
                        // trader order set by liquidation_price for long
                        redis_db::zadd(
                            &"TraderOrderbyLiquidationPriceFORLong",
                            &rt.uuid.to_string(),
                            &rt.liquidation_price.to_string(),
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::incrbyfloat(
                            &"TotalShortPositionSize",
                            &rt.positionsize.to_string(),
                        );
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
            } else {
                // trader order set by timestamp
                match rt.position_type {
                    PositionType::LONG => {
                        redis_db::zadd(
                            &"TraderOrder_LimitOrder_Pending_FOR_Long",
                            &rt.uuid.to_string(),       //value
                            &rt.entryprice.to_string(), //score
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::zadd(
                            &"TraderOrder_LimitOrder_Pending_FOR_Short",
                            &rt.uuid.to_string(),       //value
                            &rt.entryprice.to_string(), //score
                        );
                    }
                }
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
