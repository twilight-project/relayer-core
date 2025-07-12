// Example file demonstrating different logging capabilities
// This file shows how to use the various logging macros and targets

pub fn demonstrate_logging() {
    // General logging (goes to console and general log file)
    tracing::info!("This is a general info message");
    tracing::warn!("This is a general warning");
    tracing::error!("This is a general error");

    // Trading-specific logs (go to relayer-trading.log)
    crate::log_trading!(info, "New trading order received: {}", "order_123");
    crate::log_trading!(warn, "Trading volume threshold exceeded");
    crate::log_trading!(error, "Failed to execute trade: {}", "insufficient_funds");

    // Price-specific logs (go to relayer-price.log)
    crate::log_price!(info, "BTC price updated: ${}", 45000.50);
    crate::log_price!(debug, "Price feed connection established");
    crate::log_price!(error, "Price feed connection lost");

    // Heartbeat-specific logs (go to relayer-heartbeat.log)
    crate::log_heartbeat!(info, "Heartbeat check completed");
    crate::log_heartbeat!(warn, "Heartbeat response delayed");
    crate::log_heartbeat!(debug, "Scheduling next heartbeat in 60s");

    // Database-specific logs (go to relayer-database.log)
    crate::log_database!(info, "Database connection established");
    crate::log_database!(warn, "Database query took {}ms", 1500);
    crate::log_database!(error, "Database connection failed: {}", "timeout");

    // ZKOS transaction logs (go to zkos-tx-log)
    crate::log_zkos_tx!(info, "Transaction processed: {}", "tx_12345");
    crate::log_zkos_tx!(debug, "Transaction validation started");
    crate::log_zkos_tx!(error, "Transaction failed: {}", "insufficient_balance");

    // Advanced logging with structured data
    tracing::info!(
        user_id = 12345,
        action = "place_order",
        amount = 0.001,
        "User placed a new order"
    );

    // Logging with spans (for tracing execution flow)
    let span = tracing::info_span!("order_processing", order_id = "order_456");
    let _enter = span.enter();

    tracing::info!("Starting order validation");
    tracing::debug!("Checking user balance");
    tracing::info!("Order validation completed");

    // The span automatically ends when _enter is dropped
}

// Example of using logging in different modules
pub mod trading_module {
    pub fn process_order() {
        crate::log_trading!(info, "Processing new trading order");

        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(100));

        crate::log_trading!(debug, "Order validation completed");
        crate::log_trading!(info, "Order successfully processed");
    }
}

pub mod price_module {
    pub fn update_price(new_price: f64) {
        crate::log_price!(info, "Received price update: ${:.2}", new_price);

        if new_price > 50000.0 {
            crate::log_price!(warn, "Price is above $50k threshold: ${:.2}", new_price);
        }

        crate::log_price!(debug, "Price update processed successfully");
    }
}

pub mod database_module {
    pub fn save_data(data: &str) -> Result<(), &'static str> {
        crate::log_database!(debug, "Attempting to save data: {}", data);

        // Simulate database operation
        if data.is_empty() {
            crate::log_database!(error, "Cannot save empty data");
            return Err("Empty data");
        }

        crate::log_database!(info, "Data saved successfully");
        Ok(())
    }
}

pub mod zkos_transaction_module {
    pub fn process_zkos_transaction(tx_id: &str, amount: f64) -> Result<(), &'static str> {
        crate::log_zkos_tx!(info, "Starting ZKOS transaction processing: {}", tx_id);

        // Simulate transaction validation
        if amount <= 0.0 {
            crate::log_zkos_tx!(error, "Invalid transaction amount: {}", amount);
            return Err("Invalid amount");
        }

        crate::log_zkos_tx!(debug, "Transaction validation passed for {}", tx_id);

        // Simulate blockchain interaction
        crate::log_zkos_tx!(debug, "Submitting transaction to blockchain: {}", tx_id);

        // Simulate success
        crate::log_zkos_tx!(info, "ZKOS transaction completed successfully: {}", tx_id);
        Ok(())
    }

    pub fn validate_zkos_proof(proof_data: &str) -> bool {
        crate::log_zkos_tx!(debug, "Validating ZKOS proof: {}", proof_data);

        if proof_data.is_empty() {
            crate::log_zkos_tx!(error, "Empty proof data provided");
            return false;
        }

        // Simulate proof validation
        crate::log_zkos_tx!(info, "ZKOS proof validation successful");
        true
    }
}

// Example of conditional logging based on environment
pub fn conditional_logging_example() {
    // This will only log in debug builds
    tracing::debug!("This is debug information");

    // You can also use conditional compilation
    #[cfg(debug_assertions)]
    tracing::info!("This only appears in debug builds");

    #[cfg(not(debug_assertions))]
    tracing::info!("This only appears in release builds");
}
