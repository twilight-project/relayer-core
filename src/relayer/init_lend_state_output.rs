use address::{Address, AddressType};
use quisquislib::keys::PublicKey;
use quisquislib::keys::SecretKey;
use quisquislib::ristretto::RistrettoPublicKey;
use quisquislib::ristretto::RistrettoSecretKey;
use rand::rngs::OsRng;
use relayerwalletlib::zkoswalletlib::keys_management::*;
use zkvm::zkos_types::Output;
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

pub fn init_relayer_wallet() {
    dotenv::dotenv().expect("Failed loading dotenv");
    println!(
        "RELAYER_WALLET_PASSWORD {:?} \n
    RELAYER_WALLET_IV {:?} \n
    RELAYER_WALLET_SEED  {:?} \n
    RELAYER_WALLET_PATH {:?} ",
        RELAYER_WALLET_PASSWORD.to_string().as_bytes(),
        RELAYER_WALLET_IV.to_string(),
        &RELAYER_WALLET_SEED.to_string().as_bytes(),
        Some(RELAYER_WALLET_PATH.to_string())
    );

    let password = b"your_password_he";
    let iv = b"your_password_he"; // Use a secure way to handle the password
    let seed =
        "UTQTkXOhF+D550+JW9A1rEQaXDtX9CYqbDOFqCY44S8ZYMoVzj8tybCB/Okwt+pblM0l3t9/eEJtfBpPcJwfZw==";

    // init_wallet(
    //     RELAYER_WALLET_PASSWORD.to_string().as_bytes(),
    //     RELAYER_WALLET_IV.to_string(),
    //     &RELAYER_WALLET_SEED.to_string().as_bytes(),
    //     Some(RELAYER_WALLET_PATH.to_string()),
    // );
    // init_wallet(
    //     RELAYER_WALLET_PASSWORD.to_string().as_bytes(),
    //     RELAYER_WALLET_PATH.to_string(),
    //     RELAYER_WALLET_IV.to_string().as_bytes(),
    //     Some(RELAYER_WALLET_SEED.to_string()),
    // );
    init_wallet(
        password,
        RELAYER_WALLET_PATH.to_string(),
        iv,
        Some(seed.to_string().clone()),
    );
}

pub fn last_state_output_fixed() -> Output {
    let hex = "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306363636131366235633036303734353634666262336366316233333638326562316663303966313133333266393333303031636638383436623838383634353636623663353131303536623434636537316134326362363937356264323462613662373561336638363466313236396266626238666136306363396261636633643933333939313731010000000000000000ca9a3b000000000000000000000000f6de14e6e9a756ea6120b26ebc74d235e1a7e1dfbd4469fdc025925f539421040101000000000000000200000001000000000000000a0000000000000000000000000000001999b2e9c23c0689402293e15eea45aacf9966ac8e85619734886de36db56d0f00000000";
    let hex_decode = hex::decode(hex).unwrap();
    let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
    last_state_output
}

pub fn last_state_output_string() -> String {
    let hex = "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306363636131366235633036303734353634666262336366316233333638326562316663303966313133333266393333303031636638383436623838383634353636623663353131303536623434636537316134326362363937356264323462613662373561336638363466313236396266626238666136306363396261636633643933333939313731010000000000000000ca9a3b000000000000000000000000f6de14e6e9a756ea6120b26ebc74d235e1a7e1dfbd4469fdc025925f539421040101000000000000000200000001000000000000000a0000000000000000000000000000001999b2e9c23c0689402293e15eea45aacf9966ac8e85619734886de36db56d0f00000000".to_string();
    hex
}

pub fn get_sk_from_fixed_wallet() -> RistrettoSecretKey {
    let seed =
        "UTQTkXOhF+D550+JW9A1rEQaXDtX9CYqbDOFqCY33S8ZYMoVzj8tybCB/Okwt+cblM0l3a8/eEJtfBpPcJwfZw++";
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
    use zkvm::zkos_types::{Input, Output};
    #[test]
    fn test_create_wallet() {
        init_relayer_wallet();
    }

    #[test]
    fn test_get_outputhex_to_output() {
        let hex = "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306363636131366235633036303734353634666262336366316233333638326562316663303966313133333266393333303031636638383436623838383634353636623663353131303536623434636537316134326362363937356264323462613662373561336638363466313236396266626238666136306363396261636633643933333939313731010000000000000000ca9a3b000000000000000000000000f6de14e6e9a756ea6120b26ebc74d235e1a7e1dfbd4469fdc025925f539421040101000000000000000200000001000000000000000a0000000000000000000000000000001999b2e9c23c0689402293e15eea45aacf9966ac8e85619734886de36db56d0f00000000";
        let hex_decode = hex::decode(hex).unwrap();
        let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
        println!("output data: {:?}", last_state_output);
    }

    #[test]
    fn test_get_sk_from_fixed_wallet() {
        println!(
            "test_get_sk_from_fixed_wallet: {:?}",
            get_sk_from_fixed_wallet()
        );
    }
    #[test]
    fn get_pk_from_fixed_wallet() {
        println!("get_pk_from_fixed_wallet: {:?}", get_pk_from_fixed_wallet());
    }
}
