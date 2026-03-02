# `snapshot_and_reload.rs` — Documentation

## Overview

This file handles **saving and restoring the entire relayer state** to/from disk. It implements a snapshot mechanism that serializes all in-memory databases (trader orders, lend orders, lend pool, sorted sets, risk state, fee configs, UTXO output storage, queue state) into a single binary file. On startup, it reads this file back and replays any Kafka events that occurred after the snapshot was taken, bringing the relayer back to a consistent state.

---

## File Structure

### 1. Data Types

| Type | Purpose |
|------|---------|
| `OrderDBSnapShot<T>` | Generic snapshot of an order database (order table, sequence counters, zkos messages, account hash set) |
| `OrderDBSnapShotTO` | Type alias for `OrderDBSnapShot<TraderOrder>` |
| `OrderDBSnapShotLO` | Type alias for `OrderDBSnapShot<LendOrder>` |
| `SnapshotDBOldV1` through `SnapshotDBOldV6` | Legacy snapshot struct versions for backward-compatible migration |
| `SnapshotDB` | Current (V7/V8) snapshot struct containing all relayer state |
| `SnapshotBuilder` | Internal builder struct used during Kafka event replay to accumulate state |

### 2. Snapshot Versions

| Version | Changes |
|---------|---------|
| V1 | Original format — no SLTP sorted sets, no risk state |
| V2 | Added `sltp_close_long/short_sortedset_db` |
| V3 | Added `risk_state: RiskStateOld` |
| V4 | Added `risk_params: Option<RiskParamsOld>` |
| V5 | Updated to `RiskStateOldV5` (added pause_funding, pause_price_feed) |
| V6 | Updated to `RiskState` (current), still uses `RiskParamsOld` |
| V7 | Updated to `RiskParams` (with mm_ratio) — current struct layout |
| V8 | Same struct as V7 + **zstd compression** + version header + CRC32 checksum |

Each old version has a `migrate_to_new()` method that converts it to the next version, forming a migration chain (e.g., V1 → V2 → V3 → ... → V7).

### 3. Wire Format (V8+)

```
┌──────────────┬──────────────┬────────────────────────────┐
│ version (4B) │ crc32 (4B)   │ zstd-compressed payload    │
│ LE u32       │ LE u32       │ bincode-serialized SnapshotDB │
└──────────────┴──────────────┴────────────────────────────┘
```

- **Version**: identifies which struct to deserialize into
- **CRC32**: integrity check over the compressed payload bytes
- **Payload**: zstd-compressed bincode serialization of `SnapshotDB`

Legacy files (pre-V8) have no header and are raw bincode — handled by the fallback path.

### 4. Key Functions

| Function | Purpose |
|----------|---------|
| `snapshot()` | Main entry point. Reads snapshot file, decodes it (versioned or legacy), replays Kafka events via `create_snapshot_data()`, writes updated snapshot back to disk |
| `decode_versioned_snapshot()` | Reads version header, verifies CRC32, decompresses if V8+, deserializes the correct struct version |
| `decode_legacy_snapshot()` | Fallback trial-and-error deserialization for pre-header snapshot files (tries V7 down to V1) |
| `create_snapshot_data()` | Connects to Kafka, replays all events since the snapshot was taken using `SnapshotBuilder`, returns the updated state |
| `load_from_snapshot()` | Calls `snapshot()`, then writes all restored data into the global static variables (`TRADER_ORDER_DB`, `TRADER_LP_LONG`, etc.) |
| `write_snapshot_atomically()` | Writes to a temp file, then atomically renames it over the final path (crash-safe) |

### 5. SnapshotBuilder

The `SnapshotBuilder` struct encapsulates all mutable state during Kafka event replay. Each event type has a dedicated handler method:

| Method | Handles |
|--------|---------|
| `handle_trader_order()` | `Event::TraderOrder` — create, cancel, execute, settle-on-limit |
| `handle_trader_order_limit_update()` | `Event::TraderOrderLimitUpdate` — zkos msg updates for limit fills |
| `handle_trader_order_update()` | `Event::TraderOrderUpdate` — order state changes (e.g., PENDING → FILLED) |
| `handle_trader_order_funding_update()` | `Event::TraderOrderFundingUpdate` — liquidation price changes from funding |
| `handle_trader_order_liquidation()` | `Event::TraderOrderLiquidation` — order liquidated and removed |
| `handle_lend_order()` | `Event::LendOrder` — create and execute lend orders |
| `handle_fee_update()` | `Event::FeeUpdate` — fee rate changes |
| `handle_current_price_update()` | `Event::CurrentPriceUpdate` — price tick + queue manager bulk operations |
| `handle_sorted_set_update()` | `Event::SortedSetDBUpdate` — all sorted set add/remove/update/bulk-search operations |
| `handle_output_storage()` | `Event::TxHash` and `Event::TxHashUpdate` — UTXO output storage add/remove |

### 6. Global State Restored

`load_from_snapshot()` writes into these globals (defined in `localdb.rs`):

- `TRADER_ORDER_DB` — trader order table (spawned in a separate thread)
- `LEND_ORDER_DB` — lend order table (spawned in a separate thread)
- `LEND_POOL_DB` — lend pool state
- `TRADER_LP_LONG / SHORT` — liquidation price sorted sets
- `TRADER_LIMIT_OPEN_LONG / SHORT` — open limit price sorted sets
- `TRADER_LIMIT_CLOSE_LONG / SHORT` — close limit price sorted sets
- `TRADER_SLTP_CLOSE_LONG / SHORT` — stop-loss/take-profit sorted sets
- `POSITION_SIZE_LOG` — position size tracking
- `RISK_ENGINE_STATE` — risk engine state (from `risk_engine.rs`)
- `RISK_PARAMS` — risk parameters (from `risk_engine.rs`)
- `OUTPUT_STORAGE` — UTXO output hex storage (from `config.rs`)
- Fee rates and current price via `set_localdb()` / `set_fee()`

---

## Improvements Over Older Code

### 1. Version Header (was: nested trial-and-error deserialization)

**Before:** The `snapshot()` function tried to deserialize V7, then V6, then V5... down to V1, with **12 levels of nested `match` blocks** (~70 lines of deeply indented code). Adding V8 would have made it worse.

**After:** A 4-byte version header is prepended to the file. On read, the version is read first and a **flat `match`** dispatches to the correct deserializer. Adding V9 is a single new match arm. Legacy files without headers still work via `decode_legacy_snapshot()` (flat `if let` chain).

### 2. CRC32 Checksum (was: no integrity checking)

**Before:** A partially written or bit-rotted snapshot file would produce a confusing bincode deserialization error, or worse, **silently load corrupt data**.

**After:** A CRC32 checksum is written alongside the payload and verified on load before any deserialization is attempted. Corrupt files now produce a clear error message: `"Snapshot checksum mismatch: stored=0x..., computed=0x..."`.

### 3. SnapshotBuilder Pattern (was: ~900-line monolithic function)

**Before:** `create_snapshot_data()` was a single ~900-line function with a giant `match` inside a `while` loop. All event handlers (trader orders, lend orders, sorted sets, tx hashes, fees, prices, risk state, etc.) were inline.

**After:** Event handlers are extracted into methods on a `SnapshotBuilder` struct. The main loop is now ~40 lines. Each handler is independently readable and testable.

### 4. TxHash Deduplication (was: ~110 lines of copy-paste)

**Before:** `Event::TxHash` (~55 lines) and `Event::TxHashUpdate` (~55 lines) had nearly identical code doing uuid-to-bytes conversion, hex decoding, bincode deserialization, and output storage operations.

**After:** Both call a single `handle_output_storage()` method (~35 lines).

### 5. `lock_or_err!` Macro (was: ~15 identical 7-line blocks)

**Before:** `load_from_snapshot()` had ~15 identical lock acquisition blocks, each 7-10 lines:
```rust
let mut x = match GLOBAL.lock() {
    Ok(lock) => lock,
    Err(arg) => {
        crate::log_heartbeat!(error, "Error locking GLOBAL : {:?}", arg);
        return Err(arg.to_string());
    }
};
```

**After:** Each is a single line:
```rust
let mut x = lock_or_err!(GLOBAL, "GLOBAL");
```

### 6. Generic `OrderDBSnapShot<T>` (was: two identical structs)

**Before:** `OrderDBSnapShotTO` and `OrderDBSnapShotLO` were two separate structs with identical fields, `new()`, `remove_order_check()`, and `set_order_check()` — duplicated across ~60 lines.

**After:** A single generic `OrderDBSnapShot<T>` with type aliases:
```rust
pub type OrderDBSnapShotTO = OrderDBSnapShot<TraderOrder>;
pub type OrderDBSnapShotLO = OrderDBSnapShot<LendOrder>;
```

### 7. Atomic Write Fix (was: unnecessary delete before rename)

**Before:** `write_snapshot_atomically()` called `fs::remove_file()` before `fs::rename()`. This created a brief window where **no snapshot file exists** — a crash during that window = data loss.

**After:** The `remove_file` call is removed. `fs::rename()` atomically replaces the destination on Unix. No crash-unsafe window.

### 8. Zstd Compression (was: uncompressed bincode)

**Before:** Snapshot files were raw bincode — no compression applied. Large order tables meant large files.

**After:** The payload is compressed with zstd (level 3) before writing. Typical compression ratio is **3-5x** with minimal CPU cost (~400MB/s compress, ~1GB/s decompress). The snapshot logs the compression ratio on each write.

### 9. Removed Unnecessary `.clone()` Calls

**Before:**
- `.clone()` on `usize` fields (which are `Copy` types — cloning is unnecessary)
- `.clone()` on owned values when constructing the final `SnapshotDB` (the variables were already owned and could be moved)
- `.clone()` on `f64` references in `load_from_snapshot`

**After:** `Copy` types are assigned directly. Owned values are moved via `SnapshotBuilder::build()`. References are dereferenced with `*value`.

### 10. Snapshot Metadata Logging (was: only basic status logs)

**Before:** After loading a snapshot, only logged whether each database "was loaded" or "not found" — no counts or sizes.

**After:** `log_metadata()` logs a detailed summary:
```
Snapshot loaded: 1523 trader orders, 42 lend orders,
liq_long=500, liq_short=300, open_long=120, open_short=80,
close_long=45, close_short=30, sltp_long=15, sltp_short=10,
offset=(0, 54321), timestamp=2024-01-15T10:30:00Z
```

---

## Summary of Impact

| Category | Before | After |
|----------|--------|-------|
| Deserialization | 12 levels of nested match | Flat version-based dispatch |
| Integrity | None | CRC32 checksum |
| Compression | None | zstd (~3-5x smaller) |
| `create_snapshot_data` | ~900 lines, 1 function | ~40-line loop + 10 handler methods |
| TxHash handling | ~110 lines duplicated | ~35 lines shared |
| Lock boilerplate | ~105 lines (15 x 7) | ~15 lines (15 x 1) |
| Order DB structs | 2 identical structs | 1 generic + 2 type aliases |
| Atomic write | Crash-unsafe delete window | Direct atomic rename |
| Cloning | Unnecessary on Copy types | Direct assignment / move |
| Debug logging | Minimal | Full metadata summary |
