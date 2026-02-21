use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
//inc last_update_at :timestamp
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TraderOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub position_type: PositionType,
    pub order_status: OrderStatus,
    pub order_type: OrderType,
    pub entryprice: f64,
    pub execution_price: f64,
    pub positionsize: f64,
    pub leverage: f64,
    pub initial_margin: f64,
    pub available_margin: f64,
    pub timestamp: String,
    pub bankruptcy_price: f64,
    pub bankruptcy_value: f64,
    pub maintenance_margin: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
    pub settlement_price: f64,
    pub entry_nonce: usize,
    pub exit_nonce: usize,
    pub entry_sequence: usize,
    pub fee_filled: f64,
    pub fee_settled: f64,
}
impl TraderOrder {
    pub fn new_order(mut rpc_request: CreateTraderOrder) -> (Self, bool) {
        let current_price = get_localdb("CurrentPrice");
        let mut order_entry_status: bool = false;
        if rpc_request.order_type == OrderType::LIMIT {
            match rpc_request.position_type {
                PositionType::LONG => {
                    if rpc_request.entryprice >= current_price {
                        // may cancel the order
                        rpc_request.order_type = OrderType::MARKET;
                        order_entry_status = true;
                    } else {
                    }
                }
                PositionType::SHORT => {
                    if rpc_request.entryprice <= current_price {
                        // may cancel the order
                        rpc_request.order_type = OrderType::MARKET;
                        order_entry_status = true;
                    } else {
                    }
                }
            }
        } else if rpc_request.order_type == OrderType::MARKET {
            order_entry_status = true;
        }
        let account_id = rpc_request.account_id;
        let position_type = rpc_request.position_type;
        let order_type = rpc_request.order_type;
        let leverage = rpc_request.leverage;
        let initial_margin = rpc_request.initial_margin;
        let mut available_margin = rpc_request.initial_margin;
        let mut order_status = rpc_request.order_status;
        let mut entryprice = rpc_request.entryprice;
        let execution_price = rpc_request.execution_price;
        let mut fee_percentage: f64 = 0.0;
        let mut fee_value: f64;
        match order_type {
            OrderType::MARKET => {
                // entryprice = get_localdb("CurrentPrice");
                entryprice = current_price;
                order_status = OrderStatus::FILLED;
                fee_percentage = get_fee(FeeType::FilledOnMarket); //different fee for market order
            }
            OrderType::LIMIT => {
                order_status = OrderStatus::PENDING;
                fee_percentage = get_fee(FeeType::FilledOnLimit); //different fee for limit order
            }
            _ => {}
        }
        let entry_value = entryvalue(initial_margin, leverage);

        let position_side = positionside(&position_type);
        let positionsize = positionsize(entry_value, entryprice);
        fee_value = calculate_fee_on_open_order(fee_percentage, positionsize, entryprice);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let fundingrate = get_localdb("FundingRate");
        let maintenance_margin =
            maintenancemargin(entry_value, bankruptcy_value, fee_percentage, fundingrate);
        let liquidation_price = liquidationprice(
            entryprice,
            positionsize,
            position_side,
            maintenance_margin,
            initial_margin,
        );
        if order_status == OrderStatus::PENDING {
            fee_value = 0.0;
        }
        available_margin = available_margin - fee_value;
        let uuid_key = Uuid::new_v4();
        let new_account_id;

        if String::from(account_id.clone()) == String::from("account_id") {
            new_account_id = uuid_key.to_string();
        } else {
            new_account_id = String::from(account_id);
        }
        (
            TraderOrder {
                uuid: uuid_key,
                account_id: new_account_id,
                position_type,
                order_status,
                order_type,
                entryprice,
                execution_price,
                positionsize,
                leverage,
                initial_margin,
                available_margin,
                timestamp: systemtime_to_utc(),
                bankruptcy_price,
                bankruptcy_value,
                maintenance_margin,
                liquidation_price,
                unrealized_pnl: 0.0,
                settlement_price: 0.0,
                entry_nonce: 0,
                exit_nonce: 0,
                entry_sequence: 0,
                fee_filled: fee_value,
                fee_settled: 0.0,
            },
            order_entry_status,
        )
    }

    pub fn pending_order(&self, current_price: f64) -> (Self, bool) {
        let order_entry_status: bool = true;
        let position_type = self.position_type.clone();
        let leverage = self.leverage;
        let initial_margin = self.initial_margin.clone();
        let entryprice = current_price;
        let fee_percentage: f64 = get_fee(FeeType::FilledOnLimit); //different fee for market order

        let fundingrate = get_localdb("FundingRate");
        let position_side = positionside(&position_type);
        let entry_value = entryvalue(initial_margin, leverage);
        let positionsize = positionsize(entry_value, entryprice);
        let fee_value: f64 =
            calculate_fee_on_open_order(fee_percentage, positionsize, current_price);
        let bankruptcy_price = bankruptcyprice(&position_type, entryprice, leverage);
        let bankruptcy_value = bankruptcyvalue(positionsize, bankruptcy_price);
        let maintenance_margin =
            maintenancemargin(entry_value, bankruptcy_value, fee_percentage, fundingrate);
        let liquidation_price = liquidationprice(
            entryprice,
            positionsize,
            position_side,
            maintenance_margin,
            initial_margin,
        );
        let available_margin = self.initial_margin - fee_value;
        (
            TraderOrder {
                uuid: self.uuid,
                account_id: self.account_id.clone(),
                position_type,
                order_status: OrderStatus::FILLED,
                order_type: self.order_type.clone(),
                entryprice,
                execution_price: self.execution_price,
                positionsize,
                leverage,
                initial_margin,
                available_margin,
                timestamp: systemtime_to_utc(),
                bankruptcy_price,
                bankruptcy_value,
                maintenance_margin,
                liquidation_price,
                unrealized_pnl: 0.0,
                settlement_price: 0.0,
                entry_nonce: self.entry_nonce.clone(),
                exit_nonce: 0,
                entry_sequence: self.entry_sequence.clone(),
                fee_filled: fee_value,
                fee_settled: 0.0,
            },
            order_entry_status,
        )
    }

    pub fn orderinsert_localdb(self, order_entry_status: bool) -> TraderOrder {
        let ordertx = self.clone();
        // Risk engine bookkeeping: track BTC exposure separately for filled vs pending
        if ordertx.order_status == OrderStatus::FILLED && ordertx.order_type == OrderType::MARKET {
            RiskState::add_order(
                ordertx.position_type.clone(),
                entryvalue(ordertx.initial_margin, ordertx.leverage),
            );
        } else if ordertx.order_status == OrderStatus::PENDING
            && ordertx.order_type == OrderType::LIMIT
        {
            RiskState::add_pending_order(
                ordertx.position_type.clone(),
                entryvalue(ordertx.initial_margin, ordertx.leverage),
            );
        } else if ordertx.order_status == OrderStatus::FILLED
            && ordertx.order_type == OrderType::LIMIT
        {
            // Limit order fill: move exposure from pending → filled
            RiskState::remove_pending_order(
                ordertx.position_type.clone(),
                entryvalue(ordertx.initial_margin, ordertx.leverage),
            );
            RiskState::add_order(
                ordertx.position_type.clone(),
                entryvalue(ordertx.initial_margin, ordertx.leverage),
            );
        }
        if order_entry_status {
            // Adding in side wise and total position size
            PositionSizeLog::add_order(ordertx.position_type.clone(), ordertx.positionsize.clone());
            // Adding in liquidation search table
            match ordertx.position_type {
                PositionType::LONG => {
                    let mut add_to_liquidation_list = TRADER_LP_LONG.lock().unwrap();
                    let _ = add_to_liquidation_list
                        .add(ordertx.uuid, (ordertx.liquidation_price * 10000.0) as i64);
                    drop(add_to_liquidation_list);
                }
                PositionType::SHORT => {
                    let mut add_to_liquidation_list = TRADER_LP_SHORT.lock().unwrap();
                    let _ = add_to_liquidation_list
                        .add(ordertx.uuid, (ordertx.liquidation_price * 10000.0) as i64);
                    drop(add_to_liquidation_list);
                }
            }
        } else {
            match ordertx.position_type {
                PositionType::LONG => {
                    let mut add_to_open_order_list = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
                    let _ = add_to_open_order_list
                        .add(ordertx.uuid, (ordertx.entryprice * 10000.0) as i64);
                    drop(add_to_open_order_list);
                }
                PositionType::SHORT => {
                    let mut add_to_open_order_list = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
                    let _ = add_to_open_order_list
                        .add(ordertx.uuid, (ordertx.entryprice * 10000.0) as i64);
                    drop(add_to_open_order_list);
                }
            }
        }
        ordertx
    }

    pub fn check_for_settlement(
        &mut self,
        execution_price: f64,
        current_price: f64,
        cmd_order_type: OrderType,
        sltp: Option<SlTpOrder>,
    ) -> (f64, OrderStatus) {
        let mut payment: f64 = 0.0;
        let mut orderstatus: OrderStatus = OrderStatus::FILLED;
        match cmd_order_type {
            OrderType::MARKET => {
                payment =
                    self.calculatepayment_localdb(current_price, get_fee(FeeType::FilledOnMarket));
                orderstatus = OrderStatus::SETTLED;
            }
            OrderType::LIMIT => match self.position_type {
                PositionType::LONG => {
                    if execution_price <= current_price {
                        payment = self.calculatepayment_localdb(
                            current_price,
                            get_fee(FeeType::FilledOnLimit),
                        );
                        orderstatus = OrderStatus::SETTLED;
                    } else {
                        let order_caluculated =
                            self.set_execution_price_for_limit_order_localdb(execution_price);
                    }
                }
                PositionType::SHORT => {
                    if execution_price >= current_price {
                        payment = self.calculatepayment_localdb(
                            current_price,
                            get_fee(FeeType::FilledOnLimit),
                        );
                        orderstatus = OrderStatus::SETTLED;
                    } else {
                        let order_caluculated =
                            self.set_execution_price_for_limit_order_localdb(execution_price);
                    }
                }
            },
            OrderType::SLTP => match sltp {
                Some(sltp) => {
                    match sltp.sl {
                        Some(sl) => match self.position_type {
                            PositionType::LONG => {
                                if sl >= current_price {
                                    payment = self.calculatepayment_localdb(
                                        current_price,
                                        get_fee(FeeType::FilledOnMarket),
                                    );
                                    orderstatus = OrderStatus::SETTLED;
                                } else {
                                    let order_caluculated =
                                                self.set_execution_price_for_limit_order_stoploss_takeprofit_localdb(
                                                    sl,
                                                    SlTpOrderType::StopLoss
                                                );
                                }
                            }
                            PositionType::SHORT => {
                                if sl <= current_price {
                                    payment = self.calculatepayment_localdb(
                                        current_price,
                                        get_fee(FeeType::FilledOnMarket),
                                    );
                                    orderstatus = OrderStatus::SETTLED;
                                } else {
                                    let order_caluculated =
                                                self.set_execution_price_for_limit_order_stoploss_takeprofit_localdb(
                                                    sl,
                                                    SlTpOrderType::StopLoss
                                                );
                                }
                            }
                        },
                        None => {
                            println!("sl is none");
                        }
                    }
                    match sltp.tp {
                        Some(tp) => match self.position_type {
                            PositionType::LONG => {
                                if tp <= current_price {
                                    payment = self.calculatepayment_localdb(
                                        current_price,
                                        get_fee(FeeType::FilledOnMarket),
                                    );
                                    orderstatus = OrderStatus::SETTLED;
                                } else {
                                    let order_caluculated =
                                                self.set_execution_price_for_limit_order_stoploss_takeprofit_localdb(
                                                    tp,
                                                    SlTpOrderType::TakeProfit
                                                );
                                }
                            }
                            PositionType::SHORT => {
                                if tp >= current_price {
                                    payment = self.calculatepayment_localdb(
                                        current_price,
                                        get_fee(FeeType::FilledOnMarket),
                                    );
                                    orderstatus = OrderStatus::SETTLED;
                                } else {
                                    let order_caluculated =
                                                self.set_execution_price_for_limit_order_stoploss_takeprofit_localdb(
                                                    tp,
                                                    SlTpOrderType::TakeProfit
                                                );
                                }
                            }
                        },
                        None => {
                            println!("tp is none");
                        }
                    }
                }
                None => {
                    println!("sltp is none");
                }
            },
            _ => {}
        }

        (payment, orderstatus)
    }

    pub fn set_execution_price_for_limit_order_localdb(
        &mut self,
        execution_price: f64,
    ) -> Result<(), std::io::Error> {
        match self.position_type {
            PositionType::LONG => {
                let mut add_to_limit_order_list = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
                match add_to_limit_order_list.add(self.uuid, (execution_price * 10000.0) as i64) {
                    Ok(()) => {
                        drop(add_to_limit_order_list);
                        Event::new(
                            Event::SortedSetDBUpdate(SortedSetCommand::AddCloseLimitPrice(
                                self.uuid.clone(),
                                execution_price.clone(),
                                self.position_type.clone(),
                            )),
                            format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        return Ok(());
                    }
                    Err(_) => {
                        let result = add_to_limit_order_list
                            .update(self.uuid, (execution_price * 10000.0) as i64);
                        drop(add_to_limit_order_list);
                        Event::new(
                            Event::SortedSetDBUpdate(SortedSetCommand::UpdateCloseLimitPrice(
                                self.uuid.clone(),
                                execution_price.clone(),
                                self.position_type.clone(),
                            )),
                            format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        return result;
                    }
                }
            }
            PositionType::SHORT => {
                let mut add_to_limit_order_list = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
                match add_to_limit_order_list.add(self.uuid, (execution_price * 10000.0) as i64) {
                    Ok(()) => {
                        drop(add_to_limit_order_list);
                        Event::new(
                            Event::SortedSetDBUpdate(SortedSetCommand::AddCloseLimitPrice(
                                self.uuid.clone(),
                                execution_price.clone(),
                                self.position_type.clone(),
                            )),
                            format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        return Ok(());
                    }
                    Err(_) => {
                        let result = add_to_limit_order_list
                            .update(self.uuid, (execution_price * 10000.0) as i64);
                        drop(add_to_limit_order_list);
                        Event::new(
                            Event::SortedSetDBUpdate(SortedSetCommand::UpdateCloseLimitPrice(
                                self.uuid.clone(),
                                execution_price.clone(),
                                self.position_type.clone(),
                            )),
                            format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                            CORE_EVENT_LOG.clone().to_string(),
                        );
                        return result;
                    }
                }
            }
        }
    }
    pub fn set_execution_price_for_limit_order_stoploss_takeprofit_localdb(
        &mut self,
        sltp_price: f64,
        sltp_type: SlTpOrderType,
    ) -> Result<(), std::io::Error> {
        match sltp_type {
            SlTpOrderType::StopLoss => match self.position_type {
                PositionType::SHORT => {
                    let mut add_to_limit_order_list = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
                    match add_to_limit_order_list.add(self.uuid, (sltp_price * 10000.0) as i64) {
                        Ok(()) => {
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::AddStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            let result = add_to_limit_order_list
                                .update(self.uuid, (sltp_price * 10000.0) as i64);
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::UpdateStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return result;
                        }
                    }
                }
                PositionType::LONG => {
                    let mut add_to_limit_order_list = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();
                    match add_to_limit_order_list.add(self.uuid, (sltp_price * 10000.0) as i64) {
                        Ok(()) => {
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::AddStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            let result = add_to_limit_order_list
                                .update(self.uuid, (sltp_price * 10000.0) as i64);
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::UpdateStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return result;
                        }
                    }
                }
            },
            SlTpOrderType::TakeProfit => match self.position_type {
                PositionType::LONG => {
                    let mut add_to_limit_order_list = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
                    match add_to_limit_order_list.add(self.uuid, (sltp_price * 10000.0) as i64) {
                        Ok(()) => {
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::AddTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            let result = add_to_limit_order_list
                                .update(self.uuid, (sltp_price * 10000.0) as i64);
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::UpdateTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return result;
                        }
                    }
                }
                PositionType::SHORT => {
                    let mut add_to_limit_order_list = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();
                    match add_to_limit_order_list.add(self.uuid, (sltp_price * 10000.0) as i64) {
                        Ok(()) => {
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::AddTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("AddCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            let result = add_to_limit_order_list
                                .update(self.uuid, (sltp_price * 10000.0) as i64);
                            drop(add_to_limit_order_list);
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::UpdateTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        sltp_price.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("UpdateCloseLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return result;
                        }
                    }
                }
            },
        }
    }

    pub fn calculatepayment_localdb(&mut self, current_price: f64, fee: f64) -> f64 {
        // returns payment
        let ordertx = self.clone();
        let margindifference = self.available_margin - self.initial_margin;
        let u_pnl = unrealizedpnl(
            &self.position_type,
            self.positionsize,
            self.entryprice,
            current_price,
        );
        //calculate fee by AM*leverage*fee = fee deducted from payment
        let fee_value: f64 = calculate_fee_on_open_order(fee, self.positionsize, current_price);
        // println!(
        //     "unrealizedpnl: {:?} \n round {:?} \n margindifference :{:?} \n round  {:?}",
        //     u_pnl,
        //     u_pnl.round(),
        //     margindifference,
        //     margindifference.round()
        // );
        let payment = u_pnl.round() + margindifference.round() - fee_value;
        self.order_status = OrderStatus::SETTLED;
        self.available_margin = (self.initial_margin + payment).round();
        self.settlement_price = current_price;
        self.unrealized_pnl = u_pnl.round();
        self.fee_settled = fee_value;
        payment
    }

    pub fn order_remove_from_localdb(&self) {
        let ordertx = self.clone();
        PositionSizeLog::remove_order(ordertx.position_type.clone(), ordertx.positionsize.clone());
        RiskState::remove_order(
            ordertx.position_type.clone(),
            entryvalue(ordertx.initial_margin, ordertx.leverage),
        );
        match ordertx.position_type {
            PositionType::LONG => {
                let mut add_to_liquidation_list = TRADER_LP_LONG.lock().unwrap();
                let _ = add_to_liquidation_list.remove(ordertx.uuid);
                drop(add_to_liquidation_list);
                let mut remove_from_limit_order_list = TRADER_LIMIT_CLOSE_LONG.lock().unwrap();
                if let Ok((_, _)) = remove_from_limit_order_list.remove(ordertx.uuid) {
                    Event::new(
                        Event::SortedSetDBUpdate(SortedSetCommand::RemoveCloseLimitPrice(
                            ordertx.uuid,
                            PositionType::LONG,
                        )),
                        format!("RemoveCloseLimitPrice-{}", ordertx.uuid),
                        CORE_EVENT_LOG.clone().to_string(),
                    );
                }
                drop(remove_from_limit_order_list);
            }
            PositionType::SHORT => {
                let mut add_to_liquidation_list = TRADER_LP_SHORT.lock().unwrap();
                let _ = add_to_liquidation_list.remove(ordertx.uuid);
                drop(add_to_liquidation_list);
                let mut remove_from_limit_order_list = TRADER_LIMIT_CLOSE_SHORT.lock().unwrap();
                if let Ok((_, _)) = remove_from_limit_order_list.remove(ordertx.uuid) {
                    Event::new(
                        Event::SortedSetDBUpdate(SortedSetCommand::RemoveCloseLimitPrice(
                            ordertx.uuid,
                            PositionType::SHORT,
                        )),
                        format!("RemoveCloseLimitPrice-{}", ordertx.uuid),
                        CORE_EVENT_LOG.clone().to_string(),
                    );
                }
                drop(remove_from_limit_order_list);
            }
        }
    }
    pub fn cancelorder_localdb(&mut self) -> (bool, OrderStatus) {
        let result: Result<(Uuid, i64), std::io::Error>;
        match self.order_type {
            OrderType::LIMIT => match self.position_type {
                PositionType::LONG => {
                    let mut remove_from_open_order_list = TRADER_LIMIT_OPEN_LONG.lock().unwrap();
                    result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            self.order_status = OrderStatus::CANCELLED;
                            RiskState::remove_pending_order(
                                self.position_type.clone(),
                                entryvalue(self.initial_margin, self.leverage),
                            );
                            Event::new(
                                Event::SortedSetDBUpdate(SortedSetCommand::RemoveOpenLimitPrice(
                                    self.uuid.clone(),
                                    self.position_type.clone(),
                                )),
                                format!("RemoveOpenLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return (true, OrderStatus::CANCELLED);
                        }
                        Err(_) => {
                            return (false, self.order_status.clone());
                        }
                    }
                }
                PositionType::SHORT => {
                    let mut remove_from_open_order_list = TRADER_LIMIT_OPEN_SHORT.lock().unwrap();
                    result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            self.order_status = OrderStatus::CANCELLED;
                            RiskState::remove_pending_order(
                                self.position_type.clone(),
                                entryvalue(self.initial_margin, self.leverage),
                            );
                            Event::new(
                                Event::SortedSetDBUpdate(SortedSetCommand::RemoveOpenLimitPrice(
                                    self.uuid.clone(),
                                    self.position_type.clone(),
                                )),
                                format!("RemoveOpenLimitPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            return (true, OrderStatus::CANCELLED);
                        }
                        Err(_) => {
                            return (false, self.order_status.clone());
                        }
                    }
                }
            },
            _ => {
                return (false, self.order_status.clone());
            }
        }
    }
    pub fn cancel_sltp_order(
        &mut self,
        sltp_type: SlTpOrderCancel,
    ) -> (bool, OrderStatus, bool, OrderStatus) {
        let result: Result<(Uuid, i64), std::io::Error>;
        let mut cancel_status_sl: bool = false;
        let mut cancel_status_tp: bool = false;
        let mut order_status_sl: OrderStatus = OrderStatus::PENDING;
        let mut order_status_tp: OrderStatus = OrderStatus::PENDING;
        if sltp_type.sl {
            match self.position_type {
                PositionType::SHORT => {
                    let mut remove_from_open_order_list = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
                    result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            // self.order_status = OrderStatus::CANCELLED;
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::RemoveStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("RemoveStopLossCloseLIMITPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            // return (true, OrderStatus::CANCELLED);
                            cancel_status_sl = true;
                            order_status_sl = OrderStatus::CancelledStopLoss;
                        }
                        Err(_) => {
                            cancel_status_sl = false;
                            order_status_sl = OrderStatus::PENDING;
                        }
                    }
                }
                PositionType::LONG => {
                    let mut remove_from_open_order_list = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();
                    result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            // self.order_status = OrderStatus::CANCELLED;
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::RemoveStopLossCloseLIMITPrice(
                                        self.uuid.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("RemoveStopLossCloseLIMITPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            // return (true, OrderStatus::CANCELLED);
                            cancel_status_sl = true;
                            order_status_sl = OrderStatus::CancelledStopLoss;
                        }
                        Err(_) => {
                            cancel_status_sl = false;
                            order_status_sl = OrderStatus::PENDING;
                        }
                    }
                }
            }
        }
        if sltp_type.tp {
            match self.position_type {
                PositionType::LONG => {
                    let mut remove_from_open_order_list = TRADER_SLTP_CLOSE_LONG.lock().unwrap();
                    let result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            // self.order_status = OrderStatus::CANCELLED;
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::RemoveTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("RemoveTakeProfitCloseLIMITPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            // return (true, OrderStatus::CANCELLED);
                            cancel_status_tp = true;
                            order_status_tp = OrderStatus::CancelledTakeProfit;
                        }
                        Err(_) => {
                            cancel_status_tp = false;
                            order_status_tp = OrderStatus::PENDING;
                        }
                    }
                }
                PositionType::SHORT => {
                    let mut remove_from_open_order_list = TRADER_SLTP_CLOSE_SHORT.lock().unwrap();
                    let result = remove_from_open_order_list.remove(self.uuid);
                    drop(remove_from_open_order_list);
                    match result {
                        Ok((_, _)) => {
                            // self.order_status = OrderStatus::CANCELLED;
                            Event::new(
                                Event::SortedSetDBUpdate(
                                    SortedSetCommand::RemoveTakeProfitCloseLIMITPrice(
                                        self.uuid.clone(),
                                        self.position_type.clone(),
                                    ),
                                ),
                                format!("RemoveTakeProfitCloseLIMITPrice-{}", self.uuid.clone()),
                                CORE_EVENT_LOG.clone().to_string(),
                            );
                            // return (true, OrderStatus::CANCELLED);
                            cancel_status_tp = true;
                            order_status_tp = OrderStatus::CancelledTakeProfit;
                        }
                        Err(_) => {
                            cancel_status_tp = false;
                            order_status_tp = OrderStatus::PENDING;
                        }
                    }
                }
            }
        }
        (
            cancel_status_sl,
            order_status_sl,
            cancel_status_tp,
            order_status_tp,
        )
    }

    pub fn liquidate(&mut self, current_price: f64) -> f64 {
        let ordertx = self.clone();
        self.settlement_price = current_price;
        self.liquidation_price = current_price;
        self.available_margin = 0.0;
        -self.initial_margin.clone()
    }

    pub fn serialize(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap();
        serialized
    }

    pub fn deserialize(json: &String) -> Self {
        let deserialized: TraderOrder = serde_json::from_str(json).unwrap();
        deserialized
    }

    pub fn uuid_to_byte(&self) -> Vec<u8> {
        match bincode::serialize(&self.uuid) {
            Ok(uuid_v_u8) => uuid_v_u8,
            Err(_) => Vec::new(),
        }
    }
}
