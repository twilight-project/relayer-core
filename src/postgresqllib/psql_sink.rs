#![allow(dead_code)]
use crate::config::{BinanceMiniTickerPayload, POSTGRESQL_POOL_CONNECTION};

//https://rust-lang-nursery.github.io/rust-cookbook/database/postgres.html

pub fn kafka_sink(topic: &str, partition: &i32, offset: &i64, value: BinanceMiniTickerPayload) {
    //creating static connection
    let mut client = POSTGRESQL_POOL_CONNECTION.get().unwrap();

    let query = format!("INSERT INTO public.binancebtctickernew(e,TimeStamp_E,s,c,o,h,l,v,q,topic, partition_msg, offset_msg) VALUES ('{}',{},'{}',{},{},{},{},{},{},'{}',{},{});",&value.e,&value.e2,&value.s,&value.c,&value.o,&value.h,&value.l,&value.v,&value.q,&topic,partition,offset);

    client.execute(&query, &[]).unwrap();
    // match client.execute(&query, &[]) {
    //     Ok(k) => println!("ok : {:#?}", k),
    //     Err(e) => println!("err: {:#?}", e),
    // }

    //not possible with pool connection manager
    // client.close().unwrap();
}
