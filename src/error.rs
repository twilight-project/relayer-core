use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("Database error: {0}")]
    Database(#[from] postgres::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Kafka error: {0}")]
    Kafka(#[from] kafka::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Thread pool error: {0}")]
    ThreadPool(String),

    #[error("Order processing error: {0}")]
    OrderProcessing(String),

    #[error("Price update error: {0}")]
    PriceUpdate(String),

    #[error("Health check failed: {0}")]
    HealthCheck(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RelayerError>;
