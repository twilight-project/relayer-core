use crate::config::*;
use crate::relayer::*;
use uuid::Uuid;
pub fn new_trader_order_insert_sql_query(ordertx: TraderOrder) {
    let query = format!("INSERT INTO public.newtraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},'{}',{},{},{},{},{},{},{},{},{});",
    &ordertx.uuid,
    &ordertx.account_id ,
    &ordertx.position_type ,
    &ordertx.order_status ,
    &ordertx.order_type ,
    &ordertx.entryprice ,
    &ordertx.execution_price ,
    &ordertx.positionsize ,
    &ordertx.leverage ,
    &ordertx.initial_margin ,
    &ordertx.available_margin ,
    ServerTime::new(ordertx.timestamp).iso,
    &ordertx.bankruptcy_price ,
    &ordertx.bankruptcy_value ,
    &ordertx.maintenance_margin ,
    &ordertx.liquidation_price ,
    &ordertx.unrealized_pnl,
    &ordertx.settlement_price,
    &ordertx.entry_nonce,
    &ordertx.exit_nonce,
    &ordertx.entry_sequence,
);
    let psql_insert_order_pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
    psql_insert_order_pool.execute(move || {
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
        client.execute(&query, &[]).unwrap();
    });
    drop(psql_insert_order_pool);
}

pub fn pending_trader_order_insert_sql_query(ordertx: TraderOrder) {
    let query = format!("INSERT INTO public.pendinglimittraderorder(uuid, account_id, position_type,  order_status, order_type, entryprice, execution_price,positionsize, leverage, initial_margin, available_margin, timestamp, bankruptcy_price, bankruptcy_value, maintenance_margin, liquidation_price, unrealized_pnl, settlement_price, entry_nonce, exit_nonce, entry_sequence) VALUES ('{}','{}','{:#?}','{:#?}','{:#?}',{},{},{},{},{},{},'{}',{},{},{},{},{},{},{},{},{});",
    &ordertx.uuid,
    &ordertx.account_id ,
    &ordertx.position_type ,
    &ordertx.order_status ,
    &ordertx.order_type ,
    &ordertx.entryprice ,
    &ordertx.execution_price ,
    &ordertx.positionsize ,
    &ordertx.leverage ,
    &ordertx.initial_margin ,
    &ordertx.available_margin ,
    ServerTime::new(ordertx.timestamp).iso,
    &ordertx.bankruptcy_price ,
    &ordertx.bankruptcy_value ,
    &ordertx.maintenance_margin ,
    &ordertx.liquidation_price ,
    &ordertx.unrealized_pnl,
    &ordertx.settlement_price,
    &ordertx.entry_nonce,
    &ordertx.exit_nonce,
    &ordertx.entry_sequence,
);
    let psql_insert_order_pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
    psql_insert_order_pool.execute(move || {
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
        client.execute(&query, &[]).unwrap();
    });
    drop(psql_insert_order_pool);
}

pub fn pending_trader_order_update_sql_query(orderid: Uuid) {
    let pool = THREADPOOL_PSQL_ORDER_INSERT_QUEUE.lock().unwrap();
    pool.execute(move || {
        let query = format!(
            "UPDATE public.pendinglimittraderorder
            SET order_status='{:#?}'
            WHERE uuid='{}';",
            OrderStatus::FILLED,
            &orderid
        );
        let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();
        client.execute(&query, &[]).unwrap();
    });
    drop(pool);
}
