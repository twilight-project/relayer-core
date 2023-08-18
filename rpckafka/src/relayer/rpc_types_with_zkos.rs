use super::rpc_types::*;
use crate::relayer::types::*;
use elgamalsign::Signature;
use quisquislib::accounts::SigmaProof;
use quisquislib::ristretto::RistrettoPublicKey;
use serde_derive::{Deserialize, Serialize};
use transaction::{Input, Output};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosCreateOrder {
    pub input: Input,         //coin type input
    pub output: Output,       // memo type output
    pub signature: Signature, //quisquis signature
    pub proof: SigmaProof,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosSettleMsg {
    pub input: Input,         //memo type input
    pub signature: Signature, //quisquis signature
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosCancelMsg {
    pub public_key: RistrettoPublicKey,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosQueryMsg {
    pub public_key: RistrettoPublicKey,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateTraderOrderZkos {
    pub create_trader_order: CreateTraderOrder,
    pub input: ZkosCreateOrder,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateLendOrderZkos {
    pub create_lend_order: CreateLendOrder,
    pub input: ZkosCreateOrder,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteTraderOrderZkos {
    pub execute_trader_order: ExecuteTraderOrder,
    pub msg: ZkosSettleMsg,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteLendOrderZkos {
    pub execute_lend_order: ExecuteLendOrder,
    pub msg: ZkosSettleMsg,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CancelTraderOrderZkos {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosCancelMsg,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryTraderOrder {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosQueryMsg,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryLendOrder {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosQueryMsg,
}
