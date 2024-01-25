use relayerwalletlib::relayer::*;
use relayerwalletlib::zkoswalletlib::keys_management::*;

lazy_static! {
    pub static ref RELAYER_WALLET_IV: &[u8; 16] =
        std::env::var("RELAYER_WALLET_IV").expect("missing environment variable RELAYER_WALLET_IV");
    pub static ref RELAYER_WALLET_SEED: String = std::env::var("RELAYER_WALLET_SEED")
        .expect("missing environment variable RELAYER_WALLET_SEED");
    pub static ref RELAYER_WALLET_PATH: String = std::env::var("RELAYER_WALLET_PATH")
        .expect("missing environment variable RELAYER_WALLET_PATH");
    pub static ref RELAYER_WALLET_PASSWORD: &[u8; 16] = std::env::var("RELAYER_WALLET_PASSWORD")
        .expect("missing environment variable RELAYER_WALLET_PASSWORD");
}

pub fn init_relayer_wallet() {
    dotenv::dotenv().expect("Failed loading dotenv");
    initialize_relayer_wallet(
        RELAYER_WALLET_PASSWORD,
        *RELAYER_WALLET_IV,
        RELAYER_WALLET_SEED,
        RELAYER_WALLET_PATH,
    );
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_create_wallet() {
        println!("show key: {:?}", init_relayer_wallet());
    }
}
