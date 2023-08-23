use super::rpc_types::*;
use crate::relayer::types::*;
use quisquislib::accounts::SigmaProof;
use quisquislib::ristretto::RistrettoPublicKey;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
use zkschnorr::Signature;
use zkvm::zkos_types::{Input, Output};

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
    pub public_key: String,
    pub signature: Signature, //quisquis signature  //canceltradeorder sign
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZkosQueryMsg {
    pub public_key: String,
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
pub struct QueryTraderOrderZkos {
    pub query_trader_order: QueryTraderOrder,
    pub msg: ZkosQueryMsg,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryLendOrderZkos {
    pub query_lend_order: QueryLendOrder,
    pub msg: ZkosQueryMsg,
}

impl CreateTraderOrderZkos {
    pub fn new(
        create_trader_order: CreateTraderOrder,
        input: ZkosCreateOrder,
    ) -> CreateTraderOrderZkos {
        CreateTraderOrderZkos {
            create_trader_order,
            input,
        }
    }
    pub fn encode_as_hex_string(&self) -> String {
        let byt = bincode::serialize(&self).unwrap();
        hex::encode(&byt)
    }

    // pub fn decode_from_hex_sting(data: String) -> Result<Self, std::io::error> {
    //     bincode::deserialize(&hex::decode(&data))

    //     // Err(std::io::Error::new(
    //     //     std::io::ErrorKind::Other,
    //     //     "order not found",
    //     // ))
    // }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ByteRec {
    pub data: String,
}
