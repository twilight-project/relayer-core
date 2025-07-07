# Twilight Relayer Logging System

This guide explains how to use the comprehensive logging system implemented in the Twilight Relayer project. The system creates different log files for different types of output, making it easier to debug and monitor specific components.

## Log Files Created

The logging system automatically creates the following log files in the `logs/` directory:

1. **`relayer-general.log`** - General application logs (INFO and WARN levels)
2. **`relayer-errors.log`** - Error logs only (ERROR level and above) - **CLEAN, NO NOISE!**
3. **`relayer-trading.log`** - Trading-specific logs
4. **`relayer-price.log`** - Price feed and price update logs
5. **`relayer-heartbeat.log`** - Heartbeat and health check logs
6. **`relayer-database.log`** - Database operation logs
7. **`zkos-tx-log`** - ZKOS transaction logs from zkos-relayer-wallet library

All logs are also displayed in the console with color formatting for easy development.

## Log Rotation

- **Daily Rotation**: Log files are automatically rotated daily
- **Date Stamps**: Each day's logs are saved with date stamps
- **No Size Limits**: Current implementation uses daily rotation only

## How to Use

### 1. General Logging

Use standard `tracing` macros for general application logs:

```rust
tracing::info!("Application started");
tracing::warn!("Configuration value missing, using default");
tracing::error!("Critical error occurred: {}", error_msg);
```

### 2. Category-Specific Logging

Use the provided macros for specific log categories:

#### Trading Logs

```rust
crate::log_trading!(info, "New trading order received: {}", order_id);
crate::log_trading!(warn, "Trading volume threshold exceeded");
crate::log_trading!(error, "Failed to execute trade: {}", error);
```

#### Price Feed Logs

```rust
crate::log_price!(info, "BTC price updated: ${}", price);
crate::log_price!(debug, "Price feed connection established");
crate::log_price!(error, "Price feed connection lost");
```

#### Heartbeat Logs

```rust
crate::log_heartbeat!(info, "Heartbeat check completed");
crate::log_heartbeat!(warn, "Heartbeat response delayed");
crate::log_heartbeat!(debug, "Scheduling next heartbeat in 60s");
```

#### Database Logs

```rust
crate::log_database!(info, "Database connection established");
crate::log_database!(warn, "Database query took {}ms", duration);
crate::log_database!(error, "Database connection failed: {}", error);
```

#### ZKOS Transaction Logs

```rust
crate::log_zkos_tx!(info, "Transaction processed: {}", tx_id);
crate::log_zkos_tx!(debug, "Transaction validation started");
crate::log_zkos_tx!(error, "Transaction failed: {}", error);
```

### 3. Structured Logging

Add structured data to your logs for better analysis:

```rust
tracing::info!(
    user_id = 12345,
    action = "place_order",
    amount = 0.001,
    symbol = "BTC",
    "User placed a new order"
);
```

### 4. Tracing Spans

Use spans to trace execution flow across function calls:

```rust
let span = tracing::info_span!("order_processing", order_id = "order_456");
let _enter = span.enter();

tracing::info!("Starting order validation");
// ... do work ...
tracing::info!("Order validation completed");

// Span automatically ends when _enter is dropped
```

## Log Levels

The system supports the following log levels (from most to least verbose):

- **`trace`** - Very detailed information, typically only of interest when diagnosing problems
- **`debug`** - Detailed information on the flow through the system
- **`info`** - General information about what the program is doing
- **`warn`** - Something unexpected happened, but the program is still working
- **`error`** - Something failed, which might or might not be recoverable

## Environment Configuration

You can control logging levels using environment variables:

```bash
# Set global log level
export RUST_LOG=debug

# Set specific module log levels
export RUST_LOG="info,relayer::trading=debug,relayer::price=trace"

# Set specific target log levels
export RUST_LOG="relayer::heartbeat=debug"

# Enable ZKOS transaction debugging
export RUST_LOG="info,zkos_relayer_wallet=debug,transaction=debug"
```

## Clean Error Logs (No More Noise!)

The error log has been specially configured to eliminate external dependency noise:

- **Only actual ERROR level logs** from your application
- **No TRACE/DEBUG spam** from WebSocket, Kafka, or HTTP libraries
- **Clean output** for easy bug detection

## Monitoring Different Components

### Real-time Monitoring

```bash
# Watch all logs
tail -f logs/relayer-general.log

# Watch specific category
tail -f logs/relayer-trading.log

# Watch errors only (CLEAN!)
tail -f logs/relayer-errors.log

# Watch ZKOS transactions
tail -f logs/zkos-tx-log

# Watch price updates
tail -f logs/relayer-price.log
```

### Searching Logs

```bash
# Search for specific patterns
grep "ERROR" logs/relayer-general.log
grep "order_123" logs/relayer-trading.log

# Search ZKOS transaction logs
grep "Transaction" logs/zkos-tx-log

# Search across all log files
grep -r "price update" logs/
```

## ZKOS Transaction Logging

The `zkos-tx-log` file captures logs from:

- `zkos-relayer-wallet` library
- `transaction` module
- `transactionapi` module
- `zkvm` module
- Any logs using `crate::log_zkos_tx!` macro

This helps separate transaction processing logs from general application logs.

### Example ZKOS Transaction Logging

```rust
// In your transaction processing code
crate::log_zkos_tx!(info, "Processing transaction: {}", tx_id);
crate::log_zkos_tx!(debug, "Transaction details: {:?}", transaction);

// For transaction errors
crate::log_zkos_tx!(error, "Transaction validation failed: {}", error);
```

## Troubleshooting

### No Logs Appearing

1. Check that `logging::init_logging()` is called in main
2. Verify the `logs/` directory exists and is writable
3. Check environment variables (`RUST_LOG`)

### Performance Concerns

- Logging is asynchronous and shouldn't impact performance significantly
- Adjust log levels in production (use `info` or `warn` instead of `debug`)
- Monitor disk space usage for log files

### Missing Category Logs

- Ensure you're using the correct target in your logging macros
- Check that the specific log filters are properly configured

### External Dependency Noise

If you see unwanted logs from external libraries:

- Check the console filter settings in `src/logging.rs`
- Add new noise reduction directives as needed

## Best Practices

1. **Use Appropriate Levels**:

   - `error` for actual errors that need attention
   - `warn` for potential issues
   - `info` for general progress updates
   - `debug` for detailed debugging information

2. **Use Structured Data**: Include relevant context in your logs

   ```rust
   tracing::error!(
       error = %err,
       user_id = user_id,
       action = "withdraw",
       "Failed to process withdrawal"
   );
   ```

3. **Use Spans for Context**: Wrap related operations in spans

   ```rust
   let span = tracing::info_span!("user_action", user_id = 123);
   let _guard = span.enter();
   // All logs within this scope will include the span context9
   ```

4. **Choose the Right Category**: Use specific logging macros for better organization
   - Trading operations → `log_trading!`
   - Price updates → `log_price!`
   - Health checks → `log_heartbeat!`
   - Database operations → `log_database!`
   - ZKOS transactions → `log_zkos_tx!`

For more examples and advanced usage, see the `src/logging_examples.rs` file in the project.
