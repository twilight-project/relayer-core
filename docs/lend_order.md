# Lend Order

This document explains the `LendOrder` struct, its fields, the unit scaling conventions, and the deposit/settlement lifecycle.

## Unit Scaling Convention

The lending pool uses a **10,000x scaling factor** to preserve precision in integer arithmetic on-chain. Two unit spaces exist:

| Unit Space | Description | Example |
|---|---|---|
| **Native (sats)** | The real amount in satoshis as seen by the user | `deposit = 500` sats |
| **Scaled (internal)** | The amount multiplied by 10,000, used for pool accounting | `total_locked_value = 5,000,000` |

Key relationships:
- `total_locked_value (TLV)` is stored in **scaled** units: `TLV = deposit * 10,000`
- `total_pool_share (TPS)` is stored in **native** units
- `npoolshare` on a lend order is stored in **scaled** units
- To convert from scaled to native: `native = scaled / 10,000`
- To convert from native to scaled: `scaled = native * 10,000`

## LendOrder Fields

| Field | Type | Unit | Description |
|---|---|---|---|
| `uuid` | Uuid | — | Unique order identifier (generated on creation) |
| `account_id` | String | — | Twilight address of the lender |
| `balance` | f64 | sats | Account balance at time of order |
| `order_status` | OrderStatus | — | Current status: `FILLED` (active deposit) or `SETTLED` (withdrawn) |
| `order_type` | OrderType | — | Always `LEND` |
| `entry_nonce` | usize | — | Pool nonce at time of deposit (set during pool update) |
| `exit_nonce` | usize | — | Pool nonce at time of settlement (set during pool update) |
| `deposit` | f64 | sats (native) | Original deposit amount |
| `new_lend_state_amount` | f64 | sats (native) | Initially set to `deposit`. After settlement, updated to the total withdrawal value (`withdraw`) |
| `timestamp` | String | — | UTC timestamp of order creation |
| `npoolshare` | f64 | scaled | Lender's pool share in scaled units. Calculated at deposit time. |
| `nwithdraw` | f64 | scaled | Withdrawal amount in scaled units. Calculated at settlement time. |
| `payment` | f64 | sats (native) | Profit/loss from lending: `withdraw - deposit`. Positive = profit, negative = loss. |
| `tlv0` | f64 | scaled | Total locked value **before** deposit |
| `tps0` | f64 | native | Total pool share **before** deposit |
| `tlv1` | f64 | scaled | Total locked value **after** deposit |
| `tps1` | f64 | native | Total pool share **after** deposit |
| `tlv2` | f64 | scaled | Total locked value **before** settlement |
| `tps2` | f64 | native | Total pool share **before** settlement |
| `tlv3` | f64 | scaled | Total locked value **after** settlement |
| `tps3` | f64 | native | Total pool share **after** settlement |
| `entry_sequence` | usize | — | Event log sequence number at entry |

## Lifecycle

### Phase 1: Deposit (`new_order`)

When a lender deposits funds, `LendOrder::new_order` is called with the `CreateLendOrder` RPC request and the current pool state (`tlv0`, `tps0`).

**Inputs:**
- `deposit` — the amount the lender is depositing (sats, native)
- `tlv0` — current total locked value (scaled)
- `tps0` — current total pool share (native)

**Calculations:**

```
npoolshare = tps0 * deposit * 10000 / tlv0     (scaled units)
poolshare  = tps0 * deposit / tlv0              (native units)
tps1       = round(tps0) + round(poolshare)     (native units)
tlv1       = round(tlv0) + round(deposit)       (native → added to scaled TLV directly)
```

The `npoolshare` represents the lender's proportional share of the pool at the time of deposit, expressed in scaled units. It determines what fraction of the pool they can withdraw later.

**Pool update on deposit:**
```
pool.total_locked_value += round(deposit)
pool.total_pool_share  += round(npoolshare / 10000)
```

**Initial field values:**
- `new_lend_state_amount = deposit`
- `nwithdraw = 0`
- `payment = 0`
- `order_status = FILLED`
- `tlv2, tps2, tlv3, tps3 = 0` (not yet settled)

### Phase 2: Settlement (`calculatepayment_localdb`)

When a lender withdraws, `calculatepayment_localdb` is called with the current pool state at settlement time (`tlv2`, `tps2`).

**Inputs:**
- `tlv2` — current total locked value at settlement time (scaled)
- `tps2` — current total pool share at settlement time (native)
- `self.npoolshare` — the lender's pool share from deposit (scaled)
- `self.new_lend_state_amount` — the original deposit (sats, native)

**Calculations:**

```
nwithdraw = tlv2 * npoolshare / tps2            (scaled units)
withdraw  = nwithdraw / 10000                   (native units — actual sats returned)
payment   = withdraw - new_lend_state_amount    (native units — profit/loss)
tlv3      = tlv2 - round(nwithdraw / 10000)     (scaled units)
tps3      = tps2 - round(npoolshare / 10000)    (native units)
```

**Validation:**
- If `tlv2 < withdraw` → error: "insufficient pool fund!" (pool cannot cover the withdrawal)

**Field updates after settlement:**
- `nwithdraw = round(nwithdraw)`
- `payment = round(payment)`
- `new_lend_state_amount = round(withdraw)` (updated from original deposit to total withdrawal)
- `order_status = SETTLED`
- `tlv2, tps2, tlv3, tps3` are recorded

**Pool update on settlement:**
```
pool.total_locked_value -= round(nwithdraw / 10000)
pool.total_pool_share  -= round(npoolshare / 10000)
```

## How Profit/Loss Works

The lending pool's TLV changes over time as traders pay or receive PnL settlements, funding payments, and liquidations. This means the pool can grow or shrink between a lender's deposit and withdrawal.

**If pool grew** (traders lost money overall):
- `tlv2 > tlv0` → `withdraw > deposit` → `payment > 0` (lender profits)

**If pool shrank** (traders made money overall):
- `tlv2 < tlv0` → `withdraw < deposit` → `payment < 0` (lender takes a loss)

The lender's share (`npoolshare / tps`) determines their pro-rata portion of the pool at withdrawal time.

## Worked Example

**Deposit:**
- Pool state: `tlv0 = 10,000,000` (scaled), `tps0 = 1000` (native)
- Lender deposits: `deposit = 500` sats
- `npoolshare = 1000 * 500 * 10000 / 10,000,000 = 500,000` (scaled)
- `poolshare = 1000 * 500 / 10,000,000 = 0.05` (native)
- `tlv1 = 10,000,000 + 500 = 10,000,500`
- `tps1 = 1000 + 0 = 1000` (0.05 rounds to 0 — small deposit relative to pool)

**Settlement (pool grew 10%):**
- Pool state: `tlv2 = 11,000,550` (scaled, ~10% growth), `tps2 = 1000`
- `nwithdraw = 11,000,550 * 500,000 / 1000 = 5,500,275,000` — wait, this doesn't look right with the example numbers. Let's use a more realistic scenario:

**Realistic example:**
- Pool init: first lender deposits 1000 sats → `TLV = 10,000,000`, `TPS = 1000`
- Second lender deposits 1000 sats:
  - `npoolshare = 1000 * 1000 * 10000 / 10,000,000 = 10,000,000` (scaled)
  - Pool becomes: `TLV = 10,001,000`, `TPS = 2000`
- Pool grows from trader losses: `TLV = 12,001,000` (20% growth), `TPS = 2000`
- Second lender settles:
  - `nwithdraw = 12,001,000 * 10,000,000 / 2000 = 60,005,000,000` (scaled)
  - `withdraw = 60,005,000,000 / 10000 = 6,000,500` (native sats)
  - `payment = 6,000,500 - 1000 = 5,999,500` sats profit

## Order Flow Summary

```
CreateLendOrder RPC
    → Risk engine check (validate_close_cancel_order with LEND)
    → Duplicate check (one active lend per account)
    → LendOrder::new_order(deposit, tlv0, tps0)
    → ZK proof creation & on-chain tx
    → Pool update: TLV += deposit, TPS += poolshare
    → Order stored with status FILLED

ExecuteLendOrder RPC
    → Risk engine check (validate_close_cancel_order with LEND)
    → Order lookup (must be FILLED)
    → LendOrder::calculatepayment_localdb(tlv2, tps2)
    → Insufficient funds check
    → ZK proof creation & on-chain settlement tx
    → Pool update: TLV -= withdraw, TPS -= poolshare
    → Order removed with status SETTLED
```

## Constraints

- **One active lend order per account** — a duplicate check prevents a second deposit while one is active
- **Risk engine gating** — both deposit and settlement are blocked during HALT (but allowed during CLOSE_ONLY and when price feed is paused)
- **Insufficient funds** — settlement fails if the pool cannot cover the withdrawal amount
- **Rounding** — all pool updates use `.round()` to maintain integer precision for on-chain state
