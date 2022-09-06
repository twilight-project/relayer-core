#![allow(dead_code)]
#![allow(unused_imports)]
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
lazy_static! {
       /// Static Globle PostgreSQL Pool connection
    ///
    /// https://stackoverflow.com/questions/63150183/how-do-i-keep-a-global-postgres-connection
    ///
    pub static ref POSTGRESQL_POOL_CONNECTION: r2d2::Pool<PostgresConnectionManager<NoTls>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        // POSTGRESQL_URL
        let postgresql_url =
            std::env::var("POSTGRESQL_URL").expect("missing environment variable POSTGRESQL_URL");
        let manager = PostgresConnectionManager::new(
            // TODO: PLEASE MAKE SURE NOT TO USE HARD CODED CREDENTIALS!!!
            postgresql_url.parse().unwrap(),
            NoTls,
        );
        r2d2::Pool::new(manager).unwrap()
    };
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
