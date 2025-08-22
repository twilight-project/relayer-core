# Twilight Relayer Core

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.87+-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

A high-performance matching engine written in Rust, designed to handle tens of thousands of orders per second using Event Sourcing pattern with CQRS framework.

## 🏗️ Architecture

```
                          +----------+
Client Request Queue  >>  |  Relayer |  >> Event Logs
                          +----------+
                               |
                               v
                       [ PostgreSQL DB ]
                               |
                               v
                         [ Redis Cache ]
```

The relayer core implements:

- **Event Sourcing**: All state changes are stored as events
- **CQRS Pattern**: Command Query Responsibility Segregation
- **High-Performance Matching**: Handles thousands of orders per second
- **Real-time Processing**: WebSocket connections for live price feeds
- **Blockchain Integration**: ZKOS chain transaction support

## 📋 Prerequisites

Before running the relayer core, ensure you have the following installed:

- **Rust** (1.87.0+) - [Install Rust](https://rustup.rs/)
- **Docker & Docker Compose** - [Install Docker](https://docs.docker.com/get-docker/)
- **PostgreSQL** (13+)
- **Redis** (6+)
- **Apache Kafka** with Zookeeper

## 🚀 Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/twilight-project/relayer-core.git
cd relayer-core
```

### 2. Environment Setup

Create your environment configuration file:

```bash
cp .env.example .env
```

Edit the `.env` file with your specific configuration:

```bash
nano .env
```

### 3. Database Setup

Start the required services using Docker Compose:

```bash
# Start Kafka, Zookeeper
docker compose up --build kafka zookeeper
```

### 4. Create Kafka Topics

Create the necessary Kafka topics for message queuing:

```bash
# Create all required topics
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CLIENT-REQUEST --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic SnapShotLogTopic --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CoreEventLogTopic --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic RelayerStateQueue --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CLIENT-FAILED-REQUEST --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime"
```

### 5. Build and Run

#### Development Mode

```bash
# Build the project
cargo build

# Run the relayer core
cargo run --bin main
```

#### Production Mode

```bash
# Build optimized release
cargo build --release

# Run the optimized binary
./target/release/main
```

#### Docker Deployment

```bash
# Build and run all services
docker-compose up --build
```

## ⚙️ Configuration

### Environment Variables

The relayer core uses environment variables for configuration. Here are the key categories:

#### Kafka Configuration

```bash
# Kafka broker
BROKER=localhost:9092

# Kafka topics
RPC_CLIENT_REQUEST=CLIENT-REQUEST
CORE_EVENT_LOG=CoreEventLogTopic
SNAPSHOT_LOG=SnapShotLogTopic
RELAYER_STATE_QUEUE=RelayerStateQueue

```

#### Server Configuration

```bash
# Relayer admin server settings
RELAYER_SERVER_SOCKETADDR=0.0.0.0:3031
RELAYER_SERVER_THREAD=2
```

#### Trading Configuration

```bash
# Trading fees (as percentages)
FILLED_ON_MARKET=0.04
FILLED_ON_LIMIT=0.02
SETTLED_ON_MARKET=0.04
SETTLED_ON_LIMIT=0.02
```

#### Blockchain Wallet Configuration

```bash
# Wallet security (REQUIRED)
RELAYER_WALLET_IV=your_wallet_iv_here
RELAYER_WALLET_SEED=your_wallet_seed_here
RELAYER_WALLET_PATH=/path/to/wallet/file
RELAYER_WALLET_PASSWORD=your_wallet_password_here

# Blockchain transactions
ENABLE_ZKOS_CHAIN_TRANSACTION=true
```

### Creating .env.example

To create a template for new deployments:

```bash
# Copy your configured .env to create an example
cp .env .env.example

# Remove sensitive information from .env.example
sed -i 's/=.*/=/' .env.example
```

## 🔧 Build Options

### Debug Build

```bash
cargo build
```

### Release Build

```bash
cargo build --release
```

## 🏃 Running the Application

### Local Development

1. **Start Dependencies**:

```bash
docker compose build
```

```bash
docker compose up --build kafka zookeeper
```

```bash
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CLIENT-REQUEST --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic SnapShotLogTopic --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CoreEventLogTopic --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic RelayerStateQueue --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime" && \
docker exec -it zookeeper sh -c "cd usr/bin && kafka-topics --topic CLIENT-FAILED-REQUEST --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact --config message.timestamp.type=LogAppendTime"
```

```bash
chmod -R 777 dockerize/redisDB/log/
```

```bash
docker compose up --build postgresql-master redis-server database
```

```bash
docker compose up --build rpckafka querykafkapsql
```

```bash
docker compose up --build auth api archiver frontend
```

```bash
docker compose up --build relayer-dev
```

### Production with Supervisor

For production deployments, use Supervisor for process management:

```bash
# Install Supervisor
sudo apt update
sudo apt install supervisor

# Create log directory
mkdir -p /home/ubuntu/relayer-core/logs

# Configure Supervisor
sudo tee /etc/supervisor/conf.d/relayer.conf > /dev/null <<EOF
[program:relayer]
command=/home/ubuntu/relayer-core/target/release/main
directory=/home/ubuntu/relayer-core
autostart=true
autorestart=true
stderr_logfile=/home/ubuntu/relayer-core/logs/relayer.err.log
stdout_logfile=/home/ubuntu/relayer-core/logs/relayer.out.log
user=ubuntu
environment=HOME="/home/ubuntu"
stderr_logfile_maxbytes=50MB
stdout_logfile_maxbytes=50MB
stderr_logfile_backups=10
stdout_logfile_backups=10
EOF

# Update and start Supervisor
sudo supervisorctl reread
sudo supervisorctl update
sudo supervisorctl start relayer
```

## 📡 API Documentation

The relayer core provides several API endpoints:

### Relayer Admin Server (Port 3031)

- Internal relayer communication
- State synchronization
- Event broadcasting
- Fee Management

For detailed API documentation, refer to the [Postman Collection](./postman-requests/Twilight%20RelayerAPI%20HMAC.postman_collection.json).

## 🐳 Docker Deployment

### Full Stack Deployment

```bash
# Start all services
docker-compose up -d
```

### Individual Service Deployment

```bash
# Start only the relayer core
docker-compose up -d relayer-core

# Start only dependencies
docker-compose up -d kafka zookeeper
```

### Docker Build Options

````bash
# Build with standard Dockerfile
docker build -t relayer-core .


## 🔍 Monitoring and Logging

### Log Files

- **Application Logs**: `./logs/relayer.out.log`
- **Error Logs**: `./logs/relayer.err.log`
- **Rust Logs**: Configure with `RUST_LOG` environment variable

### Log Levels

```bash
# Set log level
export RUST_LOG=info          # info, debug, warn, error, trace
export RUST_BACKTRACE=full    # Enable full backtraces
````

### Health Checks

The relayer provides health check endpoints:

- HTTP health check on configured ports
- Kafka connectivity status
- Database connection status
- Redis connection status

## 🧪 Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
cargo test --test integration
```

### Load Testing

```bash
# Use provided Postman collection for load testing
# Configure concurrent requests based on your requirements
```

## 🔐 Security Considerations

### Wallet Security

- **Never commit wallet seeds or passwords to version control**
- Use strong, randomly generated passwords
- Regularly rotate wallet credentials
- Backup wallet files securely

### Network Security

- Configure firewall rules for exposed ports
- Use TLS/SSL for external connections
- Implement rate limiting
- Monitor for suspicious activities

### Database Security

- Use strong database passwords
- Enable database encryption at rest
- Implement proper access controls
- Regular security updates

## 🚀 Performance Optimization

### System Requirements

**Minimum Requirements:**

- CPU: 4 cores
- RAM: 8GB
- Storage: 100GB SSD
- Network: 100Mbps

**Recommended Requirements:**

- CPU: 8+ cores
- RAM: 16GB+
- Storage: 500GB+ NVMe SSD
- Network: 1Gbps+

### Configuration Tuning

```bash
# Increase thread counts for high load
RPC_SERVER_THREAD=20
RELAYER_SERVER_THREAD=4

# Optimize database connections
# Configure connection pooling in your database URLs

# Tune Kafka settings
# Increase partitions for higher throughput
```

## 🛠️ Development

### Code Structure

```
src/
├── main.rs              # Application entry point
├── lib.rs               # Library exports
├── config/              # Configuration handling
├── database/            # Database interactions
├── kafka/               # Kafka message handling
├── matching/            # Order matching engine
├── rpc/                 # RPC server implementation
├── websocket/           # WebSocket connections
└── utils/               # Utility functions
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## 📚 Additional Resources

- [Deployment Guide](./DEPLOYEMENT.md)
- [API Documentation](./postman-requests/)
- [Docker Compose Files](./docker-compose.yaml)
- [Nginx Configuration](./nginx/)
- [SQL Scripts](./sqlscript/)

## 🤝 Support

- **Issues**: [GitHub Issues](https://github.com/twilight-project/relayer-core/issues)
- **Documentation**: [Project Wiki](https://github.com/twilight-project/relayer-core/wiki)
- **Community**: [Discord Server](https://discord.gg/twilight)

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🔄 Changelog

### v0.1.0

- Initial release
- Core matching engine implementation
- Event sourcing architecture
- CQRS pattern implementation
- Docker deployment support
- Supervisor integration

---

**Note**: This is a high-performance trading system. Always test thoroughly in a development environment before deploying to production.
