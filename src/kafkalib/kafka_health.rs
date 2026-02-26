#![allow(dead_code)]
#![allow(unused_imports)]

use crate::config::BROKERS;
use kafka::producer::{AsBytes, Producer, Record, RequiredAcks};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// --- Global Kafka health state ---

/// When true, Kafka is considered unhealthy. Mirrors the STALE_PRICE_PAUSED pattern.
pub static KAFKA_UNHEALTHY: AtomicBool = AtomicBool::new(false);

static CONSECUTIVE_FAILURES: AtomicU64 = AtomicU64::new(0);
static CONSECUTIVE_SUCCESSES: AtomicU64 = AtomicU64::new(0);

// --- Health tracking functions ---

/// Record a successful Kafka operation. Resets failure counter, increments success counter.
/// When successes reach the recovery threshold and Kafka was unhealthy, clears the flag.
pub fn record_kafka_success() {
    CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);
    let successes = CONSECUTIVE_SUCCESSES.fetch_add(1, Ordering::Relaxed) + 1;

    if KAFKA_UNHEALTHY.load(Ordering::Relaxed)
        && successes >= *crate::config::KAFKA_RECOVERY_THRESHOLD
    {
        KAFKA_UNHEALTHY.store(false, Ordering::Relaxed);
        crate::log_heartbeat!(
            info,
            "KAFKA_HEALTH: Kafka recovered after {} consecutive successes — resuming normal operation",
            successes
        );
    }
}

/// Record a failed Kafka operation. Resets success counter, increments failure counter.
/// When failures reach the halt threshold, sets KAFKA_UNHEALTHY = true.
pub fn record_kafka_failure() {
    CONSECUTIVE_SUCCESSES.store(0, Ordering::Relaxed);
    let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;

    if !KAFKA_UNHEALTHY.load(Ordering::Relaxed)
        && failures >= *crate::config::KAFKA_FAILURE_HALT_THRESHOLD
    {
        KAFKA_UNHEALTHY.store(true, Ordering::Relaxed);
        crate::log_heartbeat!(
            error,
            "KAFKA_HEALTH: {} consecutive Kafka failures — setting KAFKA_UNHEALTHY=true",
            failures
        );
    }
}

/// Exponential backoff sleep: min(base_ms * 2^attempt, max_ms)
pub fn backoff_sleep(attempt: u64) {
    let base_ms = *crate::config::KAFKA_RETRY_BASE_MS;
    let max_ms = *crate::config::KAFKA_RETRY_MAX_MS;
    let delay_ms = std::cmp::min(
        base_ms.saturating_mul(1u64.checked_shl(attempt as u32).unwrap_or(u64::MAX)),
        max_ms,
    );
    std::thread::sleep(Duration::from_millis(delay_ms));
}

/// Attempt to create a Kafka Producer with retries and backoff.
/// Returns None if all retries fail (NO PANIC).
pub fn create_producer_with_retry(
    max_retries: u64,
    ack_timeout_secs: u64,
) -> Option<Producer> {
    dotenv::dotenv().ok();
    for attempt in 0..max_retries {
        match Producer::from_hosts(BROKERS.clone())
            .with_ack_timeout(Duration::from_secs(ack_timeout_secs))
            .with_required_acks(RequiredAcks::One)
            .create()
        {
            Ok(producer) => {
                if attempt > 0 {
                    crate::log_heartbeat!(
                        info,
                        "KAFKA_HEALTH: Producer created after {} retries",
                        attempt
                    );
                }
                return Some(producer);
            }
            Err(e) => {
                crate::log_heartbeat!(
                    warn,
                    "KAFKA_HEALTH: Producer creation attempt {}/{} failed: {:?}",
                    attempt + 1,
                    max_retries,
                    e
                );
                if attempt + 1 < max_retries {
                    backoff_sleep(attempt);
                }
            }
        }
    }
    crate::log_heartbeat!(
        error,
        "KAFKA_HEALTH: Failed to create producer after {} retries",
        max_retries
    );
    None
}

// --- ResilientProducer ---

/// A Kafka producer wrapper that handles reconnection and health tracking.
/// Holds a Mutex<Option<Producer>> internally — never panics on creation.
pub struct ResilientProducer {
    inner: Mutex<Option<Producer>>,
    ack_timeout_secs: u64,
}

// Safety: ResilientProducer only contains a Mutex (which is Send+Sync) and a u64
unsafe impl Sync for ResilientProducer {}

impl ResilientProducer {
    /// Create a new ResilientProducer. Attempts initial connection (3 tries).
    /// Stores None if all attempts fail — does NOT panic.
    pub fn new(ack_timeout_secs: u64) -> Self {
        let producer = create_producer_with_retry(3, ack_timeout_secs);
        if producer.is_none() {
            crate::log_heartbeat!(
                warn,
                "KAFKA_HEALTH: ResilientProducer starting with no connection (will retry on send)"
            );
        }
        ResilientProducer {
            inner: Mutex::new(producer),
            ack_timeout_secs,
        }
    }

    /// Send a record to Kafka. On failure, drops the broken producer, reconnects, and retries once.
    /// Calls record_kafka_success/failure on each outcome.
    pub fn send<'a, K, V>(&self, record: &Record<'a, K, V>) -> Result<(), String>
    where
        K: AsBytes,
        V: AsBytes,
    {
        let mut guard = self.inner.lock().map_err(|e| format!("Lock poisoned: {}", e))?;

        // If we have no producer, try to reconnect first
        if guard.is_none() {
            *guard = create_producer_with_retry(3, self.ack_timeout_secs);
            if guard.is_none() {
                record_kafka_failure();
                return Err("Kafka producer unavailable (reconnect failed)".to_string());
            }
        }

        // First attempt
        if let Some(ref mut producer) = *guard {
            match producer.send(record) {
                Ok(_) => {
                    record_kafka_success();
                    return Ok(());
                }
                Err(e) => {
                    crate::log_heartbeat!(
                        warn,
                        "KAFKA_HEALTH: Send failed, attempting reconnect: {:?}",
                        e
                    );
                }
            }
        }

        // Drop broken producer and reconnect
        *guard = None;
        *guard = create_producer_with_retry(3, self.ack_timeout_secs);

        // Retry once
        if let Some(ref mut producer) = *guard {
            match producer.send(record) {
                Ok(_) => {
                    record_kafka_success();
                    return Ok(());
                }
                Err(e) => {
                    *guard = None;
                    record_kafka_failure();
                    return Err(format!("Kafka send failed after reconnect: {:?}", e));
                }
            }
        }

        record_kafka_failure();
        Err("Kafka producer unavailable after reconnect attempt".to_string())
    }
}
