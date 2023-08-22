use crate::config::POSTGRESQL_POOL_CONNECTION;
use crate::relayer::*;
use elgamalsign::Signature;
use quisquislib::accounts::SigmaProof;
use quisquislib::ristretto::RistrettoPublicKey;
use serde_derive::{Deserialize, Serialize};
use transaction::{Input, Output};
use uuid::{uuid, Uuid};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosQueryMsg {
    pub public_key: RistrettoPublicKey,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryTraderOrderZkos {
    pub query_trader_order: QueryTraderOrder,
    pub msg: ZkosQueryMsg,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryLendOrderZkos {
    pub query_lend_order: QueryLendOrder,
    pub msg: ZkosQueryMsg,
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

pub fn get_order_details_by_account_id(account: String) -> Result<TraderOrder, std::io::Error> {
    let query = format!(" SELECT id, uuid, account_id, position_type, order_status, order_type, entryprice, execution_price, positionsize, leverage, initial_margin, available_margin, \"timestamp\", bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence
	FROM public.trader_order where account_id='{}' Order By  timestamp desc Limit 1 ;",account);
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
    for row in client.query(&query, &[]).unwrap() {
        let response = TraderOrder {
            uuid: serde_json::from_str(row.get("uuid")).unwrap(),
            account_id: row.get("account_id"),
            position_type: serde_json::from_str(row.get("position_type")).unwrap(),
            order_status: serde_json::from_str(row.get("order_status")).unwrap(),
            order_type: serde_json::from_str(row.get("order_type")).unwrap(),
            entryprice: row.get("entryprice"),
            execution_price: row.get("execution_price:"),
            positionsize: row.get("positionsize"),
            leverage: row.get("leverage"),
            initial_margin: row.get("initial_margin"),
            available_margin: row.get("available_margin"),
            timestamp: row.get("timestamp"),
            bankruptcy_price: row.get("bankruptcy_price"),
            bankruptcy_value: row.get("bankruptcy_value"),
            maintenance_margin: row.get("maintenance_margin"),
            liquidation_price: row.get("liquidation_price"),
            unrealized_pnl: row.get("unrealized_pnl"),
            settlement_price: row.get("settlement_price"),
            entry_nonce: serde_json::from_str(row.get("entry_nonce")).unwrap(),
            exit_nonce: serde_json::from_str(row.get("exit_nonce")).unwrap(),
            entry_sequence: serde_json::from_str(row.get("entry_sequence")).unwrap(),
        };
        return Ok(response);
    }
    return Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "data not found",
    ));
}
