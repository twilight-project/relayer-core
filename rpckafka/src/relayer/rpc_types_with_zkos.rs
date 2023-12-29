use super::rpc_types::*;
use quisquislib::accounts::SigmaProof;
use relayerwalletlib::verify_client_message::{
    verify_query_order, verify_settle_requests, verify_trade_lend_order,
};
use relayerwalletlib::zkoswalletlib::relayer_types;
use serde_derive::{Deserialize, Serialize};
// use transaction::verify_relayer::{
//     verify_query_order, verify_settle_requests, verify_trade_lend_order,
// };
use zkschnorr::Signature;
use zkvm::zkos_types::{Input, Output};

/********* zkos wasm msg Start */
// To create zkos Wasm request for new Trade and Lend Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosCreateOrder {
    pub input: Input,         //coin type input
    pub output: Output,       // memo type output
    pub signature: Signature, //quisquis signature
    pub proof: SigmaProof,
}

// To create zkos Wasm request to Settle Trade and Lend Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosSettleMsg {
    pub input: Input,         //memo type input
    pub signature: Signature, //quisquis signature
}

// To create zkos Wasm request for cancel any Limit Trade Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosCancelMsg {
    pub public_key: String,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}

// To create zkos Wasm request for query Trade and Lend Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosQueryMsg {
    pub public_key: String,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}

/********* zkos wasm msg End */

/***** Relayer RPC with Zkos Integration Start */

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateTraderOrderZkos {
    pub create_trader_order: CreateTraderOrder,
    pub input: ZkosCreateOrder,
}
impl CreateTraderOrderZkos {
    pub fn verify_order(&mut self) -> Result<bool, &'static str> {
        verify_trade_lend_order(
            self.input.input.clone(),
            self.input.output.clone(),
            self.input.signature.clone(),
            self.input.proof.clone(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateLendOrderZkos {
    pub create_lend_order: CreateLendOrder,
    pub input: ZkosCreateOrder,
}
impl CreateLendOrderZkos {
    pub fn verify_order(&mut self) -> Result<bool, &'static str> {
        verify_trade_lend_order(
            self.input.input.clone(),
            self.input.output.clone(),
            self.input.signature.clone(),
            self.input.proof.clone(),
        )
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteTraderOrderZkos {
    pub execute_trader_order: ExecuteTraderOrder,
    pub msg: ZkosSettleMsg,
}
impl ExecuteTraderOrderZkos {
    pub fn verify_order(&mut self) -> Result<(), &'static str> {
        verify_settle_requests(self.msg.input.clone(), self.msg.signature.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteLendOrderZkos {
    pub execute_lend_order: ExecuteLendOrder,
    pub msg: ZkosSettleMsg,
}
impl ExecuteLendOrderZkos {
    pub fn verify_order(&mut self) -> Result<(), &'static str> {
        verify_settle_requests(self.msg.output.clone(), self.msg.signature.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CancelTraderOrderZkos {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosCancelMsg,
}

impl CancelTraderOrderZkos {
    pub fn verify_query(&mut self) -> Result<(), &'static str> {
        verify_query_order(
            serde_json::from_str(&self.msg.public_key.clone()).unwrap(),
            self.msg.signature.clone(),
            &bincode::serialize(&self.cancel_trader_order).unwrap(),
        )
    }
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ByteRec {
    pub data: String,
}

/***** Relayer RPC with Zkos Integration End */

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequestResponse {
    pub msg: String,
    pub id_key: String,
}

impl RequestResponse {
    pub fn new(msg: String, id_key: String) -> Self {
        RequestResponse { msg, id_key }
    }
}
