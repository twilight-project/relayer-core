//! ## Twilight Price Feeder
//!TPF is Twilight Price Feeder which feeds the external BTC price into the twilight system (i.e. updating Redis IN-MEMORY database) built-in Rust focusing on the latest BTC price insertion using Binance Individual Symbol Mini Ticker Stream for twilight zk-matchbook.
//!
//!
//!   
//!
//!
#![allow(dead_code)]
extern crate futures;
extern crate serde_json;
extern crate tokionew;
extern crate websocket;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use std::io::stdin;
use std::thread;

use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
// #[path = "btc_price_feeder.rs"]
use crate::pricefeederlib::btc_price_feeder;

/// This function is used for initializing websocket with binance BTC/USDT pair (Mini Ticker) and update BTC price into redisDB having key/s `btc:price`, `btc:price:full_payload`.
pub fn receive_btc_price() {
    dotenv::dotenv().ok();

    // BINANCE_BTC_SOCKET URL for retriving BTCUSDT pair latest price
    let url = match std::env::var("BINANCE_BTC_SOCKET") {
        Ok(ws_adder) => ws_adder,
        Err(_) => "wss://stream.binance.com:9443/ws/btcusdt@miniTicker".to_string(),
    };

    println!("Connecting to {}", &url);
    let mut runtime = tokionew::runtime::current_thread::Builder::new()
        .build()
        .unwrap();

    // standard in isn't supported in mio yet, so we use a thread
    // see https://github.com/carllerche/mio/issues/321
    let (usr_msg, stdin_ch) = mpsc::channel(0);
    thread::spawn(|| {
        let mut input = String::new();
        let mut stdin_sink = usr_msg.wait();
        loop {
            input.clear();
            stdin().read_line(&mut input).unwrap();
            let trimmed = input.trim();
            let (close, msg) = match trimmed {
                "/close" => (true, OwnedMessage::Close(None)),
                "/ping" => (false, OwnedMessage::Ping(b"PING".to_vec())),
                _ => (false, OwnedMessage::Text(trimmed.to_string())),
            };
            stdin_sink
                .send(msg)
                .expect("Sending message across stdin channel.");

            if close {
                break;
            }
        }
    });

    let runner = ClientBuilder::new(&url)
        .unwrap()
        .async_connect_secure(None)
        .and_then(|(duplex, _)| {
            let (sink, stream) = duplex.split();
            stream
                .filter_map(|message| {
                    // let binance_payload_raw = message.clone();

                    // // get json string from Websocket::OwnedMessage in binance_payload_json
                    // let binance_payload_json = match binance_payload_raw {
                    //     OwnedMessage::Text(d) => d,
                    //     _ => "None".to_string(),
                    // };

                    // // BTC Price feeder
                    // // This fucntion is taking payload string received from binance websocket and updating the btc price into redisDB
                    // btc_price_feeder::update_btc_price(binance_payload_json);

                    // Using Ping/Pong mechanism to make connection alive before standard websocket connection timeout time
                    // Binance Note: # The websocket server will send a ping frame every 3 minutes. If the websocket server does not receive a pong frame back from the connection within a 10 minute period, the connection will be disconnected. Unsolicited pong frames are allowed.
                    match message {
                        OwnedMessage::Close(e) => {
                            thread::spawn(|| {
                                thread::sleep(std::time::Duration::from_millis(10));
                                thread::Builder::new()
                                    .name(String::from("BTC Binance Websocket Connection 1"))
                                    .spawn(move || {
                                        // thread::sleep(time::Duration::from_millis(1000));
                                        receive_btc_price();
                                    })
                                    .unwrap();
                            });
                            Some(OwnedMessage::Close(e))
                        }
                        OwnedMessage::Ping(d) => Some(OwnedMessage::Pong(d)),
                        OwnedMessage::Text(binance_payload_json) => {
                            btc_price_feeder::update_btc_price(binance_payload_json);
                            None
                        }
                        _ => None,
                    }
                })
                .select(stdin_ch.map_err(|_| WebSocketError::NoDataAvailable))
                .forward(sink)
        });
    let _ = runtime.block_on(runner).expect("No internet Connection");
}
