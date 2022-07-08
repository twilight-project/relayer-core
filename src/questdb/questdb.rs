use crate::relayer::CloseTrade;
use crate::relayer::Side;
use std::io::prelude::*;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// let mut stream = TcpStream::connect("127.0.0.1:9009").unwrap();
// for i in 1..1000 {
//     stream.write(data1).unwrap();
// }
// stream.flush().unwrap();
lazy_static! {
    pub static ref QUESTDB_INFLUX: Mutex<TcpStream> = Mutex::new(connect());
}

pub fn connect() -> TcpStream {
    dotenv::dotenv().expect("Failed loading dotenv");
    let questdb_url = std::env::var("QUESTDB_INFLUX_URL")
        .expect("missing environment variable QUESTDB_INFLUX_URL");
    let stream = TcpStream::connect(questdb_url).unwrap();
    stream
}

pub fn send_candledata_in_questdb(data: CloseTrade) {
    // let data = b"recentorders side=6i,price=1814.47,amount=287122.05005 1556813561098000000\n";
    let mut stream = QUESTDB_INFLUX.lock().unwrap();
    let query = format!(
        "recentorders side={}i,price={},amount={} {}\n",
        (data.side as u32),
        data.price,
        data.positionsize,
        data.timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    );
    stream.write(query.as_bytes()).unwrap();
    stream.flush().unwrap();
    drop(stream);
}
