#![allow(dead_code)]
#![allow(unused_imports)]

lazy_static! {
    pub static ref RELAYER_VERSION: String =
        std::env::var("RelayerVersion").expect("missing environment variable RelayerVersion");
    pub static ref SNAPSHOT_VERSION: String =
        std::env::var("SnapshotVersion").expect("missing environment variable SnapshotVersion");
    pub static ref RPC_QUEUE_MODE: String =
        std::env::var("RPC_QUEUE_MODE").expect("missing environment variable RPC_QUEUE_MODE");
    pub static ref RPC_SERVER_SOCKETADDR: String = std::env::var("RPC_SERVER_SOCKETADDR")
        .expect("missing environment variable RPC_SERVER_SOCKETADDR");
    pub static ref RPC_SERVER_THREAD: usize = std::env::var("RPC_SERVER_THREAD")
        .expect("missing environment variable RPC_SERVER_THREAD")
        .parse::<usize>()
        .unwrap();
}
