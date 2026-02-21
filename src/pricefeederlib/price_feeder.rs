//! ## Twilight Price Feeder
//! TPF is Twilight Price Feeder which feeds the external BTC price into the twilight system
//! (i.e. updating Redis IN-MEMORY database) built-in Rust focusing on the latest BTC price
//! insertion using Binance Individual Symbol Mini Ticker Stream for twilight zk-matchbook.

#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{delay_for, interval};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::pricefeederlib::btc_price_feeder;

/// Shared price state
#[derive(Debug, Clone)]
pub struct PriceState {
    pub last_price: Arc<RwLock<f64>>,
}

impl PriceState {
    pub fn new() -> Self {
        Self {
            last_price: Arc::new(RwLock::new(0.0)),
        }
    }

    pub async fn update_price(&self, new_price: f64) {
        let mut price = self.last_price.write().await;
        *price = new_price;
    }

    pub async fn get_price(&self) -> f64 {
        *self.last_price.read().await
    }
}

/// Configuration for the price feeder
#[derive(Debug, Clone)]
pub struct Config {
    pub websocket_url: String,
    pub reconnect_delay: Duration,
    pub ping_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            websocket_url: std::env::var("BINANCE_BTC_SOCKET").unwrap_or_else(|_| {
                "wss://stream.binance.com:9443/ws/btcusdt@miniTicker".to_string()
            }),
            reconnect_delay: Duration::from_secs(2),
            ping_interval: Duration::from_secs(30),
        }
    }
}

/// Main price feeder struct
pub struct PriceFeeder {
    config: Config,
    price_state: PriceState,
}

impl PriceFeeder {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            price_state: PriceState::new(),
        }
    }

    /// Start the price feeding process
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::log_heartbeat!(info, "Starting BTC price feeder...");

        loop {
            match self.connect_and_stream().await {
                Ok(_) => {
                    crate::log_price!(debug, "WebSocket connection closed normally");
                }
                Err(e) => {
                    crate::log_price!(
                        error,
                        "WebSocket error: {}. Reconnecting in {:?}",
                        e,
                        self.config.reconnect_delay
                    );
                    delay_for(self.config.reconnect_delay).await;
                }
            }
        }
    }

    /// Connect to WebSocket and handle the stream
    async fn connect_and_stream(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::log_price!(info, "Connecting to {}", self.config.websocket_url);

        let (ws_stream, _) = connect_async(&self.config.websocket_url).await?;
        crate::log_price!(
            info,
            "Connected successfully! Starting message processing..."
        );
        let (mut write, mut read) = ws_stream.split();

        // Channel for handling ping/pong and control messages
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        // Spawn ping task
        let ping_tx = tx.clone();
        let ping_task = tokio::spawn(async move {
            let mut ping_interval = interval(Duration::from_secs(30));
            loop {
                ping_interval.tick().await;
                if ping_tx.send(Message::Ping(b"ping".to_vec())).is_err() {
                    break;
                }
            }
        });

        // Spawn write task
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Main message processing loop
        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    self.process_price_message(text).await;
                }
                Message::Ping(data) => {
                    if tx.send(Message::Pong(data)).is_err() {
                        break;
                    }
                }
                Message::Pong(_) => {
                    // Pong received, connection is alive
                }
                Message::Close(_) => {
                    crate::log_price!(
                        debug,
                        "Binance server closed connection (likely 24hr timeout) - reconnecting..."
                    );
                    break;
                }
                Message::Binary(_) => {
                    crate::log_price!(warn, "Received unexpected binary message");
                }
            }
        }

        // Signal tasks to stop by dropping the channel
        drop(tx);

        // Wait for tasks to complete (they'll exit when channel is closed)
        let _ = ping_task.await;
        let _ = write_task.await;

        Ok(())
    }

    /// Process incoming price messages
    async fn process_price_message(&self, text: String) {
        let current_price = self.price_state.get_price().await;
        let new_price = btc_price_feeder::update_btc_price(text, &current_price);
        if new_price != current_price {
            self.price_state.update_price(new_price).await;
            // Auto-resume price feed if it was paused due to staleness
            use crate::relayer::{RiskState, RISK_ENGINE_STATE};
            let state = RISK_ENGINE_STATE.lock().unwrap();
            let was_paused = state.pause_price_feed;
            drop(state);
            if was_paused {
                RiskState::set_pause_price_feed(false);
                crate::log_heartbeat!(info, "STALE PRICE: Fresh price received — auto-resuming price feed");
            }
        }
    }

    /// Get the current price
    pub async fn get_current_price(&self) -> f64 {
        self.price_state.get_price().await
    }
}

/// Convenience function to start the price feeder with default config
pub async fn receive_btc_price() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv::dotenv().ok();

    let config = Config::default();
    let feeder = PriceFeeder::new(config);

    feeder.start().await
}

/// Start price feeder with custom configuration
pub async fn start_price_feeder_with_config(
    config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let feeder = PriceFeeder::new(config);
    feeder.start().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[test]
    fn test_price_state() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = PriceState::new();

            // Initial price should be 0.0
            assert_eq!(state.get_price().await, 0.0);

            // Update price
            state.update_price(50000.0).await;
            assert_eq!(state.get_price().await, 50000.0);
        });
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.websocket_url.contains("binance.com"));
        assert!(config.reconnect_delay.as_secs() > 0);
    }

    #[test]
    fn test_price_feeder_creation() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = Config::default();
            let feeder = PriceFeeder::new(config);

            // Initial price should be 0.0
            assert_eq!(feeder.get_current_price().await, 0.0);
        });
    }

    // Integration test (commented out as it requires network)
    #[test]
    fn test_price_feed_websocket() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = timeout(Duration::from_secs(10), receive_btc_price()).await;
            // Should timeout (connection should stay alive)
            assert!(result.is_err());
        });
    }
}
