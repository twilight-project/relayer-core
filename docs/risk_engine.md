# Risk Engine

The risk engine is the central gatekeeper for all trading and lending operations. Every order — open, close, cancel, lend deposit, lend withdrawal — must pass through the risk engine before execution.

## Global State

The risk engine maintains two pieces of global state:

| State | Description |
|---|---|
| `RISK_ENGINE_STATE` | Tracks total filled long/short exposure, total pending long/short exposure, and manual control flags (halt, close-only, pause funding, pause price feed). |
| `RISK_PARAMS` | Configurable risk parameters loaded from environment variables at startup. Can be updated at runtime via admin commands. |
| `STALE_PRICE_PAUSED` | Atomic flag set when the price feed is detected as stale. |

## Risk Parameters

All parameters are loaded from environment variables with sensible defaults:

| Env Var | Parameter | Default | Description |
|---|---|---|---|
| `RISK_MAX_OI_MULT` | `max_oi_mult` (alpha) | 4.0 | Max total open interest as a multiple of pool equity |
| `RISK_MAX_NET_MULT` | `max_net_mult` (beta) | 0.8 | Max net directional exposure as a multiple of pool equity |
| `RISK_MAX_POSITION_PCT` | `max_position_pct` (gamma) | 0.02 | Max single position size as a fraction of pool equity |
| `RISK_MIN_POSITION_BTC` | `min_position_btc` | 0.0 | Minimum position size in sats (0 = disabled) |
| `RISK_MAX_LEVERAGE` | `max_leverage` | 50.0 | Maximum allowed leverage (0 = use existing limit) |
| `RISK_MM_RATIO` | `mm_ratio` | 0.4 | Maintenance margin ratio (0.4 = 40%) |

## Market Status

The market can be in one of three states:

| Status | Meaning |
|---|---|
| **HEALTHY** | Normal operation. All order types are allowed. |
| **CLOSE_ONLY** | Only close, cancel, and lend operations are allowed. No new positions can be opened. |
| **HALT** | Almost everything is blocked. Only lend operations are allowed (see details below). |

### How Market Status Is Determined

Status is evaluated in order of priority:

1. **Manual halt flag** is set → `HALT` (reason: `MANUAL_HALT`)
2. **Manual close-only flag** is set → `CLOSE_ONLY` (reason: `MANUAL_CLOSE_ONLY`)
3. **Pool equity <= 0** → `HALT` (reason: `POOL_EQUITY_INVALID`)
4. None of the above → `HEALTHY`

## Operation Permissions by Market State

### Trader Operations

| Operation | HEALTHY | CLOSE_ONLY | HALT |
|---|---|---|---|
| Open Market order | Allowed (subject to limits) | Rejected | Rejected |
| Open Limit order | Allowed (subject to limits) | Rejected | Rejected |
| Close Market order | Allowed | Allowed | Rejected |
| Close SLTP (SL/TP settle) | Allowed | Allowed | Rejected |
| Cancel Limit order | Allowed | Allowed | Rejected |

### Lender Operations

| Operation | HEALTHY | CLOSE_ONLY | HALT |
|---|---|---|---|
| Lend Deposit | Allowed | Allowed | Rejected |
| Lend Withdrawal | Allowed | Allowed | Rejected |
| Lend Settle | Allowed | Allowed | Rejected |

> **Note:** Lend operations use `validate_close_cancel_order` with `OrderType::LEND`. They are blocked during HALT but allowed during CLOSE_ONLY. Lend operations also bypass the price feed pause check — they can proceed even when the price feed is paused.

## Pre-Validation Checks (Before Market Status)

Two conditions are checked before anything else, regardless of market status:

### 1. Kafka Health

If Kafka is unhealthy (`KAFKA_UNHEALTHY = true`), **all operations are rejected** — opens, closes, cancels, and lends alike. This ensures event sourcing integrity: no order can be processed if events cannot be reliably persisted.

- Rejection reason: `KAFKA_UNHEALTHY`

### 2. Price Feed Pause

If the price feed is paused (either via admin flag `pause_price_feed` or stale price detection `STALE_PRICE_PAUSED`), the behavior depends on the operation type:

| Operation | Price Feed Paused? |
|---|---|
| Open orders (market/limit) | **Rejected** (`PRICE_FEED_PAUSED`) |
| Close / Cancel (non-lend) | **Rejected** (`PRICE_FEED_PAUSED`) |
| Lend operations | **Allowed** (lend operations bypass this check) |

## Open Order Validation (Full Pipeline)

When a new position is opened (market or limit), the risk engine runs this validation pipeline in order:

1. **Kafka health check** — reject if Kafka is unhealthy
2. **Price feed check** — reject if price feed is paused or stale
3. **Market status check** — reject if HALT or CLOSE_ONLY
4. **Basic param validation** — reject if IM <= 0 or leverage <= 0
5. **Max leverage check** — reject if leverage exceeds `max_leverage`
6. **Entry value calculation** — `entry_value = IM * leverage`
7. **Min size check** — reject if entry value < `min_position_btc`
8. **Limit checks** (evaluated in tightest-constraint-first order):
   - **Per-position cap** — reject if entry value > `max_position_pct * pool_equity`
   - **OI headroom** — reject if entry value > remaining OI capacity
   - **Net exposure headroom** — reject if entry value > remaining directional net capacity
   - **Combined directional limit** — reject if entry value > `min(OI headroom, net headroom, per-position cap)`

If all checks pass, the order is accepted and the entry value is returned.

## Limit Computation

The risk engine computes directional limits based on pool equity and current exposure:

```
oi_max       = max_oi_mult * pool_equity
net_max      = max_net_mult * pool_equity
pos_max      = max_position_pct * pool_equity

oi_headroom  = max(0, oi_max - current_oi)
net_headroom_long  = max(0, net_max - net_exposure)
net_headroom_short = max(0, net_max + net_exposure)

max_long  = min(oi_headroom, net_headroom_long, pos_max)
max_short = min(oi_headroom, net_headroom_short, pos_max)
```

Where:
- `current_oi = total_long + total_short`
- `net_exposure = total_long - total_short`

If the market is not HEALTHY, both `max_long` and `max_short` are forced to 0.

## Rejection Reasons

| Reason | Description |
|---|---|
| `HALT:{reason}` | Market is halted (manual or pool equity invalid) |
| `CLOSE_ONLY:{reason}` | Market is in close-only mode |
| `INVALID_PARAMS` | IM or leverage is <= 0 |
| `LEVERAGE_TOO_HIGH` | Requested leverage exceeds max |
| `BELOW_MIN_SIZE` | Position size below minimum |
| `SIZE_TOO_LARGE` | Position exceeds per-position cap |
| `OI_LIMIT_REACHED` | Total OI would exceed max |
| `SKEW_LIMIT_REACHED` | Net directional exposure would exceed max |
| `LIMIT_REACHED` | Combined limit exceeded |
| `PRICE_FEED_PAUSED` | Price feed is paused or stale |
| `KAFKA_UNHEALTHY` | Kafka is down, no event sourcing guarantee |

## Exposure Tracking

The risk engine tracks exposure through event-sourced state mutations:

- **`add_order`** — called when a position is filled; increments total long or short
- **`remove_order`** — called when a position is closed; decrements total long or short (floors at 0)
- **`add_pending_order`** — called when a limit order is placed; increments pending long or short
- **`remove_pending_order`** — called when a limit order is filled or cancelled; decrements pending long or short (floors at 0)
- **`recalculate_exposure`** — resets all four exposure fields to recomputed values (used during snapshot recovery or reconciliation)

Each mutation emits a `RiskEngineUpdate` event to the core event log for persistence and replay.

## Admin Controls

The following flags can be toggled at runtime:

| Command | Effect |
|---|---|
| `set_manual_halt(true/false)` | Puts the market into HALT / releases it |
| `set_manual_close_only(true/false)` | Puts the market into CLOSE_ONLY / releases it |
| `set_pause_funding(true/false)` | Pauses / resumes funding rate updates |
| `set_pause_price_feed(true/false)` | Pauses / resumes price feed (blocks opens and non-lend closes) |
| `update_risk_params(...)` | Replaces all risk parameters at runtime |

## `get_market_stats` API Method

The `get_market_stats` RPC method exposes the risk engine state to external consumers. It is served by the **relayer-data-api** (not the core relayer) and reads cached state from Redis rather than the in-memory globals.

### How It Works

1. **Read `RiskState` from Redis** — fetches the `risk_state` key. Falls back to a default empty `RiskState` if missing or unparseable.
2. **Read `RiskParams` from Redis** — fetches the `risk_params` key. Falls back to `RiskParams::from_env()` if missing.
3. **Read pool equity from Postgres** — queries the `lend_pool` table via `LendPool::get()` and calls `get_total_locked_value()`. Falls back to 0.0 on error.
4. **Compute stats** — replicates the same market status and limit computation logic as the core relayer (`compute_market_status` + `compute_limits`), producing a `MarketRiskStatsResponse`.

### RPC Details

- **HTTP Method:** `POST`
- **RPC Method:** `get_market_stats`
- **Parameters:** None (`params: null`)

### Request Example

```javascript
var raw = JSON.stringify({
  jsonrpc: "2.0",
  method: "get_market_stats",
  id: 123,
  params: null,
});

fetch("API_ENDPOINT/api", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: raw,
})
  .then((response) => response.json())
  .then((result) => console.log(result));
```

### Response Example

```json
{
  "jsonrpc": "2.0",
  "result": {
    "pool_equity_btc": 10.5,
    "total_long_btc": 3.2,
    "total_short_btc": 2.8,
    "total_pending_long_btc": 0.5,
    "total_pending_short_btc": 0.3,
    "open_interest_btc": 6.0,
    "net_exposure_btc": 0.4,
    "long_pct": 0.5333,
    "short_pct": 0.4667,
    "utilization": 0.5714,
    "max_long_btc": 2.5,
    "max_short_btc": 2.9,
    "status": "HEALTHY",
    "status_reason": null,
    "params": {
      "max_oi_mult": 4.0,
      "max_net_mult": 0.8,
      "max_position_pct": 0.02,
      "min_position_btc": 0.0,
      "max_leverage": 50.0,
      "mm_ratio": 0.4
    }
  },
  "id": 123
}
```

### Response Fields

| Field | Type | Description |
|---|---|---|
| `pool_equity_btc` | number | Total pool equity in BTC (from lending pool) |
| `total_long_btc` | number | Sum of entry values of all filled long positions |
| `total_short_btc` | number | Sum of entry values of all filled short positions |
| `total_pending_long_btc` | number | Sum of entry values of all pending long limit orders |
| `total_pending_short_btc` | number | Sum of entry values of all pending short limit orders |
| `open_interest_btc` | number | Total open interest (`total_long + total_short`) |
| `net_exposure_btc` | number | Net directional exposure (`total_long - total_short`) |
| `long_pct` | number | Fraction of OI that is long (0-1). 0 when OI is 0. |
| `short_pct` | number | Fraction of OI that is short (0-1). 0 when OI is 0. |
| `utilization` | number | OI / pool equity ratio. 0 when pool equity is 0. |
| `max_long_btc` | number | Maximum additional long entry value currently allowed. 0 if market is not HEALTHY. |
| `max_short_btc` | number | Maximum additional short entry value currently allowed. 0 if market is not HEALTHY. |
| `status` | string | `"HEALTHY"`, `"CLOSE_ONLY"`, or `"HALT"` |
| `status_reason` | string? | Reason for non-healthy status. `null` when HEALTHY. Values: `"MANUAL_HALT"`, `"MANUAL_CLOSE_ONLY"`, `"POOL_EQUITY_INVALID"` |
| `params` | object | Current risk parameters (see Risk Parameters section above) |

### Use Cases

- Real-time market health monitoring and circuit breaker status checks
- Position sizing — use `max_long_btc` / `max_short_btc` to know the largest order the engine will accept
- Utilization and capacity analysis for trading decisions
- Long/short balance tracking and net exposure monitoring
- Automated trading system integration for risk-aware order routing

### Architecture Note

The data-api reads from **Redis** (where the core relayer publishes state), not from the core relayer's in-memory state directly. This means there is a small propagation delay between a state change in the core relayer and its visibility in the API response. The computation logic (market status determination, limit calculation) is duplicated in the data-api to avoid a runtime dependency on the core relayer process.
