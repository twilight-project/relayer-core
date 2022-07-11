use crate::config::*;
use crate::redislib::redis_db;
use crate::redislib::redisdb_orderinsert::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::thread;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    pub timestamp: SystemTime,
    pub bankruptcy_price: f64,
    pub bankruptcy_value: f64,
    pub maintenance_margin: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
    pub settlement_price: f64,
    pub entry_nonce: u128,
    pub exit_nonce: u128,
    pub entry_sequence: u128,
}

impl TraderOrder {
    pub fn new(
        account_id: &str,
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        available_margin: f64,
        mut order_status: OrderStatus,
        mut entryprice: f64,
        execution_price: f64,
    ) -> Self {
        let fee: f64;
        if order_type == OrderType::MARKET {
            entryprice = get_localdb("CurrentPrice");
            order_status = OrderStatus::FILLED;
            fee = get_localdb("Fee"); //different fee for market order
        } else if order_type == OrderType::LIMIT {
            order_status = OrderStatus::PENDING;
            fee = get_localdb("Fee"); //different fee for limit order
        } else {
            // order_status = OrderStatus::PENDING;
            fee = get_localdb("Fee"); //different fee for limit order
        }
        let position_side = positionside(&position_type);
        let entry_value = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entry_value, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fundingrate = get_localdb("FundingRate");
        let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, fundingrate);
        let liquidation_price = liquidationprice(
            entryprice,
            positionsize,
            position_side,
            maintenance_margin,
            initial_margin,
        );
        TraderOrder {
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
            timestamp: SystemTime::now(),
            bankruptcy_price,
            bankruptcy_value,
            maintenance_margin,
            liquidation_price,
            unrealized_pnl: 0.0,
            settlement_price: 0.0,
            entry_nonce: 0,
            exit_nonce: 0,
            entry_sequence: 0,
        }
    }

    pub fn newtraderorderinsert(self) -> Self {
        let mut ordertx = self.clone();
        let mut order_entry_status: bool = false;
        let current_price = get_localdb("CurrentPrice");

        match ordertx.order_type {
            OrderType::LIMIT => match ordertx.position_type {
                PositionType::LONG => {
                    if ordertx.entryprice >= current_price {
                        order_entry_status = true;
                        ordertx.order_status = OrderStatus::FILLED;
                        ordertx.entry_nonce = redis_db::get_nonce_u128();
                        ordertx.entry_sequence =
                            redis_db::incr_entry_sequence_by_one_trader_order();
                        ordertx.entryprice = current_price;
                    } else {
                        ordertx.order_status = OrderStatus::PENDING;
                    }
                }
                PositionType::SHORT => {
                    if ordertx.entryprice <= current_price {
                        order_entry_status = true;
                        ordertx.order_status = OrderStatus::FILLED;
                        ordertx.entry_nonce = redis_db::get_nonce_u128();
                        ordertx.entry_sequence =
                            redis_db::incr_entry_sequence_by_one_trader_order();
                        ordertx.entryprice = current_price;
                    } else {
                        ordertx.order_status = OrderStatus::PENDING;
                    }
                }
            },
            OrderType::MARKET => {
                ordertx.order_status = OrderStatus::FILLED;
                ordertx.entry_nonce = redis_db::get_nonce_u128();
                ordertx.entry_sequence = redis_db::incr_entry_sequence_by_one_trader_order();
                // ordertx.entryprice = current_price;
                order_entry_status = true;
            }
            _ => {}
        }

        let ordertx_clone = ordertx.clone();
        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        let threadpool_max_order_insert_pool = THREADPOOL_MAX_ORDER_INSERT.lock().unwrap();
        threadpool_max_order_insert_pool.execute(move || {
            // ordertx.entry_nonce = redis_db::get_nonce_u128();

            let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},'{}',{},{},{},{},{},{},{},{},{});",
                &ordertx.uuid,
                &ordertx.account_id ,
                &ordertx.position_type ,
                &ordertx.order_status ,
                &ordertx.order_type ,
                &ordertx.entryprice ,
                &ordertx.execution_price ,
                &ordertx.positionsize ,
                &ordertx.leverage ,
                &ordertx.initial_margin ,
                &ordertx.available_margin ,
                ServerTime::new(ordertx.timestamp).iso,
                &ordertx.bankruptcy_price ,
                &ordertx.bankruptcy_value ,
                &ordertx.maintenance_margin ,
                &ordertx.liquidation_price ,
                &ordertx.unrealized_pnl,
                &ordertx.settlement_price,
                &ordertx.entry_nonce,
                &ordertx.exit_nonce,
                &ordertx.entry_sequence,
            );
            // trader order saved in redis, orderid as key
            redis_db::set(&ordertx.uuid.to_string(), &ordertx.serialize());

            if order_entry_status {
                // trader order set by timestamp
                redis_db::zadd(
                    &"TraderOrder",
                    &ordertx.uuid.to_string(),           //value
                    &ordertx.entry_sequence.to_string(), //score
                );

                // update pool size when new order get inserted

                match ordertx.position_type {
                    PositionType::LONG => {
                        redis_db::incrbyfloat(
                            &"TotalLongPositionSize",
                            &ordertx.positionsize.to_string(),
                        );
                        // trader order set by liquidation_price for long
                        redis_db::zadd(
                            &"TraderOrderbyLiquidationPriceFORLong",
                            &ordertx.uuid.to_string(),
                            &ordertx.liquidation_price.to_string(),
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::incrbyfloat(
                            &"TotalShortPositionSize",
                            &ordertx.positionsize.to_string(),
                        );
                        // trader order set by liquidation_price for short
                        redis_db::zadd(
                            &"TraderOrderbyLiquidationPriceFORShort",
                            &ordertx.uuid.to_string(),
                            &ordertx.liquidation_price.to_string(),
                        );
                    }
                }

                redis_db::incrbyfloat(&"TotalPoolPositionSize", &ordertx.positionsize.to_string());

                // thread to store trader order data in postgreSQL
                let psql_insert_order_pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
                psql_insert_order_pool.execute(move || {
                    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                    client.execute(&query, &[]).unwrap();
                });
                drop(psql_insert_order_pool);
                let side = match ordertx.position_type {
                    PositionType::SHORT => Side::SELL,
                    PositionType::LONG => Side::BUY,
                };
                update_recent_orders(CloseTrade {
                    side: side,
                    positionsize: ordertx.positionsize,
                    price: ordertx.entryprice,
                    timestamp: std::time::SystemTime::now(),
                });
            } else {
                // trader order set by timestamp
                match ordertx.position_type {
                    PositionType::LONG => {
                        redis_db::zadd(
                            &"TraderOrder_LimitOrder_Pending_FOR_Long",
                            &ordertx.uuid.to_string(),       //value
                            &ordertx.entryprice.to_string(), //score
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::zadd(
                            &"TraderOrder_LimitOrder_Pending_FOR_Short",
                            &ordertx.uuid.to_string(),       //value
                            &ordertx.entryprice.to_string(), //score
                        );
                    }
                }
                // insert pending order in newpending table
                let query = format!("INSERT INTO public.pendinglimittraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},'{}',{},{},{},{},{},{},{},{},{});",
                    &ordertx.uuid,
                    &ordertx.account_id ,
                    &ordertx.position_type ,
                    &ordertx.order_status ,
                    &ordertx.order_type ,
                    &ordertx.entryprice ,
                    &ordertx.execution_price ,
                    &ordertx.positionsize ,
                    &ordertx.leverage ,
                    &ordertx.initial_margin ,
                    &ordertx.available_margin ,
                    ServerTime::new(ordertx.timestamp).iso,
                    &ordertx.bankruptcy_price ,
                    &ordertx.bankruptcy_value ,
                    &ordertx.maintenance_margin ,
                    &ordertx.liquidation_price ,
                    &ordertx.unrealized_pnl,
                    &ordertx.settlement_price,
                    &ordertx.entry_nonce,
                    &ordertx.exit_nonce,
                    &ordertx.entry_sequence,
                );
                // thread to store trader order data in postgreSQL
                let psql_insert_order_pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
                psql_insert_order_pool.execute(move || {
                    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                    client.execute(&query, &[]).unwrap();
                });
                drop(psql_insert_order_pool);
            }
        });
        drop(threadpool_max_order_insert_pool);
        // handle.join().unwrap();
        return ordertx_clone;
    }

    pub fn removeorderfromredis(self) -> Self {
        let ordertx = self.clone();
        let ordertx_clone = self.clone();

        let redis_threadpool = THREADPOOL_REDIS_ORDER_REMOVE.lock().unwrap();
        redis_threadpool.execute(move || {
            let mut cmd_array: Vec<String> = Vec::new();

            match ordertx.position_type {
                PositionType::LONG => {
                    cmd_array.push(String::from("TraderOrder_Settelment_by_LONG_Limit"));
                    cmd_array.push(String::from("TotalLongPositionSize"));
                    cmd_array.push(String::from("TraderOrderbyLiquidationPriceFORLong"));
                }
                PositionType::SHORT => {
                    cmd_array.push(String::from("TraderOrder_Settelment_by_SHORT_Limit"));
                    cmd_array.push(String::from("TotalShortPositionSize"));
                    cmd_array.push(String::from("TraderOrderbyLiquidationPriceFORShort"));
                }
            }
            order_remove_from_redis_pipeline(ordertx, cmd_array);
        });
        drop(redis_threadpool);

        let side = match ordertx_clone.position_type {
            PositionType::LONG => Side::SELL,
            PositionType::SHORT => Side::BUY,
        };
        update_recent_orders(CloseTrade {
            side: side,
            positionsize: ordertx_clone.positionsize,
            price: ordertx_clone.settlement_price,
            timestamp: std::time::SystemTime::now(),
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

    pub fn update_trader_order_table_into_db_on_funding_cycle(
        self,
        orderid: String,
        isliquidated: bool,
    ) {
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
        let pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
        pool.execute(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
            client.execute(&query, &[]).unwrap();
        });
        drop(pool);
        return self;
    }

    pub fn updatepsqlonliquidation(self) -> Self {
        let query = format!("UPDATE public.newtraderorder
            SET order_status='{:#?}',   available_margin={},  maintenance_margin={}, liquidation_price={},exit_nonce={} ,settlement_price={}
            WHERE uuid='{}';",
            &self.order_status ,
            &self.available_margin ,
            &self.maintenance_margin ,
            &self.liquidation_price ,
            &self.exit_nonce,
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
        let query = format!(
            "UPDATE public.newtraderorder
            SET order_status='{:#?}',   available_margin={},settlement_price={}, exit_nonce={}, unrealized_pnl={}
            WHERE uuid='{}';",
            &self.order_status,
            &self.available_margin,
            &self.settlement_price,
            &self.exit_nonce,
            &self.unrealized_pnl,
            &self.uuid
        );
        let pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
        pool.execute(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
            client.execute(&query, &[]).unwrap();
        });
        drop(pool);
        return self;
    }

    pub fn calculatepayment(self) -> Option<Self> {
        let mut ordertx = self.clone();
        let margindifference = ordertx.available_margin - ordertx.initial_margin;
        let current_price = get_localdb("CurrentPrice");
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
        ordertx.unrealized_pnl = u_pnl;
        let exit_nonce = updatelendaccountontraderordersettlement(payment * 10000.0);
        ordertx.exit_nonce = exit_nonce;
        ordertx = ordertx.removeorderfromredis().updatepsqlonsettlement();
        return Some(ordertx);
    }

    pub fn calculatepayment_with_current_price(self, current_price: f64) -> Self {
        let mut ordertx = self.clone();
        let margindifference = ordertx.available_margin - ordertx.initial_margin;
        // let current_price = get_localdb("CurrentPrice");
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
        ordertx.unrealized_pnl = u_pnl;
        let exit_nonce = updatelendaccountontraderordersettlement(payment * 10000.0);
        ordertx.exit_nonce = exit_nonce;
        ordertx = ordertx.removeorderfromredis().updatepsqlonsettlement();
        return ordertx;
    }

    pub fn set_execution_price_for_limit_order(self, execution_price: f64) -> Self {
        let ordertx = self.clone();

        match ordertx.position_type {
            PositionType::LONG => {
                redis_db::zadd(
                    &"TraderOrder_Settelment_by_LONG_Limit",
                    &ordertx.uuid.to_string(),
                    &execution_price.to_string(),
                );
            }
            PositionType::SHORT => {
                redis_db::zadd(
                    &"TraderOrder_Settelment_by_SHORT_Limit",
                    &ordertx.uuid.to_string(),
                    &execution_price.to_string(),
                );
            }
        }

        // need to update by procedure to check where the row exist or not, and if exist then change status of old row or create new triger to save old price
        let query = format!("INSERT INTO public.settlementpriceforlimitorder(
                uuid, account_id, position_type, order_status, order_type, execution_price, timestamp, settlement_price) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},'{}',{});",
            &ordertx.uuid,
            &ordertx.account_id ,
            &ordertx.position_type ,
            &ordertx.order_status ,
            &ordertx.order_type ,
            &execution_price ,
            ServerTime::new(SystemTime::now()).iso,
            &ordertx.settlement_price,
        );
        let handle = thread::spawn(move || {
            let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
            client.execute(&query, &[]).unwrap();
        });
        return self;
    }

    pub fn get_order_by_order_id(
        account_id: String,
        uuid: Uuid,
    ) -> Result<TraderOrder, std::io::Error> {
        let ordertx_string = redis_db::get_no_error(&uuid.to_string());
        match ordertx_string {
            Ok(order) => Ok(TraderOrder::deserialize(&order)),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        }
    }

    pub fn pending(
        account_id: &str,
        position_type: PositionType,
        order_type: OrderType,
        leverage: f64,
        initial_margin: f64,
        available_margin: f64,
        order_status: OrderStatus,
        entryprice: f64,
        execution_price: f64,
        uuid: Uuid,
        entry_nonce: u128,
        entry_sequence: u128,
    ) -> Self {
        // entryprice = get_localdb("CurrentPrice");
        let position_side = positionside(&position_type);
        let entry_value = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entry_value, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fee = get_localdb("Fee");
        let fundingrate = get_localdb("FundingRate");
        let maintenance_margin = maintenancemargin(entry_value, bankruptcy_value, fee, fundingrate);
        let liquidation_price = liquidationprice(
            entryprice,
            positionsize,
            position_side,
            maintenance_margin,
            initial_margin,
        );
        TraderOrder {
            uuid,
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
            timestamp: SystemTime::now(),
            bankruptcy_price,
            bankruptcy_value,
            maintenance_margin,
            liquidation_price,
            unrealized_pnl: 0.0,
            settlement_price: 0.0,
            entry_nonce: entry_nonce,
            exit_nonce: 0,
            entry_sequence: entry_sequence,
        }
    }

    pub fn pending_limit_traderorderinsert(self) -> Self {
        let rt = self.clone();
        let current_price = rt.entryprice;

        let rself = rt.clone();
        // thread to store trader order data in redisDB
        //inside operations can also be called in different thread
        thread::spawn(move || {
            // rt.entry_nonce = redis_db::get_nonce_u128();

            let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},'{}',{},{},{},{},{},{},{},{},{});",
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
                ServerTime::new(rt.timestamp).iso,
                &rt.bankruptcy_price ,
                &rt.bankruptcy_value ,
                &rt.maintenance_margin ,
                &rt.liquidation_price ,
                &rt.unrealized_pnl,
                &rt.settlement_price,
                &rt.entry_nonce,
                &rt.exit_nonce,
                &rt.entry_sequence,
            );
            // trader order saved in redis, orderid as key
            redis_db::set(&rt.uuid.to_string(), &rt.serialize());

            // trader order set by timestamp
            redis_db::zadd(
                &"TraderOrder",
                &rt.uuid.to_string(),           //value
                &rt.entry_sequence.to_string(), //score
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

            // match rt.order_type {
            //     OrderType::LIMIT => match rt.position_type {
            //         PositionType::LONG => {
            //             redis_db::zadd(
            //                 &"TraderOrderbyLONGLimit",
            //                 &rt.uuid.to_string(),
            //                 &rt.execution_price.to_string(),
            //             );
            //         }
            //         PositionType::SHORT => {
            //             redis_db::zadd(
            //                 &"TraderOrderbySHORTLimit",
            //                 &rt.uuid.to_string(),
            //                 &rt.execution_price.to_string(),
            //             );
            //         }
            //     },
            //     _ => {}
            // }
            // thread to store trader order data in postgreSQL
            thread::spawn(move || {
                let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                client.execute(&query, &[]).unwrap();
            });
        });
        // handle.join().unwrap();
        return rself;
    }

    pub fn cancelorder(self) -> (Self, bool) {
        let success: bool;
        let mut ordertx = self.clone();
        if ordertx.order_status == OrderStatus::PENDING {
            redis_db::del(&ordertx.uuid.to_string());
            match ordertx.order_type {
                OrderType::LIMIT => match ordertx.position_type {
                    PositionType::LONG => {
                        redis_db::zdel(
                            &"TraderOrder_LimitOrder_Pending_FOR_Long",
                            &ordertx.uuid.to_string(), //value
                        );
                    }
                    PositionType::SHORT => {
                        redis_db::zdel(
                            &"TraderOrder_LimitOrder_Pending_FOR_Short",
                            &ordertx.uuid.to_string(), //value
                        );
                    }
                },
                _ => {}
            }
            let query = format!(
                "UPDATE public.pendinglimittraderorder SET order_status='{:#?}'
                WHERE uuid='{}';",
                OrderStatus::CANCELLED,
                &ordertx.uuid
            );

            let handle = thread::spawn(move || {
                let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
                client.execute(&query, &[]).unwrap();
            });

            success = true;
            ordertx.order_status = OrderStatus::CANCELLED;
        } else {
            success = false;
        }
        return (ordertx, success);
    }
}
