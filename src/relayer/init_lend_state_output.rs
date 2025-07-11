use twilight_relayer_sdk::address::{Address, AddressType};
use twilight_relayer_sdk::quisquislib;
use twilight_relayer_sdk::quisquislib::ristretto::RistrettoPublicKey;
use twilight_relayer_sdk::quisquislib::ristretto::RistrettoSecretKey;
// use twilight_relayer_sdk::zkvm;
use twilight_relayer_sdk::zkvm::zkos_types::Output;
lazy_static! {
    pub static ref RELAYER_WALLET_IV: String =
        std::env::var("RELAYER_WALLET_IV").expect("missing environment variable RELAYER_WALLET_IV");
    pub static ref RELAYER_WALLET_SEED: String = std::env::var("RELAYER_WALLET_SEED")
        .expect("missing environment variable RELAYER_WALLET_SEED");
    pub static ref RELAYER_WALLET_PATH: String = std::env::var("RELAYER_WALLET_PATH")
        .expect("missing environment variable RELAYER_WALLET_PATH");
    pub static ref RELAYER_WALLET_PASSWORD: String = std::env::var("RELAYER_WALLET_PASSWORD")
        .expect("missing environment variable RELAYER_WALLET_PASSWORD");
}

pub fn last_state_output_string() -> String {
    let hex: String =  std::env::var("REALYER_INIT_STATE").unwrap_or("0200000002000000010000002a000000000000003138363065656636336564656531303738313738623361646236336539663836393231636161313662358a00000000000000306365303339623237666236323732376138633465343339646631346562356432316333323637346464373731323961643332656230316433373566353364633439326364356563633835343066626434353566316165646536343161653266313234383665636465636265613966346361333138393632336664333865323434336636656337323338010000000000000000c2eb0b000000000000000000000000c6efe4352f217292b29d15dcf643071915652de75119f2f33a5f92255021570c010100000000000000020000000100000000000000400d0300000000000000000000000000e82fcb55c73fea8e4e3ba481a78a458afdcb702a445885fa6d4c74c8a553130b00000000".to_string());

    hex
}

pub fn last_state_output_fixed() -> Output {
    let hex = last_state_output_string();
    let hex_decode = hex::decode(hex).unwrap();
    let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
    last_state_output
}

pub fn get_sk_from_fixed_wallet() -> RistrettoSecretKey {
    let seed = match std::env::var("RELAYER_WALLET_SEED") {
        Ok(seed) => seed,
        Err(_) => {
            println!("RELAYER_WALLET_SEED is not set, using default seed");
            "uhv30yu9rNNRH7RIEIBcN+PgZ46y7C8ebc+IvJWgzQx3vjF9JP2VJZpJzLyUfKJ0W2nue6x00pTMA69X0fERlw==".to_string()
        }
    };
    let contract_owner_sk: quisquislib::ristretto::RistrettoSecretKey =
        quisquislib::keys::SecretKey::from_bytes(seed.as_bytes());

    contract_owner_sk
}
pub fn get_pk_from_fixed_wallet() -> RistrettoPublicKey {
    let contract_owner_address = last_state_output_fixed()
        .as_output_data()
        .get_owner_address()
        .unwrap()
        .clone();

    let owner_address: Address =
        Address::from_hex(&contract_owner_address, AddressType::Standard).unwrap();

    let contract_owner_pk: RistrettoPublicKey = owner_address.into();
    contract_owner_pk
}

#[cfg(test)]
mod test {
    use super::*;
    use twilight_relayer_sdk::twilight_client_sdk::util::create_output_state_for_trade_lend_order;
    use twilight_relayer_sdk::zkvm::zkos_types::Output;

    use curve25519_dalek::scalar::Scalar;
    use twilight_relayer_sdk::twilight_client_sdk::util::get_state_info_from_output_hex;
    #[test]
    fn test_get_outputhex_to_output() {
        let tlv_init: f64 = 20048621560.0 / 100000000.0;
        let tps_init: f64 = 2000000.0;
        let nonce_init = 7;
        let hex =
            "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306339363930336661643864363461383035313830663431623739616637663739316566626563616561373266643537626631663061643733656633636134653335313037393438626538363362346334353533643565373931643266666366643765643337396461376436323766366131336263323235613635376335383430623036353136373065010000000000000000c817a8040000000000000000000000a2158ff137143bd9dad238e9d200e1003a6319a6538b65fac0f9cb2e0d2cb60201010000000000000002000000010000000000000080841e0000000000000000000000000090720732eb3652b69b2437c709aa19e07d072f44b456e0c0dff5c7a94931520c00000000";
        let hex_decode = hex::decode(hex).unwrap();
        let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
        println!("output data: {:?}", last_state_output);
        let (nonce, tlv_witness, _, tps_witness, _) =
            match get_state_info_from_output_hex(hex.to_string()) {
                Ok((nonce, tlv_witness, _tlv_blinding, tps_witness, _tps_blinding)) => (
                    nonce,
                    tlv_witness,
                    _tlv_blinding,
                    tps_witness,
                    _tps_blinding,
                ),
                Err(_arg) => (
                    nonce_init,
                    tlv_init.round() as u64,
                    Scalar::random(&mut rand::thread_rng()),
                    tps_init.round() as u64,
                    Scalar::random(&mut rand::thread_rng()),
                ),
            };
        println!(
            "nonce {:?}  tlv_witness {:?}  tps_witness {:?}",
            nonce, tlv_witness, tps_witness
        );
    }

    #[test]
    fn test_get_sk_from_fixed_wallet() {
        println!(
            "test_get_sk_from_fixed_wallet: {:?}",
            get_sk_from_fixed_wallet()
        );
    }
    #[test]
    fn test_get_pk_from_fixed_wallet() {
        println!("get_pk_from_fixed_wallet: {:?}", get_pk_from_fixed_wallet());
    }
    #[test]
    fn test_get_output_from_fixed_wallet() {
        println!(
            "get_output_from_fixed_wallet: {:?}",
            last_state_output_fixed()
                .as_output_data()
                .get_owner_address()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_get_next_state() {
        let next_output_state = create_output_state_for_trade_lend_order(
            2,
            last_state_output_fixed()
                .clone()
                .as_output_data()
                .get_script_address()
                .unwrap()
                .clone(),
            last_state_output_fixed()
                .clone()
                .as_output_data()
                .get_owner_address()
                .clone()
                .unwrap()
                .clone(),
            1100000000,
            110000,
            0,
        );
        let last_state_output = bincode::serialize(&next_output_state).unwrap();
        let hex_decode = hex::encode(last_state_output);
        println!("get_output_from_fixed_wallet: {:?}", hex_decode);
        println!("next_output_state data: {:?}", next_output_state);
    }

    #[test]
    fn test_next_state_output() {
        let tlv1 = 20048619080.0;
        let tps1 = 2000000.0;
        let nonce = 5u32;
        let last_output = last_state_output_fixed();
        let next_output_state = create_output_state_for_trade_lend_order(
            nonce,
            last_output
                .clone()
                .as_output_data()
                .get_script_address()
                .unwrap()
                .clone(),
            last_output
                .clone()
                .as_output_data()
                .get_owner_address()
                .clone()
                .unwrap()
                .clone(),
            tlv1 as u64,
            tps1 as u64,
            0,
        );
        println!("output: {:?}", next_output_state);
        println!(
            "next_ouput_hex : {}",
            hex::encode(bincode::serialize(&next_output_state).unwrap())
        );
    }
}
