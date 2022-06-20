use crate::config::{QUESTDB_POOL_CONNECTION, THREADPOOL};
use serde_derive::{Deserialize, Serialize};
// use std::sync::{mpsc, Arc, Mutex, RwLock};
use super::checkservertime::ServerTime;
use chrono::prelude::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candle {
    pub low: f64,
    pub high: f64,
    pub open: f64,
    pub close: f64,
    pub sell_volume: f64,
    pub buy_volume: f64,
    pub timestamp: std::time::SystemTime,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Candles {
    pub candles: Vec<Candle>,
}

fn time_till_min(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    let date_time = format!("{}", dt.format("%d-%m-%Y-%H:%M"));
    date_time
}
// sample_by = 1m/5m/30m/1hr/4hr/8hr/24hr
pub fn get_candle(
    sample_by: String,
    limit: i32,
    pagination: i32,
) -> Result<Candles, std::io::Error> {
    let threadpool = THREADPOOL.lock().unwrap();
    let (sender, receiver) = mpsc::channel();
    threadpool.execute(move || {
        let query = format!(" Select t3.timestamp,t3.open,t3.close,t3.min,t3.max,coalesce(t3.sell_volume , 0) as Sell_Volume, coalesce(t4.buy_volume , 0) as Buy_Volume from (

            Select t1.*,t2.sell_volume from (
                SELECT timestamp, first(price) AS open, last(price) AS close, min(price), max(price)
                    FROM recentorders WHERE timestamp > dateadd('d', -30, now())
                      SAMPLE BY {} ALIGN TO CALENDAR) t1

                LEFT OUTER JOIN (

                SELECT timestamp,sum(amount) AS sell_volume
                    FROM recentorders WHERE side=0 AND timestamp > dateadd('d', -30, now())
                          SAMPLE BY {} ALIGN TO CALENDAR) t2 ON t1.timestamp = t2.timestamp ) t3
          LEFT OUTER JOIN (

          SELECT timestamp, sum(amount) AS buy_volume
                FROM recentorders WHERE side=1 AND timestamp > dateadd('d', -30, now())
                      SAMPLE BY {} ALIGN TO CALENDAR) t4 ON t3.timestamp = t4.timestamp Limit {} ;",&sample_by,&sample_by,&sample_by,limit);
        let mut client = QUESTDB_POOL_CONNECTION.get().unwrap();
        let mut candle_data: Vec<Candle> = Vec::new();
        match client.query(&query, &[]) {
            Ok(data) => {
                for row in data {
                    // let ttime: std::time::SystemTime = row.get("timestamp");
                    candle_data.push(Candle {
                        low: row.get("min"),
                        high: row.get("max"),
                        open: row.get("open"),
                        close: row.get("close"),
                        sell_volume: row.get("Sell_Volume"),
                        buy_volume: row.get("Buy_Volume"),
                        timestamp: row.get("timestamp"),
                    });
                }
                sender.send(Ok(candle_data)).unwrap();
            }
            Err(arg) => sender
                .send(Err(std::io::Error::new(std::io::ErrorKind::Other, arg)))
                .unwrap(),
        }
    });

    // println!("{:#?}", receiver.recv().unwrap());
    match receiver.recv().unwrap() {
        Ok(value) => {
            return Ok(Candles { candles: value });
        }
        Err(arg) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, arg));
        }
    };
    // return Ok(Candles {
    //     candles: receiver.recv().unwrap(),
    // });
}

// println!(
//     "Data 2 : {:#?}",
//     serde_json::to_string(&FundingRow {
//         rate: row.get("fundingrate"),
//         price: row.get("price"),
//         timestamp: row.get("timestamp")
//     })
//     .unwrap()
// );
// let timestamp: std::time::SystemTime = row.get("timestamp");
// let price: Decimal = row.get("fundingrate");
// let fprice = price.to_f64().unwrap();
// println!("data:{:#?}", iso8601(&timestamp));
// println!(
//     "data:{:#?}",
//     timestamp
//         .duration_since(std::time::SystemTime::UNIX_EPOCH)
//         .unwrap()
// );
