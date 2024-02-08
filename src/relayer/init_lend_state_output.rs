use address::{Address, AddressType};
use quisquislib::ristretto::RistrettoPublicKey;
use quisquislib::ristretto::RistrettoSecretKey;
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
    // // nonce 1 200BTC
    // let hex = "0200000002000000040000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a000000000000003063613031333835653865396365613839613138376530623061623262316361616637313364663532376163646238386637363433353864386436353764623334636132646637656236633336373364326337623334633833366535633562623466633164393164663331383561353736303834313334626438666631323064316239653265656265660100000000000000575293e9040000000000000000000000f1d66816297466302e5e6d4e2d1ed902247da90f534f5131f26c64ed56345b01010100000000000000020000000100000000000000a91d200000000000000000000000000069616e7a1540fba2efc89ff57712fa3168ca6e48f71d2ce645c604cf87ac1e0f00000000";
    // // nonce 5 200BTC
    // let hex = "0200000002000000050000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a000000000000003063613031333835653865396365613839613138376530623061623262316361616637313364663532376163646238386637363433353864386436353764623334636132646637656236633336373364326337623334633833366535633562623466633164393164663331383561353736303834313334626438666631323064316239653265656265660100000000000000bce8b2fb040000000000000000000000e6799657180f038802406d8f817a466f3e564a2151f14f4e526393be1e69860001010000000000000002000000010000000000000025942000000000000000000000000000a645301fdd6238ec5b366b577aabf83f980176c2024f62968c2030ab10f8c00a00000000";
    // // // nonce 6 200BTC
    // let hex = "0200000002000000060000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a000000000000003063613031333835653865396365613839613138376530623061623262316361616637313364663532376163646238386637363433353864386436353764623334636132646637656236633336373364326337623334633833366535633562623466633164393164663331383561353736303834313334626438666631323064316239653265656265660100000000000000895c93e9040000000000000000000000a2b8def6ed744d51f0eb50b7f84f31d3f8f82dc1fe1327e17269b0ed05ce5804010100000000000000020000000100000000000000a91d20000000000000000000000000008837d66e08e266461e912074543682b1ab7328c2b69063886fe8548246f84a0000000000";
    // // nonce 7 200BTC
    let hex = "0200000002000000070000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a000000000000003063613031333835653865396365613839613138376530623061623262316361616637313364663532376163646238386637363433353864386436353764623334636132646637656236633336373364326337623334633833366535633562623466633164393164663331383561353736303834313334626438666631323064316239653265656265660100000000000000f8affdaa04000000000000000000000080947ab1c064b093aa33ccce420fac35482ee548e6a8ba9a391dd61c5cdac30d01010000000000000002000000010000000000000080841e00000000000000000000000000a1f93d317b674cf08977a3e49208816bf725956f02c3efce7ce5c181ead3eb0e00000000";

    let hex_decode = hex::decode(hex).unwrap();
    let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
    last_state_output
}
// pub fn last_state_output_fixed() -> Output {
//     let hex = "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306331346665623063623061363135393934613234613139333663356331383035306564666339643630333166343035386662636637663139633866616232323430363838643161366131623536613232613833343231353137303935663832393034326362346637363735333866643539303339646661316131613533663430383333313365623634010000000000000000ca9a3b0000000000000000000000003b0396f868458a935386be6cc919f673231f50cc2b4203f5afb8054442a14f0f010100000000000000020000000100000000000000a08601000000000000000000000000004989bcddd1acce90b99569b9fcdff51e85f5e6f40217b9e87fbfe254ce20970700000000";
//     let hex_decode = hex::decode(hex).unwrap();
//     let last_state_output: Output = bincode::deserialize(&hex_decode).unwrap();
//     last_state_output
// }

pub fn last_state_output_string() -> String {
    let hex = "0200000002000000010000002a000000000000003138663265626461313733666663366164326533623464336133383634613936616538613666376533308a00000000000000306331346665623063623061363135393934613234613139333663356331383035306564666339643630333166343035386662636637663139633866616232323430363838643161366131623536613232613833343231353137303935663832393034326362346637363735333866643539303339646661316131613533663430383333313365623634010000000000000000ca9a3b0000000000000000000000003b0396f868458a935386be6cc919f673231f50cc2b4203f5afb8054442a14f0f010100000000000000020000000100000000000000a08601000000000000000000000000004989bcddd1acce90b99569b9fcdff51e85f5e6f40217b9e87fbfe254ce20970700000000".to_string();
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
    use relayerwalletlib::zkoswalletlib::util::create_output_state_for_trade_lend_order;
    use zkvm::zkos_types::Output;
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
