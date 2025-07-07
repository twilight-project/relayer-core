use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use metrics::{counter, gauge, histogram, register_counter, register_gauge, register_histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

/// Metrics collector for the Twilight Relayer
pub struct RelayerMetrics {
    prometheus_handle: PrometheusHandle,
}

impl RelayerMetrics {
    /// Initialize the metrics system
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder.install_recorder()?;

        // Register all metrics
        Self::register_metrics();

        Ok(RelayerMetrics { prometheus_handle })
    }

    /// Register all application metrics
    fn register_metrics() {
        // System metrics
        register_gauge!("relayer_uptime_seconds", "Relayer uptime in seconds");
        register_gauge!("relayer_memory_usage_bytes", "Memory usage in bytes");
        register_gauge!("relayer_cpu_usage_percent", "CPU usage percentage");

        // Trading metrics
        register_counter!("relayer_trades_total", "Total number of trades processed");
        register_counter!("relayer_trades_successful", "Number of successful trades");
        register_counter!("relayer_trades_failed", "Number of failed trades");
        register_histogram!(
            "relayer_trade_duration_seconds",
            "Trade processing duration"
        );
        register_gauge!("relayer_active_orders", "Number of active orders");

        // Price feed metrics
        register_counter!(
            "relayer_price_updates_total",
            "Total price updates received"
        );
        register_gauge!(
            "relayer_last_price_update_timestamp",
            "Timestamp of last price update"
        );
        register_histogram!(
            "relayer_price_update_latency_seconds",
            "Price update latency"
        );

        // Database metrics
        register_counter!("relayer_db_queries_total", "Total database queries");
        register_counter!("relayer_db_errors_total", "Total database errors");
        register_histogram!(
            "relayer_db_query_duration_seconds",
            "Database query duration"
        );
        register_gauge!(
            "relayer_db_connections_active",
            "Active database connections"
        );

        // Redis metrics
        register_counter!("relayer_redis_operations_total", "Total Redis operations");
        register_counter!("relayer_redis_errors_total", "Total Redis errors");
        register_histogram!(
            "relayer_redis_operation_duration_seconds",
            "Redis operation duration"
        );

        // Kafka metrics
        register_counter!(
            "relayer_kafka_messages_produced",
            "Total Kafka messages produced"
        );
        register_counter!(
            "relayer_kafka_messages_consumed",
            "Total Kafka messages consumed"
        );
        register_counter!("relayer_kafka_errors_total", "Total Kafka errors");

        // ZKOS transaction metrics
        register_counter!("relayer_zkos_transactions_total", "Total ZKOS transactions");
        register_counter!(
            "relayer_zkos_transactions_successful",
            "Successful ZKOS transactions"
        );
        register_counter!(
            "relayer_zkos_transactions_failed",
            "Failed ZKOS transactions"
        );
        register_histogram!(
            "relayer_zkos_transaction_duration_seconds",
            "ZKOS transaction duration"
        );

        // Heartbeat metrics
        register_counter!("relayer_heartbeats_total", "Total heartbeats sent");
        register_gauge!(
            "relayer_last_heartbeat_timestamp",
            "Timestamp of last heartbeat"
        );

        // Error metrics
        register_counter!("relayer_errors_total", "Total errors by category");
        register_counter!("relayer_log_entries_total", "Total log entries by level");
    }

    /// Get the Prometheus handle for metrics export
    pub fn prometheus_handle(&self) -> &PrometheusHandle {
        &self.prometheus_handle
    }

    /// Start the metrics HTTP server
    pub async fn start_server(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler))
            .layer(CorsLayer::permissive())
            .with_state(Arc::new(self.prometheus_handle.clone()));

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("Metrics server listening on port {}", port);

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Handler for the /metrics endpoint
async fn metrics_handler(State(handle): State<Arc<PrometheusHandle>>) -> Response {
    match handle.render() {
        Ok(metrics) => metrics.into_response(),
        Err(e) => {
            error!("Failed to render metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to render metrics",
            )
                .into_response()
        }
    }
}

/// Handler for the /health endpoint
async fn health_handler() -> Response {
    "OK".into_response()
}

/// Convenience macros for recording metrics
#[macro_export]
macro_rules! record_trade {
    ($successful:expr, $duration:expr) => {
        metrics::counter!("relayer_trades_total").increment(1);
        if $successful {
            metrics::counter!("relayer_trades_successful").increment(1);
        } else {
            metrics::counter!("relayer_trades_failed").increment(1);
        }
        metrics::histogram!("relayer_trade_duration_seconds").record($duration);
    };
}

#[macro_export]
macro_rules! record_price_update {
    ($latency:expr) => {
        metrics::counter!("relayer_price_updates_total").increment(1);
        metrics::gauge!("relayer_last_price_update_timestamp")
            .set(chrono::Utc::now().timestamp() as f64);
        metrics::histogram!("relayer_price_update_latency_seconds").record($latency);
    };
}

#[macro_export]
macro_rules! record_db_query {
    ($success:expr, $duration:expr) => {
        metrics::counter!("relayer_db_queries_total").increment(1);
        if !$success {
            metrics::counter!("relayer_db_errors_total").increment(1);
        }
        metrics::histogram!("relayer_db_query_duration_seconds").record($duration);
    };
}

#[macro_export]
macro_rules! record_redis_operation {
    ($success:expr, $duration:expr) => {
        metrics::counter!("relayer_redis_operations_total").increment(1);
        if !$success {
            metrics::counter!("relayer_redis_errors_total").increment(1);
        }
        metrics::histogram!("relayer_redis_operation_duration_seconds").record($duration);
    };
}

#[macro_export]
macro_rules! record_kafka_message {
    ($produced:expr) => {
        if $produced {
            metrics::counter!("relayer_kafka_messages_produced").increment(1);
        } else {
            metrics::counter!("relayer_kafka_messages_consumed").increment(1);
        }
    };
}

#[macro_export]
macro_rules! record_zkos_transaction {
    ($successful:expr, $duration:expr) => {
        metrics::counter!("relayer_zkos_transactions_total").increment(1);
        if $successful {
            metrics::counter!("relayer_zkos_transactions_successful").increment(1);
        } else {
            metrics::counter!("relayer_zkos_transactions_failed").increment(1);
        }
        metrics::histogram!("relayer_zkos_transaction_duration_seconds").record($duration);
    };
}

#[macro_export]
macro_rules! record_heartbeat {
    () => {
        metrics::counter!("relayer_heartbeats_total").increment(1);
        metrics::gauge!("relayer_last_heartbeat_timestamp")
            .set(chrono::Utc::now().timestamp() as f64);
    };
}

#[macro_export]
macro_rules! record_error {
    ($category:expr) => {
        metrics::counter!("relayer_errors_total", "category" => $category).increment(1);
    };
}

#[macro_export]
macro_rules! record_log_entry {
    ($level:expr) => {
        metrics::counter!("relayer_log_entries_total", "level" => $level).increment(1);
    };
}

/// System metrics collector
pub struct SystemMetrics {
    start_time: Instant,
}

impl SystemMetrics {
    pub fn new() -> Self {
        SystemMetrics {
            start_time: Instant::now(),
        }
    }

    /// Update system metrics
    pub fn update(&self) {
        // Update uptime
        let uptime = self.start_time.elapsed().as_secs_f64();
        gauge!("relayer_uptime_seconds").set(uptime);

        // Update memory usage (simplified - in production you'd want more accurate metrics)
        if let Ok(memory_usage) = Self::get_memory_usage() {
            gauge!("relayer_memory_usage_bytes").set(memory_usage as f64);
        }

        // Update CPU usage (simplified - in production you'd want more accurate metrics)
        if let Ok(cpu_usage) = Self::get_cpu_usage() {
            gauge!("relayer_cpu_usage_percent").set(cpu_usage);
        }
    }

    fn get_memory_usage() -> Result<u64, Box<dyn std::error::Error>> {
        // Simplified memory usage - in production, use a proper system metrics library
        // like `sysinfo` or parse /proc/meminfo
        Ok(0) // Placeholder
    }

    fn get_cpu_usage() -> Result<f64, Box<dyn std::error::Error>> {
        // Simplified CPU usage - in production, use a proper system metrics library
        Ok(0.0) // Placeholder
    }
}

/// Timer utility for measuring operation duration
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            start: Instant::now(),
        }
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

/// Initialize metrics system
pub async fn init_metrics() -> Result<RelayerMetrics, Box<dyn std::error::Error>> {
    info!("Initializing metrics system...");
    let metrics = RelayerMetrics::new()?;
    info!("Metrics system initialized successfully");
    Ok(metrics)
}

/// Start metrics collection background task
pub async fn start_metrics_collection() {
    let system_metrics = SystemMetrics::new();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            system_metrics.update();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_initialization() {
        let metrics = RelayerMetrics::new().unwrap();
        assert!(metrics.prometheus_handle().render().is_ok());
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(timer.elapsed_seconds() >= 0.1);
    }
}
