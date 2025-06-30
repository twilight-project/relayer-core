use tracing::Level;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure logs directory exists
    std::fs::create_dir_all("logs")?;

    // Create different file appenders for different log types
    let (general_writer, _general_guard) =
        non_blocking(rolling::daily("logs", "relayer-general.log"));

    let (error_writer, _error_guard) = non_blocking(rolling::daily("logs", "relayer-errors.log"));

    let (trading_writer, _trading_guard) =
        non_blocking(rolling::daily("logs", "relayer-trading.log"));

    let (price_writer, _price_guard) = non_blocking(rolling::daily("logs", "relayer-price.log"));

    let (heartbeat_writer, _heartbeat_guard) =
        non_blocking(rolling::daily("logs", "relayer-heartbeat.log"));

    let (database_writer, _database_guard) =
        non_blocking(rolling::daily("logs", "relayer-database.log"));

    let (zkos_tx_writer, _zkos_tx_guard) = non_blocking(rolling::daily("logs", "zkos-tx-log"));

    // Create console writer for stdout
    let (console_writer, _console_guard) = non_blocking(std::io::stdout());

    // Create clean filters to avoid external dependency noise
    let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info")
            .add_directive("relayer::trading=debug".parse().unwrap())
            .add_directive("relayer::price=debug".parse().unwrap())
            .add_directive("relayer::heartbeat=debug".parse().unwrap())
            .add_directive("relayer::database=debug".parse().unwrap())
            .add_directive("relayer::zkos=info".parse().unwrap())
            // Reduce noise from external dependencies
            .add_directive("tungstenite=warn".parse().unwrap())
            .add_directive("tokio_tungstenite=warn".parse().unwrap())
            .add_directive("kafka=warn".parse().unwrap())
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap())
            .add_directive("mio=warn".parse().unwrap())
            .add_directive("want=warn".parse().unwrap())
            // Reduce zkos library noise in console (they go to dedicated zkos-tx-log file)
            .add_directive("zkos_relayer_wallet=warn".parse().unwrap())
            .add_directive("relayerwalletlib=warn".parse().unwrap())
            .add_directive("transaction=warn".parse().unwrap())
            .add_directive("transactionapi=warn".parse().unwrap())
            .add_directive("zkvm=warn".parse().unwrap())
            .add_directive("utxo_in_memory=warn".parse().unwrap())
            .add_directive("address=warn".parse().unwrap())
            .add_directive("zkschnorr=warn".parse().unwrap())
            .add_directive("quisquis_rust=warn".parse().unwrap())
    });

    // Error-only filter - strict ERROR level only for the error log
    let error_only_filter = EnvFilter::new("error")
        // Allow our application errors
        .add_directive("twilight_relayer_rust=error".parse().unwrap())
        .add_directive("relayer=error".parse().unwrap());

    // General filter - no trace/debug noise from external deps
    let general_filter = EnvFilter::new("info")
        .add_directive("relayer::trading=info".parse().unwrap())
        .add_directive("relayer::price=info".parse().unwrap())
        .add_directive("relayer::heartbeat=info".parse().unwrap())
        .add_directive("relayer::database=info".parse().unwrap())
        .add_directive("tungstenite=warn".parse().unwrap())
        .add_directive("tokio_tungstenite=warn".parse().unwrap())
        .add_directive("kafka=warn".parse().unwrap());

    // Console layer - shows filtered logs
    let console_layer = fmt::layer()
        .with_writer(console_writer)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(console_filter);

    // General log layer - INFO and WARN only, no external noise
    let general_layer = fmt::layer()
        .with_writer(
            general_writer
                .with_max_level(Level::WARN)
                .with_min_level(Level::INFO),
        )
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(general_filter);

    // Error log layer - ONLY actual errors, no trace/debug noise
    let error_layer = fmt::layer()
        .with_writer(error_writer.with_min_level(Level::ERROR))
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(error_only_filter);

    // Trading specific logs
    let trading_layer = fmt::layer()
        .with_writer(trading_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::new("relayer::trading=debug"));

    // Price feed specific logs
    let price_layer = fmt::layer()
        .with_writer(price_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::new("relayer::price=debug"));

    // Heartbeat specific logs
    let heartbeat_layer = fmt::layer()
        .with_writer(heartbeat_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::new("relayer::heartbeat=debug"));

    // Database specific logs
    let database_layer = fmt::layer()
        .with_writer(database_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::new("relayer::database=debug"));

    // ZKOS transaction logs - Capture zkos-relayer-wallet library logs WITHOUT modifying library
    let zkos_tx_layer = fmt::layer()
        .with_writer(zkos_tx_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(
            EnvFilter::new("off") // Start with everything off
                // Capture logs from zkos-relayer-wallet library directly
                .add_directive("zkos_relayer_wallet=debug".parse().unwrap())
                .add_directive("relayerwalletlib=debug".parse().unwrap())
                .add_directive("transaction=debug".parse().unwrap())
                .add_directive("transactionapi=debug".parse().unwrap())
                .add_directive("zkvm=debug".parse().unwrap())
                .add_directive("utxo_in_memory=debug".parse().unwrap())
                .add_directive("address=debug".parse().unwrap())
                .add_directive("zkschnorr=debug".parse().unwrap())
                .add_directive("quisquis_rust=debug".parse().unwrap())
                // Also capture any manual zkos logs from your app
                .add_directive("relayer::zkos=debug".parse().unwrap())
                // Block external noise explicitly
                .add_directive("tungstenite=off".parse().unwrap())
                .add_directive("tokio_tungstenite=off".parse().unwrap())
                .add_directive("kafka=off".parse().unwrap())
                .add_directive("hyper=off".parse().unwrap())
                .add_directive("reqwest=off".parse().unwrap())
                .add_directive("mio=off".parse().unwrap())
                .add_directive("want=off".parse().unwrap())
                .add_directive("h2=off".parse().unwrap()),
        );

    // Initialize the subscriber with all layers
    tracing_subscriber::registry()
        .with(console_layer)
        .with(general_layer)
        .with(error_layer)
        .with(trading_layer)
        .with(price_layer)
        .with(heartbeat_layer)
        .with(database_layer)
        .with(zkos_tx_layer)
        .init();

    // Keep guards alive for the lifetime of the program
    std::mem::forget(_general_guard);
    std::mem::forget(_error_guard);
    std::mem::forget(_trading_guard);
    std::mem::forget(_price_guard);
    std::mem::forget(_heartbeat_guard);
    std::mem::forget(_database_guard);
    std::mem::forget(_zkos_tx_guard);
    std::mem::forget(_console_guard);

    Ok(())
}

// Convenience macros for different log targets
#[macro_export]
macro_rules! log_trading {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(target: "relayer::trading", $($arg)*)
    };
}

#[macro_export]
macro_rules! log_price {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(target: "relayer::price", $($arg)*)
    };
}

#[macro_export]
macro_rules! log_heartbeat {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(target: "relayer::heartbeat", $($arg)*)
    };
}

#[macro_export]
macro_rules! log_database {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(target: "relayer::database", $($arg)*)
    };
}

#[macro_export]
macro_rules! log_zkos_tx {
    ($level:ident, $($arg:tt)*) => {
        tracing::$level!(target: "relayer::zkos", $($arg)*)
    };
}
