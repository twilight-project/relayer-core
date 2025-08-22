use crate::config::*;
use crate::relayer::threadpool::ThreadPool;
use std::sync::{ Arc, Mutex };
use twilight_relayer_sdk::twilight_client_sdk::programcontroller::ContractManager;

lazy_static! {
    pub static ref THREADPOOL_NORMAL_ORDER: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(12, String::from("THREADPOOL_NORMAL_ORDER"))
    );
    pub static ref THREADPOOL_NORMAL_ORDER_INSERT: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(8, String::from("THREADPOOL_NORMAL_ORDER_INSERT"))
    );
    pub static ref THREADPOOL_URGENT_ORDER: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(2, String::from("THREADPOOL_URGENT_ORDER"))
    );
    pub static ref THREADPOOL_FIFO_ORDER: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(1, String::from("THREADPOOL_FIFO_ORDER"))
    );
    pub static ref THREADPOOL_BULK_PENDING_ORDER: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(5, String::from("THREADPOOL_BULK_PENDING_ORDER"))
    );
    pub static ref THREADPOOL_BULK_PENDING_ORDER_INSERT: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(12, String::from("THREADPOOL_BULK_PENDING_ORDER_INSERT"))
    );
    pub static ref THREADPOOL_BULK_PENDING_ORDER_REMOVE: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(12, String::from("THREADPOOL_BULK_PENDING_ORDER_REMOVE"))
    );
    pub static ref THREADPOOL_ZKOS_TRADER_ORDER: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(5, String::from("THREADPOOL_ZKOS_TRADER_ORDER"))
    );
    pub static ref THREADPOOL_ZKOS_FIFO: Mutex<ThreadPool> = Mutex::new(
        ThreadPool::new(1, String::from("THREADPOOL_ZKOS_FIFO"))
    );
    pub static ref CONTRACTMANAGER: Arc<Mutex<ContractManager>> = {
        dotenv::dotenv().expect("Failed loading dotenv");
        let contract_manager = ContractManager::import_program(&WALLET_PROGRAM_PATH.clone());
        Arc::new(Mutex::new(contract_manager))
    };
}
