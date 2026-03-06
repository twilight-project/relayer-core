# Relayer Core - Order Flow Reference

> This document describes how the relayer-core handles trading orders end-to-end: creation, execution, settlement, cancellation, and liquidation. It is intended for AI agents working on the relayer-data-api archiver to understand how to interpret events for order book and recent order state.

---

## Table of Contents

1. [Core Types](#1-core-types)
2. [Event Enum](#2-event-enum)
3. [CRITICAL: Command order_type vs TraderOrder order_type](#3-critical-command-order_type-vs-traderorder-order_type)
4. [Order Lifecycle - Opening](#4-order-lifecycle---opening-createtraderorder)
5. [Order Lifecycle - Closing](#5-order-lifecycle---closing-executetraderorder--executetraderordersltp)
6. [Cancellation Flows](#6-cancellation-flows)
7. [Liquidation Flow](#7-liquidation-flow)
8. [RPC Command Handling (order_handler.rs)](#8-rpc-command-handling)
9. [RelayerCommand Handling (relayer_command_handler.rs)](#9-relayercommand-handling)
10. [Price Ticker & Heartbeat](#10-price-ticker--heartbeat)
11. [Funding Cycle](#11-funding-cycle)
12. [Sorted Set Indices](#12-sorted-set-indices)
13. [Event-to-State Mapping for Archiver](#13-event-to-state-mapping-for-archiver)
14. [TraderOrderLimitUpdate Detail](#14-traderorderlimitupdate---detailed-explanation)
15. [Status Transition Diagrams](#15-status-transition-diagrams)

---

## 1. Core Types

> Source: `twilight-client-sdk/src/relayer_types.rs` (SDK types) and `relayer-core/src/relayer/commands.rs` (relayer-internal types)

### 1.1 Enums

#### OrderType
```rust
pub enum OrderType {
    MARKET,  // Filled immediately at current price
    LIMIT,   // Pending until price reaches entry price
    SLTP,    // Stop-Loss / Take-Profit conditional close order
    LEND,    // Lending pool deposit/withdrawal (separate flow)
    DARK,    // Not currently implemented
}
```

#### OrderStatus
```rust
pub enum OrderStatus {
    // Core lifecycle statuses
    PENDING,              // Limit order waiting for price trigger
    FILLED,               // Order filled, position is open
    SETTLED,              // Position closed, payment calculated
    CANCELLED,            // Cancelled by user or system
    LIQUIDATE,            // Position liquidated (margin depleted)
    LENDED,               // Lend order is active in pool

    // SLTP-specific cancel statuses
    CancelledStopLoss,    // SLTP stop-loss leg cancelled
    CancelledTakeProfit,  // SLTP take-profit leg cancelled

    // Error statuses
    DuplicateOrder,       // Duplicate order detected
    OrderNotFound,        // Order does not exist
    Error,                // Generic error
    UtxoError,            // UTXO error
    NoResponseFromChain,  // Chain did not respond
    RejectedFromChain,    // Chain rejected the transaction
    BincodeError,         // Bincode serialization error
    HexCodeError,         // Hex encoding error
    SerializationError,   // Generic serialization error

    // Informational statuses
    RequestSubmitted,     // Order submitted to relayer
    FilledUpdated,        // Order filled and state updated, awaiting settlement
}
```

#### PositionType
```rust
pub enum PositionType {
    LONG,   // Bet on price increase (scalar: -1)
    SHORT,  // Bet on price decrease (scalar: 1)
}
```

#### RequestStatus
Largely identical to `OrderStatus`. Used in RPC response handling.
```rust
pub enum RequestStatus {
    SETTLED, LENDED, LIQUIDATE, CANCELLED, PENDING, FILLED,
    DuplicateOrder, UtxoError, Error, NoResponseFromChain,
    BincodeError, HexCodeError, SerializationError,
    RequestSubmitted, OrderNotFound, RejectedFromChain, FilledUpdated,
}
```

#### SlTpOrderType
```rust
pub enum SlTpOrderType {
    StopLoss,
    TakeProfit,
}
```

### 1.2 RPC Request Structs (from SDK - what the client sends)

#### CreateTraderOrder
Used in `RpcCommand::CreateTraderOrder` and `RpcCommand::CreateTraderOrderSlTp`.
```rust
pub struct CreateTraderOrder {
    pub account_id: String,          // User's account identifier
    pub position_type: PositionType, // LONG or SHORT
    pub order_type: OrderType,       // MARKET, LIMIT, or SLTP (drives open behavior)
    pub leverage: f64,               // 1x to 50x
    pub initial_margin: f64,         // Collateral amount in BTC
    pub available_margin: f64,       // Initially same as initial_margin
    pub order_status: OrderStatus,   // Client-suggested status (overridden by relayer)
    pub entryprice: f64,             // Desired entry price (used for LIMIT; ignored for MARKET)
    pub execution_price: f64,        // Desired close price (used later for settlement)
}
```

#### ExecuteTraderOrder
Used in `RpcCommand::ExecuteTraderOrder` and `RpcCommand::ExecuteTraderOrderSlTp`.
```rust
pub struct ExecuteTraderOrder {
    pub account_id: String,          // User's account identifier
    pub uuid: Uuid,                  // UUID of the existing order to close/settle
    pub order_type: OrderType,       // MARKET, LIMIT, or SLTP (drives close behavior)
    pub settle_margin: f64,          // Margin amount for settlement
    pub order_status: OrderStatus,   // Client-suggested status (overridden by relayer)
    pub execution_price: f64,        // Desired close price (for LIMIT close)
}
```

#### CancelTraderOrder
Used in `RpcCommand::CancelTraderOrder` and `RpcCommand::CancelTraderOrderSlTp`.
```rust
pub struct CancelTraderOrder {
    pub account_id: String,          // User's account identifier
    pub uuid: Uuid,                  // UUID of the order to cancel
    pub order_type: OrderType,       // Order type being cancelled
    pub order_status: OrderStatus,   // Client-suggested status
}
```

#### SlTpOrder
Carries stop-loss and take-profit prices. Both are optional (you can set SL only, TP only, or both).
```rust
pub struct SlTpOrder {
    pub sl: Option<f64>,  // Stop-loss trigger price (None = no stop-loss)
    pub tp: Option<f64>,  // Take-profit trigger price (None = no take-profit)
}
```

#### SlTpOrderCancel
Flags for which SLTP legs to cancel. Can cancel independently.
```rust
pub struct SlTpOrderCancel {
    pub sl: bool,  // true = cancel the stop-loss leg
    pub tp: bool,  // true = cancel the take-profit leg
}
```

#### CreateLendOrder
```rust
pub struct CreateLendOrder {
    pub account_id: String,
    pub balance: f64,
    pub order_type: OrderType,    // Always LEND
    pub order_status: OrderStatus,
    pub deposit: f64,             // Amount to deposit into lending pool
}
```

#### ExecuteLendOrder
```rust
pub struct ExecuteLendOrder {
    pub account_id: String,
    pub uuid: Uuid,
    pub order_type: OrderType,      // Always LEND
    pub settle_withdraw: f64,       // Percentage to withdraw from lending position
    pub order_status: OrderStatus,
    pub poolshare_price: f64,       // Pool share price at time of withdrawal
}
```

### 1.3 ZKOS Types (blockchain layer)

#### ZkosCreateOrder
Zero-knowledge proof components for creating an order on-chain.
```rust
pub struct ZkosCreateOrder {
    pub input: Input,            // The UTXO input being spent
    pub output: Output,          // The output memo with commitments
    pub signature: Signature,    // zkSchnorr signature proving ownership
    pub proof: SigmaProof,       // Proof of same value between input and output
}
```

#### ZkosSettleMsg
Zero-knowledge proof components for settling an order.
```rust
pub struct ZkosSettleMsg {
    pub output: Output,          // Output memo from original create order
    pub signature: Signature,    // zkSchnorr signature authorizing settlement
}
```

#### ZkosCancelMsg
Zero-knowledge proof components for cancelling an order.
```rust
pub struct ZkosCancelMsg {
    pub public_key: String,      // User's hex-encoded account address
    pub signature: Signature,    // zkSchnorr signature authorizing cancellation
}
```

#### ZkosQueryMsg
Zero-knowledge proof components for querying order state. Structurally identical to ZkosCancelMsg.
```rust
pub struct ZkosQueryMsg {
    pub public_key: String,      // User's hex-encoded account address
    pub signature: Signature,    // Signature authorizing the query
}
```

#### ClientMemoTx
Pairs a ZKOS transaction with its output memo (used internally before final message construction).
```rust
pub struct ClientMemoTx {
    pub tx: Transaction,
    pub output: Output,
}
```

### 1.4 Composite Client Messages (what the client actually sends to relayer)

These structs bundle the order metadata with ZKOS proof components. They are serialized to hex strings and sent via RPC.

#### Standard Trader Order Messages

```rust
// Create order: metadata + ZK proof
pub struct CreateTraderOrderZkos {
    pub create_trader_order: CreateTraderOrder,
    pub input: ZkosCreateOrder,
}

// Alternative create (with full Transaction instead of ZkosCreateOrder)
pub struct CreateTraderOrderClientZkos {
    pub create_trader_order: CreateTraderOrder,
    pub tx: Transaction,
}

// Execute/settle order: metadata + ZK settle proof
pub struct ExecuteTraderOrderZkos {
    pub execute_trader_order: ExecuteTraderOrder,
    pub msg: ZkosSettleMsg,
}

// Cancel order: metadata + ZK cancel proof
pub struct CancelTraderOrderZkos {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosCancelMsg,
}
```

#### SLTP Trader Order Messages (supersets of standard messages)

```rust
// Create SLTP order: metadata + Transaction + optional SL/TP + optional settle msg
pub struct CreateTraderOrderClientZkosSlTp {
    pub create_trader_order: CreateTraderOrder,
    pub tx: Transaction,
    pub sltp: Option<SlTpOrder>,       // SL/TP prices (optional)
    pub msg: Option<ZkosSettleMsg>,    // Pre-signed settle msg (optional)
}

// Execute SLTP order: metadata + ZK settle proof + optional SL/TP
pub struct ExecuteTraderOrderZkosSlTp {
    pub execute_trader_order: ExecuteTraderOrder,
    pub msg: ZkosSettleMsg,
    pub sltp: Option<SlTpOrder>,       // SL/TP prices (optional)
}

// Cancel SLTP order: metadata + ZK cancel proof + which legs to cancel
pub struct CancelTraderOrderZkosSlTp {
    pub cancel_trader_order: CancelTraderOrder,
    pub msg: ZkosCancelMsg,
    pub sltp_cancel: SlTpOrderCancel,  // Which SL/TP legs to cancel
}
```

#### Lend Order Messages

```rust
// Create lend order: metadata + ZK proof
pub struct CreateLendOrderZkos {
    pub create_lend_order: CreateLendOrder,
    pub input: ZkosCreateOrder,
}

// Execute/settle lend order: metadata + ZK settle proof
pub struct ExecuteLendOrderZkos {
    pub execute_lend_order: ExecuteLendOrder,
    pub msg: ZkosSettleMsg,
}
```

#### Query Messages

```rust
// Query trader order by account and status
pub struct QueryTraderOrder {
    pub account_id: String,
    pub order_status: OrderStatus,
}

pub struct QueryTraderOrderZkos {
    pub query_trader_order: QueryTraderOrder,
    pub msg: ZkosQueryMsg,
}

// Query lend order by account and status
pub struct QueryLendOrder {
    pub account_id: String,
    pub order_status: OrderStatus,
}

pub struct QueryLendOrderZkos {
    pub query_lend_order: QueryLendOrder,
    pub msg: ZkosQueryMsg,
}
```

#### TXType
Distinguishes between trading and lending transaction types.
```rust
pub enum TXType {
    ORDERTX,  // Standard perpetual contract trade order
    LENDTX,   // Lending or borrowing order for the DeFi pool
}
```

### 1.5 Full Order State Structs (relayer-side, stored in DB)

#### TraderOrder
```rust
pub struct TraderOrder {
    pub uuid: Uuid,                  // Unique order ID (generated by relayer)
    pub account_id: String,          // User's account identifier
    pub position_type: PositionType, // LONG or SHORT
    pub order_status: OrderStatus,   // Current status (PENDING/FILLED/SETTLED/etc.)
    pub order_type: OrderType,       // How it was OPENED (MARKET or LIMIT, set once)
    pub entryprice: f64,             // Actual entry price
    pub execution_price: f64,        // Close/settlement price
    pub positionsize: f64,           // Position size in BTC
    pub leverage: f64,               // Leverage multiplier
    pub initial_margin: f64,         // Original collateral
    pub available_margin: f64,       // Remaining margin (decreases with funding)
    pub timestamp: String,           // ISO 8601 creation time
    pub bankruptcy_price: f64,       // Price at which position value = 0
    pub bankruptcy_value: f64,       // Value at bankruptcy price
    pub maintenance_margin: f64,     // Minimum margin before liquidation
    pub liquidation_price: f64,      // Price at which position gets liquidated
    pub unrealized_pnl: f64,         // Unrealized profit/loss
    pub settlement_price: f64,       // Price at which position was closed
    pub entry_nonce: usize,          // ZKOS nonce at entry
    pub exit_nonce: usize,           // ZKOS nonce at exit
    pub entry_sequence: usize,       // Aggregate log sequence at entry
    pub fee_filled: f64,             // Fee charged on position open
    pub fee_settled: f64,            // Fee charged on position close
}
```

#### LendOrder
```rust
pub struct LendOrder {
    pub uuid: Uuid,
    pub account_id: String,
    pub balance: f64,
    pub order_status: OrderStatus,
    pub order_type: OrderType,
    pub entry_nonce: usize,
    pub exit_nonce: usize,
    pub deposit: f64,                // Deposit amount
    pub new_lend_state_amount: f64,
    pub timestamp: String,
    pub npoolshare: f64,             // Number of pool shares received
    pub nwithdraw: f64,              // Withdrawal amount
    pub payment: f64,                // Payment from pool
    pub tlv0: f64,                   // Total locked value before lend tx
    pub tps0: f64,                   // Total pool shares before lend tx
    pub tlv1: f64,                   // Total locked value after lend tx
    pub tps1: f64,                   // Total pool shares after lend tx
    pub tlv2: f64,                   // Total locked value before settlement
    pub tps2: f64,                   // Total pool shares before settlement
    pub tlv3: f64,                   // Total locked value after settlement
    pub tps3: f64,                   // Total pool shares after settlement
    pub entry_sequence: usize,
}
```

#### TxHash (response/tracking record)
```rust
pub struct TxHash {
    pub id: i64,                     // Database ID
    pub order_id: Uuid,              // Order UUID
    pub account_id: String,
    pub tx_hash: String,             // Blockchain transaction hash
    pub order_type: OrderType,
    pub order_status: OrderStatus,
    pub datetime: String,            // ISO 8601 timestamp
    pub output: Option<String>,      // Hex-encoded ZKOS Output (if any)
    pub request_id: Option<String>,  // Client request ID for correlation
}
```

### 1.6 RpcCommand (relayer-internal, wraps SDK structs + metadata)

```rust
pub enum RpcCommand {
    // Standard order commands
    CreateTraderOrder(CreateTraderOrder, Meta, ZkosHexString, RequestID),
    ExecuteTraderOrder(ExecuteTraderOrder, Meta, ZkosHexString, RequestID),
    CancelTraderOrder(CancelTraderOrder, Meta, ZkosHexString, RequestID),

    // Lend commands
    CreateLendOrder(CreateLendOrder, Meta, ZkosHexString, RequestID),
    ExecuteLendOrder(ExecuteLendOrder, Meta, ZkosHexString, RequestID),

    // SLTP variants (supersets of standard commands)
    CreateTraderOrderSlTp(CreateTraderOrder, Option<SlTpOrder>, Option<ZkosSettleMsg>, Meta, ZkosHexString, RequestID),
    ExecuteTraderOrderSlTp(ExecuteTraderOrder, Option<SlTpOrder>, Meta, ZkosHexString, RequestID),
    CancelTraderOrderSlTp(CancelTraderOrder, SlTpOrderCancel, Meta, ZkosHexString, RequestID),

    // Internal: settle limit order triggered by price ticker
    RelayerCommandTraderOrderSettleOnLimit(TraderOrder, Meta, f64),
}

// Type aliases
type ZkosHexString = String;  // Hex-encoded ZKOS message
type RequestID = String;       // Client-provided request correlation ID

pub struct Meta {
    pub metadata: HashMap<String, Option<String>>,  // Key-value metadata (prices, fees, etc.)
}
```

### 1.7 RelayerCommand (system-initiated, not from user)
```rust
pub enum RelayerCommand {
    FundingCycle(PoolBatchOrder, Meta, f64),              // Hourly funding rate application
    FundingOrderEventUpdate(TraderOrder, Meta),           // Per-order funding update
    PriceTickerLiquidation(Vec<Uuid>, Meta, f64),         // Orders to liquidate at price
    PriceTickerOrderFill(Vec<Uuid>, Meta, f64),           // Pending limits to fill at price
    PriceTickerOrderSettle(Vec<Uuid>, Meta, f64),         // Close limits to settle at price
    FundingCycleLiquidation(Vec<Uuid>, Meta, f64),        // Post-funding liquidations (not implemented)
    RpcCommandPoolupdate(),                               // Trigger lend pool batch processing
    UpdateFees(f64, f64, f64, f64),                       // Update fee rates
}
```

### 1.8 SortedSetCommand
```rust
pub enum SortedSetCommand {
    // Liquidation price tracking
    AddLiquidationPrice(Uuid, f64, PositionType),
    RemoveLiquidationPrice(Uuid, PositionType),
    UpdateLiquidationPrice(Uuid, f64, PositionType),
    BulkSearchRemoveLiquidationPrice(f64, PositionType),

    // Open limit order price tracking (pending entry)
    AddOpenLimitPrice(Uuid, f64, PositionType),
    RemoveOpenLimitPrice(Uuid, PositionType),
    UpdateOpenLimitPrice(Uuid, f64, PositionType),
    BulkSearchRemoveOpenLimitPrice(f64, PositionType),

    // Close limit order price tracking (pending exit)
    AddCloseLimitPrice(Uuid, f64, PositionType),
    RemoveCloseLimitPrice(Uuid, PositionType),
    UpdateCloseLimitPrice(Uuid, f64, PositionType),
    BulkSearchRemoveCloseLimitPrice(f64, PositionType),

    // SLTP close limit price tracking
    AddStopLossCloseLIMITPrice(Uuid, f64, PositionType),
    AddTakeProfitCloseLIMITPrice(Uuid, f64, PositionType),
    RemoveStopLossCloseLIMITPrice(Uuid, PositionType),
    RemoveTakeProfitCloseLIMITPrice(Uuid, PositionType),
    UpdateStopLossCloseLIMITPrice(Uuid, f64, PositionType),
    UpdateTakeProfitCloseLIMITPrice(Uuid, f64, PositionType),
    BulkSearchRemoveSLTPCloseLIMITPrice(f64, PositionType),
}
```

### 1.9 Risk Engine Types
```rust
pub struct RiskState {
    pub total_long_btc: f64,
    pub total_short_btc: f64,
    pub total_pending_long_btc: f64,
    pub total_pending_short_btc: f64,
    pub manual_halt: bool,
    pub manual_close_only: bool,
    pub pause_funding: bool,
    pub pause_price_feed: bool,
}

pub enum RiskEngineCommand {
    AddExposure(PositionType, f64),
    RemoveExposure(PositionType, f64),
    AddPendingExposure(PositionType, f64),
    RemovePendingExposure(PositionType, f64),
    RecalculateExposure,
    SetManualHalt(bool),
    SetManualCloseOnly(bool),
    SetPauseFunding(bool),
    SetPausePriceFeed(bool),
}

pub struct RiskParams {
    pub max_oi_mult: f64,        // Max open interest multiplier
    pub max_net_mult: f64,       // Max net exposure multiplier
    pub max_position_pct: f64,   // Max single position as % of pool
    pub min_position_btc: f64,   // Minimum position size in BTC
    pub max_leverage: f64,       // Maximum leverage allowed
    pub mm_ratio: f64,           // Maintenance margin ratio
}
```

### 1.10 Fee Types
```
FilledOnMarket  - Fee for market order fill (opening position)
FilledOnLimit   - Fee for limit order fill (opening position)
SettledOnMarket - Fee for market order settle (closing position)
SettledOnLimit  - Fee for limit order settle (closing position)
```

---

## 2. Event Enum

All state changes are recorded as events in a Kafka topic (`CORE_EVENT_LOG`). These events are the **single source of truth** for state reconstruction.

```rust
enum Event {
    // Order lifecycle events (include the full TraderOrder snapshot + triggering command + sequence)
    TraderOrder(TraderOrder, RpcCommand, usize),              // New order created
    TraderOrderUpdate(TraderOrder, RelayerCommand, usize),    // Order updated (e.g. limit fill)
    TraderOrderLimitUpdate(TraderOrder, RpcCommand, usize),   // Limit settle awaiting price
    TraderOrderFundingUpdate(TraderOrder, RelayerCommand),    // Funding rate applied to order
    TraderOrderLiquidation(TraderOrder, RelayerCommand, usize), // Order liquidated

    // Lend pool events
    LendOrder(LendOrder, RpcCommand, usize),
    PoolUpdate(LendPoolCommand, LendPool, usize),

    // Price and rate events
    FundingRateUpdate(f64, f64, String),       // funding_rate, btc_price, iso8601_time
    CurrentPriceUpdate(f64, String),           // btc_price, iso8601_time

    // Sorted set (order book index) mutations
    SortedSetDBUpdate(SortedSetCommand, String), // command, iso8601_time

    // Position tracking
    PositionSizeLogDBUpdate(PositionSizeLogCommand, PositionSizeLog),

    // Transaction hash records (for external tracking / API responses)
    TxHash(Uuid, String, String, OrderType, OrderStatus, String, Option<String>, RequestID),
    //     order_id, account_id, tx_hash, order_type, status, datetime, output, request_id
    TxHashUpdate(Uuid, String, String, OrderType, OrderStatus, String, Option<String>),
    //          order_id, account_id, tx_hash, order_type, status, datetime, output

    // System events
    Stop(String),                                          // Snapshot boundary marker
    AdvanceStateQueue(Nonce, Output),                      // ZKOS state advance
    FeeUpdate(RelayerCommand, String),                     // Fee config change, time
    RiskEngineUpdate(RiskEngineCommand, RiskState),        // Risk state mutation
    RiskParamsUpdate(RiskParams),                          // Risk parameter change
}
```

### Key Points for Archiver

- **`Event::TxHash`** is the primary event for tracking order results. It contains the order UUID, account ID, status, order type, and request ID. This is emitted for every RPC command response.
- **`Event::TxHashUpdate`** is similar but without RequestID - used for system-triggered updates (liquidations, limit fills).
- **`Event::TraderOrder`** contains the full `TraderOrder` struct snapshot at creation time.
- **`Event::TraderOrderUpdate`** contains the full `TraderOrder` struct after a limit fill (PENDING -> FILLED).
- **`Event::TraderOrderLiquidation`** contains the order at liquidation time.
- **`Event::SortedSetDBUpdate`** tracks all order book index changes - add/remove/update of prices in sorted sets.

---

## 3. CRITICAL: Command order_type vs TraderOrder order_type

**These are two different things. Do not confuse them.**

- **`TraderOrder.order_type`** = How the position was **opened** (MARKET or LIMIT). Set once at creation and never changes.
- **`cmd_order_type`** (from `CreateTraderOrder.order_type`, `ExecuteTraderOrder.order_type`, etc.) = How this **specific command** should be processed. Sent with each RPC request.

**The open and close order types are independent.** Any combination is valid:

| Opened With (TraderOrder.order_type) | Closed With (ExecuteTraderOrder.order_type) | Result |
|---|---|---|
| MARKET | MARKET | Open at current price, close at current price (immediate settle) |
| MARKET | LIMIT | Open at current price, close only when limit price is reached |
| MARKET | SLTP | Open at current price, close when SL or TP triggers |
| LIMIT | MARKET | Open when entry price reached, close at current price (immediate settle) |
| LIMIT | LIMIT | Open when entry price reached, close when limit price reached |
| LIMIT | SLTP | Open when entry price reached, close when SL or TP triggers |

**In `check_for_settlement()`**, the `cmd_order_type` parameter determines the close behavior:
- `cmd_order_type = MARKET` -> Always settles immediately at current price (uses `FilledOnMarket` fee)
- `cmd_order_type = LIMIT` -> Only settles if close price condition met, otherwise registers a close limit (uses `FilledOnLimit` fee)
- `cmd_order_type = SLTP` -> Only settles if SL or TP price condition met, otherwise registers SL/TP limits (uses `FilledOnMarket` fee)

**Also during creation in `new_order()`**: If a user sends `CreateTraderOrder` with `order_type=LIMIT` but the entry price is already crossed (e.g., LONG with entry >= current price), the relayer **converts it to MARKET** internally. So `TraderOrder.order_type` may end up as MARKET even if the user sent LIMIT.

### CRITICAL: SlTp Commands Are Supersets, Not Separate Flows

`CreateTraderOrderSlTp` and `ExecuteTraderOrderSlTp` are **upgraded versions** of `CreateTraderOrder` and `ExecuteTraderOrder`. They are NOT limited to SLTP order types. The only difference is they carry an extra `Option<SlTpOrder>` parameter.

**They use the same core logic:**
- `CreateTraderOrderSlTp` calls `TraderOrder::new_order(rpc_request)` - same function as `CreateTraderOrder`. The `rpc_request.order_type` drives the behavior (MARKET, LIMIT, or SLTP).
- `ExecuteTraderOrderSlTp` calls `check_for_settlement(execution_price, current_price, rpc_request.order_type, sltp)` - same function as `ExecuteTraderOrder`. The `rpc_request.order_type` drives the behavior.

**So if you send `CreateTraderOrderSlTp` with `order_type=MARKET`:**
- It behaves exactly like `CreateTraderOrder` with `order_type=MARKET`
- The `SlTpOrder` parameter is simply ignored during `new_order()`
- The position opens at current price, status=FILLED

**If you send `ExecuteTraderOrderSlTp` with `order_type=LIMIT`:**
- It behaves exactly like `ExecuteTraderOrder` with `order_type=LIMIT`
- The `sltp` parameter is ignored because `check_for_settlement()` matches on `cmd_order_type=LIMIT`, not SLTP
- Only `cmd_order_type=SLTP` triggers the SL/TP price checking logic

**In practice**, clients can use the SlTp variants for all order operations. The cmd `order_type` field determines which code path runs:

| cmd order_type sent to SlTp variant | Behavior |
|---|---|
| `MARKET` | Identical to non-SlTp variant with MARKET |
| `LIMIT` | Identical to non-SlTp variant with LIMIT |
| `SLTP` | Uses the SL/TP prices from `SlTpOrder` for conditional close |

---

## 4. Order Lifecycle - Opening (CreateTraderOrder)

### 4.1 Opening with cmd order_type = MARKET

```
User sends CreateTraderOrder (cmd order_type=MARKET)
  |
  v
new_order():
  - TraderOrder.order_type = MARKET
  - status = FILLED, entry_price = current_price
  - Fee = FilledOnMarket
  |
  v
Risk validation (validate_open_order)
  |-- REJECTED --> Event::TxHash(CANCELLED) + done
  |
  v
Duplicate check (set_order_check)
  |-- DUPLICATE --> Event::TxHash(DuplicateOrder) + done
  |
  v
ZKOS blockchain tx (CreateTraderOrderTX)
  |-- FAIL --> remove_order_check() + Event::TxHash(Error)
  |
  v
orderinsert_localdb():
  - Add to TRADER_LP_LONG/SHORT (liquidation price)
  - Add filled exposure to RiskState
  - Update PositionSizeLog
  |
  v
Event::TraderOrder(order, cmd, seq)
Event::TxHash(uuid, account_id, tx_hash, MARKET, FILLED, time, output, request_id)
```

### 4.2 Opening with cmd order_type = LIMIT

```
User sends CreateTraderOrder (cmd order_type=LIMIT)
  |
  v
new_order():
  - If LONG and entry_price >= current_price:
      CONVERTED TO MARKET -> TraderOrder.order_type = MARKET, same as 4.1
  - If SHORT and entry_price <= current_price:
      CONVERTED TO MARKET -> TraderOrder.order_type = MARKET, same as 4.1
  - Otherwise:
      TraderOrder.order_type = LIMIT
      status = PENDING, entry_price = requested_price
      Fee = FilledOnLimit (charged later when filled)
  |
  v
Risk validation + duplicate check (same as above)
  |
  v
For PENDING orders (not converted to MARKET):
  orderinsert_localdb():
    - Add to TRADER_LIMIT_OPEN_LONG/SHORT (entry price)
    - Add pending exposure to RiskState
  |
  v
Event::TxHash(uuid, account_id, "", LIMIT, PENDING, time, None, request_id)

--- Later: Price ticker detects entry price reached ---

RelayerCommand::PriceTickerOrderFill(order_ids, meta, current_price)
  |
  v
pending_order(current_price):
  - Recalculate position metrics with actual fill price
  - status = FILLED
  - Move from pending -> filled exposure in RiskState
  |
  v
ZKOS tx (CreateTraderOrderLIMITTX)
  |-- FAIL --> cancelorder_localdb() + remove
  |
  v
orderinsert_localdb():
  - Remove from TRADER_LIMIT_OPEN_LONG/SHORT
  - Add to TRADER_LP_LONG/SHORT (liquidation price)
  |
  v
Event::TraderOrderUpdate(order, cmd, seq)
```

### 4.3 Opening with CreateTraderOrderSlTp

Same as 4.1 or 4.2 for the opening. The difference is that **after the position is FILLED**, the SL/TP prices are immediately registered:

```
After position is FILLED:
  set_execution_price_for_limit_order_stoploss_takeprofit_localdb():
    - For StopLoss price:
      - LONG position: Add to TRADER_SLTP_CLOSE_SHORT (SL triggers when price drops)
      - SHORT position: Add to TRADER_SLTP_CLOSE_LONG (SL triggers when price rises)
    - For TakeProfit price:
      - LONG position: Add to TRADER_SLTP_CLOSE_LONG (TP triggers when price rises)
      - SHORT position: Add to TRADER_SLTP_CLOSE_SHORT (TP triggers when price drops)
  |
  v
Event::SortedSetDBUpdate(AddStopLossCloseLIMITPrice(...))
Event::SortedSetDBUpdate(AddTakeProfitCloseLIMITPrice(...))
```

---

## 5. Order Lifecycle - Closing (ExecuteTraderOrder / ExecuteTraderOrderSlTp)

**The close command's `order_type` determines how the close is processed, regardless of how the position was opened.**

### 5.1 Closing with cmd order_type = MARKET

```
User sends ExecuteTraderOrder (cmd order_type=MARKET) for any FILLED position
  |
  v
check_for_settlement(execution_price, current_price, cmd_order_type=MARKET, None):
  - ALWAYS settles immediately at current_price
  - calculatepayment_localdb(current_price, FilledOnMarket fee)
  - Returns (payment, SETTLED)
  |
  v
ZKOS settlement tx (ExecuteTraderOrderTX)
  |
  v
order_remove_from_localdb():
  - Remove from TRADER_LP_LONG/SHORT
  - Remove from TRADER_LIMIT_CLOSE_LONG/SHORT (if had a close limit)
  - Remove exposure from RiskState
  - Remove from PositionSizeLog
  |
  v
Event::TxHash(uuid, account_id, tx_hash, order_type, SETTLED, time, output, request_id)

NOTE: The order_type in TxHash is from the CMD, not the TraderOrder.
This position could have been opened as MARKET or LIMIT - doesn't matter.
```

### 5.2 Closing with cmd order_type = LIMIT

```
User sends ExecuteTraderOrder (cmd order_type=LIMIT, execution_price=X) for any FILLED position
  |
  v
check_for_settlement(execution_price=X, current_price, cmd_order_type=LIMIT, None):
  |
  ├── If price condition MET (LONG: X <= current, SHORT: X >= current):
  |     - calculatepayment_localdb(current_price, FilledOnLimit fee)
  |     - Returns (payment, SETTLED)
  |     - Same settlement flow as 5.1
  |
  └── If price condition NOT MET:
        - set_execution_price_for_limit_order_localdb(X)
          -> Add to TRADER_LIMIT_CLOSE_LONG/SHORT
          -> Event::SortedSetDBUpdate(AddCloseLimitPrice)
        - Stores ZKOS hex string for later use
        - Returns (0, FILLED) -- still FILLED, not settled yet
        |
        v
      Event::TraderOrderLimitUpdate(order, ExecuteTraderOrder cmd, sequence)
      Event::TxHash(uuid, account_id, request_id, order_type, PENDING, time, None, request_id)
        |
        v
      --- Later: Price ticker detects close price reached ---
      RelayerCommand::PriceTickerOrderSettle(order_ids, meta, current_price)
        - check_for_settlement() with MARKET type (forced settle at current price)
        - SETTLED -> order_remove_from_localdb()
```

### 5.3 Closing with cmd order_type = SLTP

```
User sends ExecuteTraderOrderSlTp (cmd order_type=SLTP, SlTpOrder{sl, tp}) for any FILLED position
  |
  v
check_for_settlement(execution_price, current_price, cmd_order_type=SLTP, Some(sltp)):
  |
  ├── StopLoss check (if sl is Some):
  |     LONG: sl >= current_price -> SETTLED (price dropped to SL)
  |     SHORT: sl <= current_price -> SETTLED (price rose to SL)
  |     Fee: FilledOnMarket (SL always uses market fee)
  |
  ├── TakeProfit check (if tp is Some):
  |     LONG: tp <= current_price -> SETTLED (price rose to TP)
  |     SHORT: tp >= current_price -> SETTLED (price dropped to TP)
  |     Fee: FilledOnMarket (TP always uses market fee)
  |
  └── Neither triggered:
        - set_execution_price_for_limit_order_stoploss_takeprofit_localdb()
          -> Add SL/TP to TRADER_SLTP_CLOSE_LONG/SHORT sorted sets
          -> Event::SortedSetDBUpdate(AddStopLossCloseLIMITPrice)
          -> Event::SortedSetDBUpdate(AddTakeProfitCloseLIMITPrice)
        - Stores ZKOS hex string for later use
        - Returns (0, FILLED) -- still FILLED
        |
        v
      Event::TraderOrderLimitUpdate(order, ExecuteTraderOrderSlTp cmd, sequence)
      Event::TxHash(uuid, account_id, request_id, order_type, PENDING, time, None, request_id)
        |
        v
      --- Later: Price ticker detects SL or TP price reached ---
      RelayerCommand::PriceTickerOrderSettle(order_ids, meta, current_price)
        - check_for_settlement() with MARKET type (forced settle at current price)
        - SETTLED -> order_remove_from_localdb()
```

---

## 6. Cancellation Flows

```
--- Cancel PENDING LIMIT order (status=PENDING) ---
CancelTraderOrder -> cancelorder_localdb():
  - Remove from TRADER_LIMIT_OPEN_LONG/SHORT
  - Remove pending exposure from RiskState
  - Event::SortedSetDBUpdate(RemoveOpenLimitPrice)
  - Event::TxHash(CANCELLED)

--- Cancel close limit on FILLED position (has pending close limit) ---
CancelTraderOrder -> cancel_close_limit_order():
  - Remove from TRADER_LIMIT_CLOSE_LONG/SHORT
  - Event::SortedSetDBUpdate(RemoveCloseLimitPrice)
  - Event::TxHash(CANCELLED)
  NOTE: The position itself is STILL OPEN (FILLED). Only the pending close is cancelled.

--- Cancel SLTP legs on FILLED position ---
CancelTraderOrderSlTp -> cancel_sltp_order(SlTpOrderCancel{sl, tp}):
  If sl=true:
    - Remove from TRADER_SLTP_CLOSE_LONG/SHORT
    - Event::SortedSetDBUpdate(RemoveStopLossCloseLIMITPrice)
    - Returns CancelledStopLoss status
  If tp=true:
    - Remove from TRADER_SLTP_CLOSE_LONG/SHORT
    - Event::SortedSetDBUpdate(RemoveTakeProfitCloseLIMITPrice)
    - Returns CancelledTakeProfit status
  NOTE: The position itself is STILL OPEN (FILLED). Only the SL/TP triggers are removed.
  User can cancel SL only, TP only, or both independently.
```

---

## 7. Liquidation Flow

```
Price ticker detects liquidation price reached:
  - TRADER_LP_LONG: liquidation_price >= current_price (LONG position underwater)
  - TRADER_LP_SHORT: liquidation_price <= current_price (SHORT position underwater)
  |
  v
RelayerCommand::PriceTickerLiquidation(order_ids, meta, current_price)
  |
  v
For each order (status must be FILLED or LIQUIDATE):
  order.liquidate(current_price):
    - available_margin = 0
    - payment = -initial_margin (entire margin lost to lend pool)
  |
  v
  Remove from PositionSizeLog, RiskState
  Remove from close limit sorted sets (if any pending close)
  |
  v
  ZKOS liquidation tx
  |
  v
  Event::TraderOrderLiquidation(order, cmd, seq)
  Event::SortedSetDBUpdate(RemoveLiquidationPrice)
  Event::SortedSetDBUpdate(RemoveCloseLimitPrice) (if had close limit)
  Add to LendPoolCommand::AddTraderOrderLiquidation
```

---

## 8. RPC Command Handling

**File**: `src/relayer/core/order_handler.rs`

Commands arrive via Kafka (`RPC_CLIENT_REQUEST` topic) and are routed by `rpc_event_handler()`.

| RPC Command | Thread Pool | Description |
|---|---|---|
| `CreateTraderOrder` | THREADPOOL_NORMAL_ORDER | Open new MARKET/LIMIT position |
| `CreateTraderOrderSlTp` | THREADPOOL_NORMAL_ORDER | Open new position with SL/TP |
| `ExecuteTraderOrder` | THREADPOOL_FIFO_ORDER | Close/settle existing position |
| `ExecuteTraderOrderSlTp` | THREADPOOL_FIFO_ORDER | Close with SL/TP conditions |
| `CancelTraderOrder` | THREADPOOL_URGENT_ORDER | Cancel open/pending order |
| `CancelTraderOrderSlTp` | THREADPOOL_URGENT_ORDER | Cancel SLTP legs |
| `CreateLendOrder` | THREADPOOL_FIFO_ORDER | Deposit into lending pool |
| `ExecuteLendOrder` | THREADPOOL_FIFO_ORDER | Withdraw from lending pool |

---

## 9. RelayerCommand Handling

**File**: `src/relayer/core/relayer_command_handler.rs`

These are system-triggered (not user-initiated).

| RelayerCommand | Trigger | Action |
|---|---|---|
| `PriceTickerOrderFill` | Price reaches open limit entry | Converts PENDING -> FILLED, executes ZKOS tx |
| `PriceTickerOrderSettle` | Price reaches close limit price | Settles position, calculates payment |
| `PriceTickerLiquidation` | Price reaches liquidation price | Liquidates position, zeroes margin |
| `FundingCycle` | Hourly scheduler | Applies funding rate, emits pool update |
| `FundingOrderEventUpdate` | After funding calculation | Updates order liquidation price |
| `FundingCycleLiquidation` | After funding (not implemented) | Placeholder |
| `UpdateFees` | Admin action | Updates fee configuration |
| `RpcCommandPoolupdate` | After order settlement | Triggers lend pool batch processing |

---

## 10. Price Ticker & Heartbeat

**File**: `src/relayer/heartbeat.rs`

The heartbeat runs a **250ms loop** that:

1. Gets current BTC price from Binance websocket feed
2. Emits `Event::CurrentPriceUpdate(price, time)`
3. Spawns 3 parallel checks:

```
check_pending_limit_order_on_price_ticker_update_localdb():
  TRADER_LIMIT_OPEN_LONG: Find orders where entry_price <= current_price -> fill them
  TRADER_LIMIT_OPEN_SHORT: Find orders where entry_price >= current_price -> fill them
  -> RelayerCommand::PriceTickerOrderFill

check_liquidating_orders_on_price_ticker_update_localdb():
  TRADER_LP_LONG: Find LONG orders where liquidation_price >= current_price -> liquidate
  TRADER_LP_SHORT: Find SHORT orders where liquidation_price <= current_price -> liquidate
  -> RelayerCommand::PriceTickerLiquidation

check_settling_limit_order_on_price_ticker_update_localdb():
  TRADER_LIMIT_CLOSE_LONG: Find LONG close limits where price <= current_price -> settle
  TRADER_LIMIT_CLOSE_SHORT: Find SHORT close limits where price >= current_price -> settle
  TRADER_SLTP_CLOSE_LONG: Same for SLTP long-side close prices
  TRADER_SLTP_CLOSE_SHORT: Same for SLTP short-side close prices
  -> RelayerCommand::PriceTickerOrderSettle
```

---

## 11. Funding Cycle

Runs every **1 hour**. For each FILLED order:

```
funding_payment = (funding_rate * position_size) / (current_price * 100.0)

LONG positions:  available_margin -= funding_payment  (longs pay when rate > 0)
SHORT positions: available_margin += funding_payment  (shorts receive when rate > 0)

Recalculate maintenance_margin and liquidation_price.

If available_margin <= maintenance_margin:
    -> Liquidate order immediately
Else:
    -> Update liquidation_price in TRADER_LP sorted set
    -> Event::TraderOrderFundingUpdate(updated_order, cmd)
```

Funding rate formula:
```
funding_rate = ((total_long - total_short) / total_position_size)^2 / (8 * psi)
If total_long <= total_short: funding_rate *= -1
```

---

## 12. Sorted Set Indices

These are price-indexed sorted sets used for fast trigger detection:

| Sorted Set | Contents | Position | Trigger Condition |
|---|---|---|---|
| `TRADER_LP_LONG` | Liquidation prices | LONG orders | `liq_price >= current_price` |
| `TRADER_LP_SHORT` | Liquidation prices | SHORT orders | `liq_price <= current_price` |
| `TRADER_LIMIT_OPEN_LONG` | Entry prices | LONG pending limits | `entry_price <= current_price` |
| `TRADER_LIMIT_OPEN_SHORT` | Entry prices | SHORT pending limits | `entry_price >= current_price` |
| `TRADER_LIMIT_CLOSE_LONG` | Close prices | LONG close limits | `close_price <= current_price` |
| `TRADER_LIMIT_CLOSE_SHORT` | Close prices | SHORT close limits | `close_price >= current_price` |
| `TRADER_SLTP_CLOSE_LONG` | SL/TP prices | Long-side SL/TP | `price <= current_price` |
| `TRADER_SLTP_CLOSE_SHORT` | SL/TP prices | Short-side SL/TP | `price >= current_price` |

**SLTP sorted set mapping (important):**
- LONG + StopLoss -> TRADER_SLTP_CLOSE_SHORT (SL is below entry, triggers on price drop)
- LONG + TakeProfit -> TRADER_SLTP_CLOSE_LONG (TP is above entry, triggers on price rise)
- SHORT + StopLoss -> TRADER_SLTP_CLOSE_LONG (SL is above entry, triggers on price rise)
- SHORT + TakeProfit -> TRADER_SLTP_CLOSE_SHORT (TP is below entry, triggers on price drop)

---

## 13. Event-to-State Mapping for Archiver

This section describes what each event means for the archiver's order book and recent orders state.

### Order Book (Open Orders)

| Event | Action on Order Book |
|---|---|
| `TraderOrder(order, RpcCommand::CreateTraderOrder, seq)` | **Add** order. If FILLED -> active position. If PENDING -> pending limit. |
| `TraderOrderUpdate(order, PriceTickerOrderFill, seq)` | **Update** order from PENDING -> FILLED. Move from pending limits to active positions. |
| `TraderOrderLimitUpdate(order, cmd, seq)` | **Update** order's close limit price. Position is still FILLED but has a pending close. |
| `TraderOrderFundingUpdate(order, cmd)` | **Update** order's margin, liquidation price. Position is still FILLED. |
| `TraderOrderLiquidation(order, cmd, seq)` | **Remove** order from book. Position liquidated. |
| `TxHash(..., SETTLED, ...)` | **Remove** order from book. Position closed. |
| `TxHash(..., CANCELLED, ...)` | **Remove** pending order from book (was PENDING limit). |
| `TxHash(..., PENDING, ...)` | **Confirm** a pending limit order or pending close limit exists. |
| `TxHash(..., FILLED, ...)` | **Confirm** a position is open. |

### Recent Orders / Trade History

| Event | What to Record |
|---|---|
| `TxHash(uuid, account_id, tx_hash, order_type, FILLED, time, output, request_id)` | Order opened (market fill or limit fill) |
| `TxHash(uuid, account_id, tx_hash, order_type, SETTLED, time, output, request_id)` | Order closed (position settled) |
| `TxHash(uuid, account_id, tx_hash, order_type, CANCELLED, time, output, request_id)` | Order cancelled |
| `TxHashUpdate(uuid, account_id, tx_hash, order_type, status, time, output)` | System-triggered update (no request_id) - liquidations, auto-fills |
| `TraderOrderLiquidation(order, cmd, seq)` | Liquidation event with full order details |

### Sorted Set / Order Book Prices

| SortedSetCommand | Meaning |
|---|---|
| `AddOpenLimitPrice(uuid, price, pos_type)` | New pending limit order at price |
| `RemoveOpenLimitPrice(uuid, pos_type)` | Pending limit order removed (filled or cancelled) |
| `AddCloseLimitPrice(uuid, price, pos_type)` | New close limit at price |
| `RemoveCloseLimitPrice(uuid, pos_type)` | Close limit removed (settled or cancelled) |
| `AddLiquidationPrice(uuid, price, pos_type)` | Position has liquidation price |
| `RemoveLiquidationPrice(uuid, pos_type)` | Position removed (settled/liquidated) |
| `UpdateLiquidationPrice(uuid, price, pos_type)` | Liquidation price changed (after funding) |
| `AddStopLossCloseLIMITPrice(uuid, price, pos_type)` | SL trigger added |
| `AddTakeProfitCloseLIMITPrice(uuid, price, pos_type)` | TP trigger added |
| `RemoveStopLossCloseLIMITPrice(uuid, pos_type)` | SL cancelled or triggered |
| `RemoveTakeProfitCloseLIMITPrice(uuid, pos_type)` | TP cancelled or triggered |
| `BulkSearchRemove*` | Price ticker triggered bulk removal |

### Risk & System State

| Event | Meaning |
|---|---|
| `RiskEngineUpdate(cmd, risk_state)` | Exposure changed. Contains full RiskState snapshot. |
| `RiskParamsUpdate(params)` | Risk parameters changed (max leverage, position limits, etc.) |
| `FundingRateUpdate(rate, price, time)` | New funding rate calculated |
| `CurrentPriceUpdate(price, time)` | BTC price update (every 250ms) |
| `FeeUpdate(cmd, time)` | Fee configuration changed |
| `PoolUpdate(cmd, lend_pool, seq)` | Lending pool state changed |

---

## 14. TraderOrderLimitUpdate - Detailed Explanation

`Event::TraderOrderLimitUpdate(TraderOrder, RpcCommand, usize)` is emitted when a user submits a **close/settle request** (`ExecuteTraderOrder` or `ExecuteTraderOrderSlTp`) but the **close price has not been reached yet**, so the order cannot settle immediately. It records the intent to close at a specific limit price.

### When It Is Emitted

It is emitted in exactly **two places** in `order_handler.rs`:

**1. ExecuteTraderOrder handler** (line ~364):
```
User sends ExecuteTraderOrder with order_type=LIMIT and execution_price
  -> check_for_settlement(execution_price, current_price, LIMIT, None)
  -> Returns (payment, OrderStatus::FILLED)  [NOT SETTLED - price not reached]
  -> set_execution_price_for_limit_order_localdb(execution_price)
     (adds close limit to TRADER_LIMIT_CLOSE_LONG/SHORT sorted set)
  -> stores ZKOS hex string via set_zkos_string_on_limit_update()
  -> Event::TraderOrderLimitUpdate(order, ExecuteTraderOrder cmd, sequence)
  -> Event::TxHash(uuid, account_id, request_id, order_type, PENDING, time, None, request_id)
```

**2. ExecuteTraderOrderSlTp handler** (line ~1086):
```
User sends ExecuteTraderOrderSlTp with SLTP close conditions
  -> check_for_settlement(execution_price, current_price, SLTP, Some(sltp_order))
  -> Returns (payment, OrderStatus::FILLED)  [NOT SETTLED - neither SL nor TP triggered]
  -> set_execution_price_for_limit_order_stoploss_takeprofit_localdb()
     (adds SL/TP to TRADER_SLTP_CLOSE_LONG/SHORT sorted sets)
  -> stores ZKOS hex string via set_zkos_string_on_limit_update()
  -> Event::TraderOrderLimitUpdate(order, ExecuteTraderOrderSlTp cmd, sequence)
  -> Event::TxHash(uuid, account_id, request_id, order_type, PENDING, time, None, request_id)
```

### What It Means

- The order's status is **still FILLED** (position is still open)
- The order now has a **pending close limit** registered in the sorted sets
- The ZKOS hex string (blockchain message for settlement) is **stored for later use** when the price triggers
- The accompanying `TxHash` event with `PENDING` status confirms the close limit is queued

### What Happens Next

The **price ticker** (heartbeat 250ms loop) will eventually detect that the close price has been reached:
```
check_settling_limit_order_on_price_ticker_update_localdb()
  -> Scans TRADER_LIMIT_CLOSE_LONG/SHORT and TRADER_SLTP_CLOSE_LONG/SHORT
  -> Finds order where close_price condition met
  -> RelayerCommand::PriceTickerOrderSettle(order_ids, meta, current_price)
  -> check_for_settlement() with MARKET type (forced settle)
  -> SETTLED -> order_remove_from_localdb()
```

### Snapshot Replay Behavior

During snapshot replay (`handle_trader_order_limit_update` in `snapshot_and_reload.rs:1315`):
- Extracts the `zkos_hex_string` from the embedded RpcCommand
- Stores it in `orderdb_traderorder.zkos_msg[order.uuid]`
- Updates `aggrigate_log_sequence` if the event's sequence is higher
- Handles both `RpcCommand::ExecuteTraderOrder` and `RpcCommand::ExecuteTraderOrderSlTp`

### For Archiver

When you see `TraderOrderLimitUpdate`:
- The order is **still open** (FILLED)
- A **close limit order** has been placed on this position
- The order should appear in the order book as an **open position with a pending close**
- The close will happen automatically when the price reaches the limit
- The `RpcCommand` inside the event tells you whether it's a regular limit close (`ExecuteTraderOrder`) or an SLTP close (`ExecuteTraderOrderSlTp`)

---

## 15. Status Transition Diagrams

### Opening Phase (CreateTraderOrder / CreateTraderOrderSlTp)
```
CreateTraderOrder (cmd order_type=MARKET)
  └──> [FILLED]  (TraderOrder.order_type = MARKET)

CreateTraderOrder (cmd order_type=LIMIT)
  ├── price already crossed ──> [FILLED]  (TraderOrder.order_type = MARKET, converted!)
  └── price not crossed ──> [PENDING]  (TraderOrder.order_type = LIMIT)
        |
        ├── PriceTickerOrderFill ──> [FILLED]
        └── CancelTraderOrder ──> [CANCELLED]
```

### Closing Phase (any FILLED position, regardless of how it was opened)
```
Position is [FILLED] (opened via MARKET or LIMIT - doesn't matter)
  |
  ├── ExecuteTraderOrder (cmd order_type=MARKET)
  |     └──> [SETTLED] immediately
  |
  ├── ExecuteTraderOrder (cmd order_type=LIMIT, execution_price=X)
  |     ├── price condition met ──> [SETTLED] immediately
  |     └── price NOT met ──> registers close limit (Event::TraderOrderLimitUpdate)
  |           ├── PriceTickerOrderSettle ──> [SETTLED]
  |           └── CancelTraderOrder ──> close limit removed (position still [FILLED])
  |
  ├── ExecuteTraderOrderSlTp (cmd order_type=SLTP, sl=X, tp=Y)
  |     ├── SL or TP triggered ──> [SETTLED] immediately
  |     └── neither triggered ──> registers SL/TP limits (Event::TraderOrderLimitUpdate)
  |           ├── PriceTickerOrderSettle ──> [SETTLED]
  |           └── CancelTraderOrderSlTp ──> SL/TP removed (position still [FILLED])
  |                 (CancelledStopLoss / CancelledTakeProfit - can cancel independently)
  |
  ├── PriceTickerLiquidation (price reaches liquidation_price)
  |     └──> [LIQUIDATE]
  |
  └── FundingCycle (margin depleted after funding applied)
        └──> [LIQUIDATE]
```

### Key Insight: Mixed Open/Close Examples
```
Example 1: Open MARKET, Close LIMIT
  CreateTraderOrder(MARKET) -> [FILLED at current price]
  ExecuteTraderOrder(LIMIT, price=X) -> close limit registered
  Price reaches X -> [SETTLED]

Example 2: Open LIMIT, Close MARKET
  CreateTraderOrder(LIMIT, entry=Y) -> [PENDING]
  Price reaches Y -> [FILLED]
  ExecuteTraderOrder(MARKET) -> [SETTLED immediately at current price]

Example 3: Open MARKET, Close SLTP
  CreateTraderOrder(MARKET) -> [FILLED]
  ExecuteTraderOrderSlTp(SLTP, sl=A, tp=B) -> SL/TP limits registered
  Price reaches A or B -> [SETTLED]

Example 4: Open LIMIT, Close SLTP (via CreateTraderOrderSlTp)
  CreateTraderOrderSlTp(LIMIT, entry=Y, sl=A, tp=B) -> [PENDING]
  Price reaches Y -> [FILLED] + SL/TP limits auto-registered
  Price reaches A or B -> [SETTLED]
```

---

## Appendix: Key File Locations

| File | Description |
|---|---|
| `src/relayer/core/order_handler.rs` | RPC command routing and order creation/execution/cancel |
| `src/relayer/traderorder.rs` | TraderOrder struct, lifecycle methods, sorted set management |
| `src/relayer/core/relayer_command_handler.rs` | System-triggered order processing (fills, liquidations, settlements) |
| `src/relayer/heartbeat.rs` | Price ticker, funding cycle, snapshot scheduler |
| `src/relayer/commands.rs` | All command/type enums (RpcCommand, RelayerCommand, SortedSetCommand, etc.) |
| `src/relayer/risk_engine.rs` | Risk validation, RiskState, RiskParams |
| `src/db/events.rs` | Event enum, event logging, Kafka producer, version upcasting |
| `src/db/snapshot_and_reload.rs` | Snapshot serialization, event replay, state reconstruction |
| `src/db/localdb.rs` | Local in-memory database (TRADER_ORDER_DB, sorted sets, etc.) |
| `src/db/sortedset.rs` | SortedSet data structure (price-indexed UUID storage) |
| `src/db/lendpool.rs` | Lending pool state machine |
| `src/relayer/lendorder.rs` | LendOrder struct and methods |
