# Kafka Resilience — Preventing Panics & Auto-Halt on Kafka Outage

## Problem

When Kafka/Zookeeper goes down during runtime, the relayer-core **panics and crashes** due to `.expect()` calls in `lazy_static` producer initialization. There is no mechanism to:

1. Survive a Kafka outage without crashing
2. Inform the risk engine that Kafka is unavailable
3. Automatically recover when Kafka comes back

### Root Cause

Two `lazy_static` blocks create Kafka producers with `.expect()`, which panics if Kafka is unreachable:

```rust
// kafkacmd.rs — panics on startup if Kafka is down
pub static ref KAFKA_PRODUCER: Mutex<Producer> = {
    Producer::from_hosts(...)
        .create()
        .expect("Failed to create kafka producer");  // PANIC
};

// events.rs — same problem
pub static ref KAFKA_PRODUCER_EVENT: Mutex<Producer> = {
    Producer::from_hosts(...)
        .create()
        .expect("error in creating kafka producer");  // PANIC
};
```

Additionally, `producer_kafka.rs` created a **new producer for every single message** — wasteful and fragile.

The consumer loop in `kafkacmd.rs` used a fixed 1-second sleep on poll errors with no health escalation, and the `client_cmd_receiver` retry loop in `heartbeat.rs` had no backoff at all.

---

## Solution

### 1. `ResilientProducer` — Panic-Free Kafka Producer

**File:** `src/kafkalib/kafka_health.rs`

Instead of creating a producer with `.expect()` (which panics), we introduced `ResilientProducer` — a wrapper around `Mutex<Option<Producer>>`:

- **On creation:** Attempts 3 connection retries with exponential backoff. If all fail, stores `None` internally instead of panicking.
- **On send:** If the inner producer is `None`, attempts reconnection first. If a send fails, drops the broken producer, reconnects, and retries once.
- **Health tracking:** Every send success/failure calls `record_kafka_success()` / `record_kafka_failure()`.

```rust
// Before — panics
pub static ref KAFKA_PRODUCER: Mutex<Producer> = {
    Producer::from_hosts(BROKERS.clone())
        .create()
        .expect("Failed to create kafka producer");
    Mutex::new(producer)
};

// After — never panics
pub static ref KAFKA_PRODUCER: ResilientProducer = ResilientProducer::new(1);
```

The same replacement was applied to `KAFKA_PRODUCER_EVENT` in `events.rs`.

### 2. Global Kafka Health Flag

**File:** `src/kafkalib/kafka_health.rs`

A global `AtomicBool` flag, following the same pattern as the existing `STALE_PRICE_PAUSED`:

```rust
pub static KAFKA_UNHEALTHY: AtomicBool = AtomicBool::new(false);
```

**Health tracking logic:**

| Counter | Threshold (env var) | Default | Action |
|---------|-------------------|---------|--------|
| Consecutive failures | `KAFKA_FAILURE_HALT_THRESHOLD` | 10 | Sets `KAFKA_UNHEALTHY = true` |
| Consecutive successes | `KAFKA_RECOVERY_THRESHOLD` | 3 | Clears `KAFKA_UNHEALTHY = false` |

- `record_kafka_failure()` — increments failure counter, resets success counter. When failures >= threshold, sets the flag.
- `record_kafka_success()` — increments success counter, resets failure counter. When successes >= threshold and currently unhealthy, clears the flag.

**Why not reuse `manual_halt`?** Setting `manual_halt = true` writes an event to Kafka via `Event::new()`, which would fail when Kafka is down — creating a circular dependency. Also, admin-set halts shouldn't auto-clear. `KAFKA_UNHEALTHY` is volatile (resets on restart), which is correct behavior.

### 3. Exponential Backoff

**File:** `src/kafkalib/kafka_health.rs`

```rust
pub fn backoff_sleep(attempt: u64) {
    let delay_ms = min(base_ms * 2^attempt, max_ms);
    thread::sleep(Duration::from_millis(delay_ms));
}
```

| Env Var | Default | Purpose |
|---------|---------|---------|
| `KAFKA_RETRY_BASE_MS` | 500 | Base backoff delay |
| `KAFKA_RETRY_MAX_MS` | 30000 | Maximum backoff cap |
| `KAFKA_MAX_STARTUP_RETRIES` | 60 | Max retries for producer creation |

Applied to:
- Consumer poll error loop in `kafkacmd.rs` (replaced fixed 1s sleep)
- `client_cmd_receiver` retry loop in `heartbeat.rs` (replaced fixed 1s sleep)
- Producer creation retries inside `ResilientProducer`

### 4. Risk Engine Integration

**File:** `src/relayer/risk_engine.rs`

Added `KafkaUnhealthy` as a new rejection reason:

```rust
pub enum RiskRejectionReason {
    // ... existing variants ...
    KafkaUnhealthy,  // NEW
}
```

**Both opens AND close/cancel operations are blocked** when Kafka is down. Without Kafka, events cannot be sourced, so allowing any state changes would break the event log consistency:

```rust
// In validate_open_order():
if KAFKA_UNHEALTHY.load(Ordering::Relaxed) {
    return Err(RiskRejectionReason::KafkaUnhealthy);
}

// In validate_close_cancel_order():
if KAFKA_UNHEALTHY.load(Ordering::Relaxed) {
    return Err(RiskRejectionReason::KafkaUnhealthy);
}
```

The `MarketRiskStats` API response now includes a `kafka_unhealthy: bool` field so monitoring/dashboards can observe the state.

### 5. Shared Producer for `produce_main()`

**File:** `src/kafkalib/producer_kafka.rs`

The old implementation created a **new Kafka producer for every message** — an expensive operation the kafka crate docs explicitly warn against. Now it reuses the shared `KAFKA_PRODUCER`:

```rust
// Before — creates a new producer per call
pub fn produce_main(payload: &String, topic: &str) {
    let broker = std::env::var("BROKER")...;
    let mut producer = Producer::from_hosts(broker).create()?;
    producer.send(&Record::from_value(topic, data))?;
}

// After — reuses the shared resilient producer
pub fn produce_main(payload: &String, topic: &str) {
    let data = payload.as_bytes();
    if let Err(e) = KAFKA_PRODUCER.send(&Record::from_value(topic, data)) {
        log_heartbeat!(error, "Failed producing messages: {}", e);
    }
}
```

---

## Files Changed

| File | What Changed |
|------|-------------|
| `src/kafkalib/kafka_health.rs` | **NEW** — `KAFKA_UNHEALTHY` flag, `ResilientProducer`, health tracking, backoff |
| `src/kafkalib/mod.rs` | Registered `kafka_health` module |
| `src/config.rs` | Added 5 env vars for thresholds and backoff tuning |
| `src/kafkalib/kafkacmd.rs` | Replaced panicking `KAFKA_PRODUCER` with `ResilientProducer`; added health tracking to consumer poll loop and offset functions |
| `src/db/events.rs` | Replaced panicking `KAFKA_PRODUCER_EVENT` with `ResilientProducer`; simplified `send_event_to_kafka_queue()` |
| `src/kafkalib/producer_kafka.rs` | Rewrote `produce_main()` to use shared producer instead of creating one per message |
| `src/relayer/risk_engine.rs` | Added `KafkaUnhealthy` rejection reason; check in `validate_open_order()` and `validate_close_cancel_order()`; added `kafka_unhealthy` to `MarketRiskStats` |
| `src/relayer/heartbeat.rs` | Added failure tracking and exponential backoff to `client_cmd_receiver` retry loop |

---

## Configuration (Environment Variables)

| Variable | Default | Description |
|----------|---------|-------------|
| `KAFKA_FAILURE_HALT_THRESHOLD` | `10` | Consecutive failures before `KAFKA_UNHEALTHY=true` |
| `KAFKA_RECOVERY_THRESHOLD` | `3` | Consecutive successes before `KAFKA_UNHEALTHY=false` |
| `KAFKA_RETRY_BASE_MS` | `500` | Base backoff delay in milliseconds |
| `KAFKA_RETRY_MAX_MS` | `30000` | Maximum backoff delay cap in milliseconds |
| `KAFKA_MAX_STARTUP_RETRIES` | `60` | Maximum retries for producer creation at startup |

---

## Behavior Under Different Scenarios

### Startup with Kafka down
- `ResilientProducer::new()` retries 3 times with backoff, then stores `None` (no panic)
- Subsequent sends will keep attempting reconnection
- After `KAFKA_FAILURE_HALT_THRESHOLD` failures, `KAFKA_UNHEALTHY` is set
- All order operations are rejected with `KAFKA_UNHEALTHY` reason

### Runtime Kafka outage
- Consumer poll errors trigger `record_kafka_failure()` with exponential backoff
- Producer send failures trigger reconnection attempts inside `ResilientProducer`
- After threshold failures, `KAFKA_UNHEALTHY` flag is set
- Risk engine blocks all new opens AND close/cancel operations

### Recovery (Kafka comes back)
- Consumer polls and producer sends start succeeding again
- `record_kafka_success()` is called on each success
- After `KAFKA_RECOVERY_THRESHOLD` consecutive successes, `KAFKA_UNHEALTHY` is cleared
- Order operations resume normally

### Close/cancel during outage
- **Blocked** — returns `KAFKA_UNHEALTHY` rejection
- Without Kafka, events cannot be written to the event log, so no state changes are safe
