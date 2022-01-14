# TPF – Twilight Price Feeder

TPF is Twilight Price Feeder which feeds the external BTC price into the twilight system (i.e. updating Redis IN-MEMORY database) built-in Rust focusing on the latest BTC price insertion using Binance Individual Symbol Mini Ticker Stream for twilight zk-matchbook.

Quick Start
-----------

On your local machine, run the following command to start a Redis container/app
```yaml
docker-compose up --build
```

Next, you can start TPF by running
```yaml
cargo run
```

This will establish WebSocket connection with binance mini ticker WebSocket Stream and start updating BTCUSDT pair ticker price in the database (in your Redis container).



## Get a Latest price from Redis DB 

To get an updated price from Redis DB, you can run get command with key : `btc:price` .

To get an updated full payload of mini ticker from Redis DB, you can run get command with key : `btc:price:full_payload` .


```yaml
// Need to create new main.rs file with the following code

```
```yaml
mod redis_db;

fn main() {
     println!("Current BTC Price is : {}",redis_db::get(&"btc:price"));
}

```
Output
```yaml
Current BTC Price is : xxxxx.xxxxxxxx
```
