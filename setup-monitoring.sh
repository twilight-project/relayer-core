#!/bin/bash

# Twilight Relayer Monitoring Setup Script
# This script sets up Grafana, Prometheus, and Loki for monitoring the Twilight Relayer

set -e

echo "🚀 Setting up Twilight Relayer Monitoring Stack..."

# Create monitoring directories if they don't exist
echo "📁 Creating monitoring directories..."
mkdir -p monitoring/prometheus/rules
mkdir -p monitoring/grafana/{provisioning/datasources,provisioning/dashboards,dashboards}
mkdir -p monitoring/loki
mkdir -p monitoring/promtail
mkdir -p logs

# Set proper permissions for log directory
echo "🔐 Setting up log directory permissions..."
chmod 755 logs

# Create prometheus rules directory
echo "📊 Creating Prometheus alerting rules..."
cat > monitoring/prometheus/rules/relayer-alerts.yml << 'EOF'
groups:
  - name: relayer.rules
    rules:
      - alert: RelayerDown
        expr: up{job="relayer"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Relayer is down"
          description: "The Twilight Relayer has been down for more than 1 minute."

      - alert: HighErrorRate
        expr: rate(relayer_errors_total[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors per second."

      - alert: DatabaseConnectionsHigh
        expr: relayer_db_connections_active > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High database connection usage"
          description: "Database connections are at {{ $value }}."

      - alert: TradingFailureRate
        expr: rate(relayer_trades_failed[5m]) / rate(relayer_trades_total[5m]) > 0.1
        for: 3m
        labels:
          severity: critical
        annotations:
          summary: "High trading failure rate"
          description: "Trading failure rate is {{ $value | humanizePercentage }}."
EOF

# Start the monitoring stack
echo "🐳 Starting monitoring services..."
docker-compose -f docker-compose.monitoring.yaml up -d

# Wait for services to be ready
echo "⏳ Waiting for services to be ready..."
sleep 30

# Check if services are running
echo "🔍 Checking service status..."
docker-compose -f docker-compose.monitoring.yaml ps

# Display access information
echo ""
echo "✅ Monitoring stack is ready!"
echo ""
echo "🎯 Access your monitoring tools:"
echo "  📊 Grafana:    http://localhost:3000 (admin/admin123)"
echo "  📈 Prometheus: http://localhost:9090"
echo "  📋 Loki:      http://localhost:3100"
echo "  🖥️  Node Exporter: http://localhost:9100"
echo "  🐳 cAdvisor:   http://localhost:8080"
echo ""
echo "📝 To view logs in real-time:"
echo "  tail -f logs/relayer-general.log"
echo "  tail -f logs/relayer-errors.log"
echo ""
echo "🔧 To stop the monitoring stack:"
echo "  docker-compose -f docker-compose.monitoring.yaml down"
echo ""
echo "🚀 To start your relayer with metrics enabled:"
echo "  Add the following to your main.rs initialization:"
echo "    use twilight_relayer_rust::metrics;"
echo "    let metrics = metrics::init_metrics().await?;"
echo "    metrics::start_metrics_collection().await;"
echo "    tokio::spawn(async move { metrics.start_server(3030).await });"
echo ""

# Check if main relayer is running
if docker ps | grep -q "relayer-dev"; then
    echo "✅ Relayer is running and should be exposing metrics on port 3030"
else
    echo "⚠️  Relayer is not running. Start it with:"
    echo "   docker-compose up -d relayer-dev"
fi

echo ""
echo "📚 For more information, see the monitoring documentation."
echo "Happy monitoring! 🎉" 