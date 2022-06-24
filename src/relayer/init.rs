// mod perpetual;
// use crate::perpetual::interface::{LendOrder, OrderStatus, OrderType, PositionType, TraderOrder};
use crate::config::LOCALDB;
use crate::config::{POSTGRESQL_POOL_CONNECTION, QUESTDB_POOL_CONNECTION};
use crate::redislib::redis_db;
use crate::relayer::*;
// pub fn initprice() {
//     redis_db::set("Fee", "0.0");
//     redis_db::set("FundingRate", "0.0");
//     redis_db::set("LendNonce", "0");
//     redis_db::set("CurrentPrice", "40000.0");
//     redis_db::set("btc:price", "40000.0");
//     let mut local_storage = LOCALDB.lock().unwrap();
//     local_storage.insert("CurrentPrice", 40000.0);
//     local_storage.insert("btc:price", 40000.0);
//     local_storage.insert("FundingRate", 0.0);
//     local_storage.insert("Fee", 0.0);
//     // drop(local_storage);
//     initialize_lend_pool(100000.0, 10.0);
//     update_recent_order_from_db();
// }

pub fn init_psql() {
    match create_binance_ticker_table() {
        Ok(_) => println!("binancebtctickernew table inserted successfully"),
        Err(arg) => println!("Some Error 1 Found, {:#?}", arg),
    }
    match create_newtraderorder_table() {
        Ok(_) => println!("newtraderorder table inserted successfully"),
        Err(arg) => println!("Some Error 2 Found, {:#?}", arg),
    }
    match create_newlendorder_table() {
        Ok(_) => println!("newlendorder table inserted successfully"),
        Err(arg) => println!("Some Error 3 Found, {:#?}", arg),
    }

    match create_pendinglimittraderorder_table() {
        Ok(_) => println!("pendinglimittraderorder table inserted successfully"),
        Err(arg) => println!("Some Error 4 Found, {:#?}", arg),
    }
    match create_settlementpriceforlimitorder_table() {
        Ok(_) => println!("settlementpriceforlimitorder table inserted successfully"),
        Err(arg) => println!("Some Error 5 Found, {:#?}", arg),
    }

    match create_btcpricehistory_table() {
        Ok(_) => println!("btcpricehistory table inserted successfully"),
        Err(arg) => println!("Some Error 6 Found, {:#?}", arg),
    }
    match create_fundingratehistory_table() {
        Ok(_) => println!("fundingratehistory 7 table inserted successfully"),
        Err(arg) => println!("Some Error Found, {:#?}", arg),
    }

    match create_insert_btcprice_procedure() {
        Ok(_) => println!("insert_btcprice procedure inserted successfully"),
        Err(arg) => println!("Some Error 8 Found, {:#?}", arg),
    }
    match create_insert_fundingrate_procedure() {
        Ok(_) => println!("insert_fundingrate procedure inserted successfully"),
        Err(arg) => println!("Some Error 9 Found, {:#?}", arg),
    }
    match create_trades_history_questdb() {
        Ok(_) => println!("trades_history table inserted successfully"),
        Err(arg) => println!("Some Error 9 Found, {:#?}", arg),
    }
}

fn create_binance_ticker_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS binancebtctickernew(
        id SERIAL PRIMARY KEY
       ,e VARCHAR(14) NOT NULL
      ,TimeStamp_E BIGINT  NOT NULL
      ,s VARCHAR(7) NOT NULL
      ,c NUMERIC(50,8) NOT NULL
      ,o NUMERIC(50,8) NOT NULL
      ,h NUMERIC(50,8) NOT NULL
      ,l NUMERIC(50,8) NOT NULL
      ,v NUMERIC(150,8) NOT NULL
      ,q NUMERIC(150,8) NOT NULL
    ,topic VARCHAR(50) NOT NULL
    ,partition_msg BIGINT NOT NULL
    ,offset_msg BIGINT NOT NULL
    );
    "
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_newtraderorder_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS newtraderorder(
           uuid               VARCHAR(100) NOT NULL PRIMARY KEY
          ,account_id         TEXT NOT NULL
          ,position_type      VARCHAR(50) NOT NULL
          -- ,position_side      INT  NOT NULL
          ,order_status       VARCHAR(50) NOT NULL
          ,order_type         VARCHAR(50) NOT NULL
          ,entryprice         NUMERIC NOT NULL
          ,execution_price    NUMERIC NOT NULL
          ,positionsize       NUMERIC NOT NULL
          ,leverage           NUMERIC NOT NULL
          ,initial_margin     NUMERIC NOT NULL
          ,available_margin   NUMERIC NOT NULL
          ,timestamp          bigint  NOT NULL
          ,bankruptcy_price   NUMERIC NOT NULL
          ,bankruptcy_value   NUMERIC NOT NULL
          ,maintenance_margin NUMERIC NOT NULL
          ,liquidation_price  NUMERIC NOT NULL
          ,unrealized_pnl     NUMERIC NOT NULL
          ,settlement_price   NUMERIC NOT NULL
          ,entry_nonce       bigint  NOT NULL
          ,exit_nonce        bigint  NOT NULL
          ,entry_sequence    bigint  NOT NULL
        );
    "
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_newlendorder_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS newlendorder(
            uuid                    VARCHAR(100) NOT NULL PRIMARY KEY
           ,account_id              TEXT NOT NULL
           ,balance                 NUMERIC NOT NULL
           ,order_status            VARCHAR(50) NOT NULL
           ,order_type              VARCHAR(50) NOT NULL
           ,entry_nonce             bigint NOT NULL
           ,exit_nonce              bigint NOT NULL
           ,deposit                 NUMERIC NOT NULL
           ,new_lend_state_amount   NUMERIC NOT NULL
           ,timestamp               bigint  NOT NULL
           ,npoolshare              NUMERIC NOT NULL
           ,nwithdraw               NUMERIC NOT NULL
           ,payment                 NUMERIC NOT NULL
           ,tlv0                    NUMERIC NOT NULL
           ,tps0                    NUMERIC NOT NULL
           ,tlv1                    NUMERIC NOT NULL
           ,tps1                    NUMERIC NOT NULL
           ,tlv2                    NUMERIC NOT NULL
           ,tps2                    NUMERIC NOT NULL
           ,tlv3                    NUMERIC NOT NULL
           ,tps3                    NUMERIC NOT NULL
           ,entry_sequence          bigint  NOT NULL
         );"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_pendinglimittraderorder_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS pendinglimittraderorder(
            uuid               VARCHAR(100) NOT NULL PRIMARY KEY
           ,account_id         TEXT NOT NULL
           ,position_type      VARCHAR(50) NOT NULL
           -- ,position_side      INT  NOT NULL
           ,order_status       VARCHAR(50) NOT NULL
           ,order_type         VARCHAR(50) NOT NULL
           ,entryprice         NUMERIC NOT NULL
           ,execution_price    NUMERIC NOT NULL
           ,positionsize       NUMERIC NOT NULL
           ,leverage           NUMERIC NOT NULL
           ,initial_margin     NUMERIC NOT NULL
           ,available_margin   NUMERIC NOT NULL
           ,timestamp          bigint  NOT NULL
           ,bankruptcy_price   NUMERIC NOT NULL
           ,bankruptcy_value   NUMERIC NOT NULL
           ,maintenance_margin NUMERIC NOT NULL
           ,liquidation_price  NUMERIC NOT NULL
           ,unrealized_pnl     NUMERIC NOT NULL
           ,settlement_price   NUMERIC NOT NULL
           ,entry_nonce       bigint  NOT NULL
           ,exit_nonce        bigint  NOT NULL
           ,entry_sequence    bigint  NOT NULL
         );"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_settlementpriceforlimitorder_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS settlementpriceforlimitorder(
            id SERIAL PRIMARY KEY
           ,uuid               VARCHAR(100) NOT NULL
           ,account_id         TEXT NOT NULL
           ,position_type      VARCHAR(50) NOT NULL
           ,order_status       VARCHAR(50) NOT NULL
           ,order_type         VARCHAR(50) NOT NULL
           ,execution_price    NUMERIC NOT NULL
           ,timestamp          bigint  NOT NULL
           ,settlement_price   NUMERIC NOT NULL
         );"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_btcpricehistory_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS btcpricehistory(
            id SERIAL PRIMARY KEY
           ,price   NUMERIC NOT NULL
           ,timestamp          timestamp  NOT NULL
        );
        "
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_fundingratehistory_table() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS api.fundingratehistory(
            id SERIAL PRIMARY KEY
           ,fundingrate   NUMERIC NOT NULL
           ,price   NUMERIC NOT NULL
           ,timestamp          timestamp  NOT NULL
        );"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_insert_btcprice_procedure() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE OR REPLACE PROCEDURE public.insert_btcprice(price numeric)
        LANGUAGE SQL
        AS $$
          INSERT INTO public.btcpricehistory ( price, \"timestamp\") VALUES (price,CURRENT_TIMESTAMP);
        $$;"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
fn create_insert_fundingrate_procedure() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE OR REPLACE PROCEDURE api.insert_fundingrate(fundingrate numeric,price numeric)
        LANGUAGE SQL
        AS $$
          INSERT INTO api.fundingratehistory ( fundingrate,price, \"timestamp\") VALUES (fundingrate,price,CURRENT_TIMESTAMP);
        $$;"
    );
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}

fn create_trades_history_questdb() -> Result<(), r2d2_postgres::postgres::Error> {
    let query = format!(
        "CREATE TABLE IF NOT EXISTS 'recentorders' (
            side INT,
            price DOUBLE,
            amount DOUBLE,
            timestamp TIMESTAMP
          ) timestamp (timestamp) PARTITION BY DAY;
          "
    );
    let mut client = QUESTDB_POOL_CONNECTION.get().unwrap();

    match client.execute(&query, &[]) {
        Ok(_) => Ok(()),
        Err(arg) => Err(arg),
    }
}
