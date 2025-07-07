# Twilight Relayer Monitoring Guide

This guide explains how to set up and use the comprehensive monitoring stack for the Twilight Relayer project using Grafana, Prometheus, and Loki.

## 🚀 Quick Start

### 1. Setup Monitoring Stack

Run the setup script to deploy all monitoring services:

```bash
./setup-monitoring.sh
```

This will:

- Create all necessary configuration files
- Start Grafana, Prometheus, Loki, and supporting services
- Set up log collection from your existing log files
- Create alerting rules for critical metrics

### 2. Access Monitoring Tools

Once setup is complete, you can access:

- **Grafana Dashboard**: http://localhost:3000
  - Username: `admin`
  - Password: `admin123`
- **Prometheus**: http://localhost:9090
- **Loki**: http://localhost:3100
- **Node Exporter**: http://localhost:9100
- **cAdvisor**: http://localhost:8080

## 📊 What You'll Monitor

### System Metrics

- **CPU Usage**: Real-time CPU utilization
- **Memory Usage**: Memory consumption and availability
- **Disk Usage**: Disk space utilization
- **Network I/O**: Network traffic and throughput

### Application Metrics

- **Trading Operations**: Success/failure rates, processing times
- **Price Feed**: Update frequency, latency, last update timestamps
- **Database Operations**: Query performance, connection pool usage
- **Redis Operations**: Cache hit/miss rates, operation latency
- **Kafka Messages**: Message production/consumption rates
- **ZKOS Transactions**: Transaction processing metrics
- **Heartbeat**: Service health check status

### Log Analysis

- **Error Logs**: Clean error tracking without noise
- **Trading Logs**: Trading-specific events and operations
- **Price Logs**: Price feed updates and changes
- **Database Logs**: Database operation logs
- **Heartbeat Logs**: Health check and monitoring logs
- **ZKOS Logs**: ZKOS transaction processing logs

## 🔧 Integration with Your Relayer

### 1. Add Metrics to Your Main Application

Update your `main.rs` to include metrics:

```rust
use twilight_relayer_rust::metrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (existing)
    twilight_relayer_rust::logging::init_logging();

    // Initialize metrics (new)
    let metrics = metrics::init_metrics().await?;

    // Start metrics collection background task
    metrics::start_metrics_collection().await;

    // Start metrics HTTP server (expose /metrics endpoint)
    let metrics_server = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = metrics_server.start_server(3030).await {
            tracing::error!("Failed to start metrics server: {}", e);
        }
    });

    // Your existing application code...

    Ok(())
}
```

### 2. Add Metrics to Your Code

Use the provided macros to record metrics throughout your application:

#### Trading Operations

```rust
use twilight_relayer_rust::{record_trade, metrics::Timer};

async fn process_trade(order: &Order) -> Result<(), TradeError> {
    let timer = Timer::new();

    match execute_trade(order).await {
        Ok(_) => {
            record_trade!(true, timer.elapsed_seconds());
            tracing::info!("Trade executed successfully");
        }
        Err(e) => {
            record_trade!(false, timer.elapsed_seconds());
            tracing::error!("Trade failed: {}", e);
        }
    }
}
```

#### Price Updates

```rust
use twilight_relayer_rust::record_price_update;

async fn handle_price_update(price_data: PriceData) {
    let latency = calculate_latency(&price_data);
    record_price_update!(latency);

    // Process price update...
}
```

#### Database Operations

```rust
use twilight_relayer_rust::{record_db_query, metrics::Timer};

async fn query_database(query: &str) -> Result<Vec<Row>, DbError> {
    let timer = Timer::new();

    match execute_query(query).await {
        Ok(rows) => {
            record_db_query!(true, timer.elapsed_seconds());
            Ok(rows)
        }
        Err(e) => {
            record_db_query!(false, timer.elapsed_seconds());
            Err(e)
        }
    }
}
```

#### Redis Operations

```rust
use twilight_relayer_rust::{record_redis_operation, metrics::Timer};

async fn redis_set(key: &str, value: &str) -> Result<(), RedisError> {
    let timer = Timer::new();

    match redis_client.set(key, value).await {
        Ok(_) => {
            record_redis_operation!(true, timer.elapsed_seconds());
            Ok(())
        }
        Err(e) => {
            record_redis_operation!(false, timer.elapsed_seconds());
            Err(e)
        }
    }
}
```

#### ZKOS Transactions

```rust
use twilight_relayer_rust::{record_zkos_transaction, metrics::Timer};

async fn process_zkos_transaction(tx: &Transaction) -> Result<(), TxError> {
    let timer = Timer::new();

    match zkos_processor.process(tx).await {
        Ok(_) => {
            record_zkos_transaction!(true, timer.elapsed_seconds());
            Ok(())
        }
        Err(e) => {
            record_zkos_transaction!(false, timer.elapsed_seconds());
            Err(e)
        }
    }
}
```

#### Heartbeat

```rust
use twilight_relayer_rust::record_heartbeat;

async fn send_heartbeat() {
    // Send heartbeat...
    record_heartbeat!();
}
```

## 📈 Using Grafana

### Default Dashboard

The setup includes a comprehensive dashboard called "Twilight Relayer Dashboard" with:

1. **System Overview**: CPU, Memory, Disk, and Network metrics
2. **Application Logs**: Real-time log viewing by category
3. **Log Volume**: Visualization of log generation rates
4. **Container Metrics**: Docker container resource usage

### Creating Custom Dashboards

1. Go to Grafana (http://localhost:3000)
2. Click "+" → "Dashboard"
3. Add panels using Prometheus queries

#### Example Queries

**Trading Success Rate**:

```promql
rate(relayer_trades_successful[5m]) / rate(relayer_trades_total[5m]) * 100
```

**Average Trade Duration**:

```promql
rate(relayer_trade_duration_seconds_sum[5m]) / rate(relayer_trade_duration_seconds_count[5m])
```

**Database Connection Usage**:

```promql
relayer_db_connections_active
```

**Error Rate by Category**:

```promql
rate(relayer_errors_total[5m])
```

## 🔍 Log Analysis with Loki

### Viewing Logs in Grafana

1. Go to "Explore" in Grafana
2. Select "Loki" as the data source
3. Use LogQL queries to filter logs

#### Example LogQL Queries

**All error logs**:

```logql
{job="relayer", category="errors"}
```

**Trading logs from the last hour**:

```logql
{job="relayer", category="trading"} |= "trade" [1h]
```

**Database errors**:

```logql
{job="relayer", category="database"} |= "error"
```

**ZKOS transaction logs**:

```logql
{job="relayer", category="zkos"} |= "transaction"
```

## 🚨 Alerting

### Pre-configured Alerts

The setup includes several pre-configured alerts:

1. **RelayerDown**: Triggers when the relayer stops responding
2. **HighErrorRate**: Triggers when error rate exceeds threshold
3. **DatabaseConnectionsHigh**: Triggers when DB connections are high
4. **TradingFailureRate**: Triggers when trade failure rate is high

### Adding Custom Alerts

1. Edit `monitoring/prometheus/rules/relayer-alerts.yml`
2. Add your custom alerting rules
3. Restart Prometheus: `docker-compose -f docker-compose.monitoring.yaml restart prometheus`

#### Example Custom Alert

```yaml
- alert: PriceFeedStale
  expr: time() - relayer_last_price_update_timestamp > 300
  for: 1m
  labels:
    severity: warning
  annotations:
    summary: "Price feed is stale"
    description: "No price updates received for more than 5 minutes."
```

## 🔧 Maintenance

### Managing Services

**Start monitoring stack**:

```bash
docker-compose -f docker-compose.monitoring.yaml up -d
```

**Stop monitoring stack**:

```bash
docker-compose -f docker-compose.monitoring.yaml down
```

**View logs**:

```bash
docker-compose -f docker-compose.monitoring.yaml logs -f grafana
docker-compose -f docker-compose.monitoring.yaml logs -f prometheus
```

**Restart specific service**:

```bash
docker-compose -f docker-compose.monitoring.yaml restart grafana
```

### Data Retention

- **Prometheus**: Configured for 200 hours of retention
- **Loki**: Uses local storage with automatic cleanup
- **Grafana**: Dashboards and settings are persisted

### Backup

Important data locations:

- Grafana dashboards: `grafana_data` volume
- Prometheus data: `prometheus_data` volume
- Loki data: `loki_data` volume

## 🐛 Troubleshooting

### Common Issues

**Grafana not loading**:

- Check if port 3000 is available
- Verify Grafana container is running: `docker ps`

**No metrics in Prometheus**:

- Ensure your relayer is exposing metrics on port 3030
- Check Prometheus targets: http://localhost:9090/targets

**No logs in Loki**:

- Verify log files exist in `logs/` directory
- Check Promtail configuration and container logs

**Permission issues**:

- Ensure log directory has proper permissions: `chmod 755 logs`
- Check Docker has access to log files

### Debugging Commands

```bash
# Check all containers
docker-compose -f docker-compose.monitoring.yaml ps

# Check specific service logs
docker-compose -f docker-compose.monitoring.yaml logs grafana

# Test metrics endpoint
curl http://localhost:3030/metrics

# Test Prometheus query
curl 'http://localhost:9090/api/v1/query?query=up'
```

## 📚 Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Loki Documentation](https://grafana.com/docs/loki/)
- [PromQL Tutorial](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [LogQL Tutorial](https://grafana.com/docs/loki/latest/logql/)

## 🎯 Best Practices

1. **Monitor Key Business Metrics**: Focus on trading success rates, latency, and error rates
2. **Set Meaningful Alerts**: Avoid alert fatigue with well-tuned thresholds
3. **Use Structured Logging**: Include relevant context in your logs
4. **Regular Dashboard Reviews**: Keep dashboards updated and relevant
5. **Backup Configurations**: Regularly backup your Grafana dashboards and Prometheus rules

Happy monitoring! 🎉
