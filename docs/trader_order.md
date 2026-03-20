# Trader Order

This document explains the `TraderOrder` struct, its fields, the formulas used for position calculations, and the full lifecycle of market, limit, and SL/TP orders.

## TraderOrder Fields

| Field | Type | Unit | Description |
|---|---|---|---|
| `uuid` | Uuid | -- | Unique order identifier (generated on creation) |
| `account_id` | String | -- | Twilight address of the trader |
| `position_type` | PositionType | -- | `LONG` or `SHORT` |
| `order_status` | OrderStatus | -- | Current status (see Order Statuses below) |
| `order_type` | OrderType | -- | `MARKET`, `LIMIT`, or `SLTP` |
| `entryprice` | f64 | sats | Price at which the position was opened. For market orders, set to current price at fill time. |
| `execution_price` | f64 | sats | Requested execution/close price from the RPC request |
| `positionsize` | f64 | sats^2 | Notional position size: `entry_value * entryprice` |
| `leverage` | f64 | -- | Leverage multiplier |
| `initial_margin` | f64 | sats | Collateral deposited by the trader (IM) |
| `available_margin` | f64 | sats | IM minus fees (and funding adjustments). Updated on settlement. |
| `timestamp` | String | -- | UTC timestamp of order creation |
| `bankruptcy_price` | f64 | sats | Price at which the position has zero equity |
| `bankruptcy_value` | f64 | sats | Position size divided by bankruptcy price |
| `maintenance_margin` | f64 | sats | Minimum margin required to keep the position open |
| `liquidation_price` | f64 | sats | Price at which the position gets liquidated |
| `unrealized_pnl` | f64 | sats | Unrealized profit/loss (set on settlement) |
| `settlement_price` | f64 | sats | Price at which the position was closed (0 while open) |
| `entry_nonce` | usize | -- | Pool nonce at time of entry |
| `exit_nonce` | usize | -- | Pool nonce at time of exit |
| `entry_sequence` | usize | -- | Event log sequence number at entry |
| `fee_filled` | f64 | sats | Fee charged on order open |
| `fee_settled` | f64 | sats | Fee charged on order close/settlement |

## Order Statuses

| Status | Meaning |
|---|---|
| `PENDING` | Limit order placed, waiting for price to reach entry price |
| `FILLED` | Position is active (market order filled, or limit order triggered) |
| `SETTLED` | Position is closed and PnL has been settled |
| `CANCELLED` | Limit order cancelled before fill |
| `LimitPriceAdded` | Close limit price registered for a filled position |
| `LimitPriceUpdated` | Close limit price replaced with a new one |
| `StopLossAdded` | Stop loss price registered |
| `StopLossUpdated` | Stop loss price replaced |
| `TakeProfitAdded` | Take profit price registered |
| `TakeProfitUpdated` | Take profit price replaced |
| `CancelledLimitClose` | Close limit was removed (due to position settlement or explicit cancel) |
| `CancelledStopLoss` | Stop loss was removed (due to position settlement or explicit cancel) |
| `CancelledTakeProfit` | Take profit was removed (due to position settlement or explicit cancel) |

## Formulas

All helper functions are defined in `src/relayer/utils.rs`.

### Entry Value

```
entry_value = initial_margin * leverage
```

This is the notional BTC exposure in sats. It is also the value tracked by the risk engine.

### Position Size

```
position_size = entry_value * entry_price
```

Denominated in sats^2 (sats * price-in-sats). Used in PnL and liquidation calculations.

### Position Side

```
LONG  -> -1
SHORT ->  1
```

A sign multiplier used in the liquidation price formula.

### Bankruptcy Price

The price at which the trader's equity reaches zero (100% loss of initial margin).

```
LONG:  bankruptcy_price = entry_price * leverage / (leverage + 1)
SHORT: bankruptcy_price = entry_price * leverage / (leverage - 1)    (if leverage > 1, else 0)
```

### Bankruptcy Value

```
bankruptcy_value = position_size / bankruptcy_price
```

### Maintenance Margin

The minimum margin the trader must maintain. Incorporates the configurable `mm_ratio` from risk params, fees, and funding.

```
maintenance_margin = (mm_ratio * entry_value + fee% * bankruptcy_value + funding_rate * bankruptcy_value) / 100
```

Where `mm_ratio` defaults to 0.4 (40%) and is configurable via `RISK_MM_RATIO`.

### Liquidation Price

The price at which the position's remaining margin equals the maintenance margin.

```
liquidation_price = entry_price * position_size / (position_side * entry_price * (mm - im) + position_size)
```

Where `position_side` is -1 for LONG, +1 for SHORT, `mm` is maintenance margin, and `im` is initial margin.

### Unrealized PnL

Profit or loss at a given settlement/current price, using an inverse contract formula:

```
LONG:  unrealized_pnl = position_size * (settle_price - entry_price) / (entry_price * settle_price)
SHORT: unrealized_pnl = position_size * (entry_price - settle_price) / (entry_price * settle_price)
```

Positive value = profit, negative = loss.

### Fee Calculation

```
fee = max(1, (fee_percentage / 100) * position_size / price)
```

Minimum fee is 1 sat. Separate fee percentages exist for market orders (`FilledOnMarket`) and limit orders (`FilledOnLimit`).

## Order Lifecycle

### Market Order Open

1. `new_order` is called with the `CreateTraderOrder` RPC request
2. Entry price is set to current market price
3. Order status is set to `FILLED` immediately
4. All position fields are calculated (entry value, position size, bankruptcy price, liquidation price, etc.)
5. Fee is deducted from available margin: `available_margin = initial_margin - fee`
6. `orderinsert_localdb` registers the order:
   - Risk engine: `RiskState::add_order(position_type, entry_value)` -- adds to filled exposure
   - Position size log: tracks total position size per side
   - Liquidation list: adds to sorted set keyed by `liquidation_price * 10000` for liquidation scanning

### Limit Order Open

1. `new_order` is called; if the limit price would fill immediately (LONG entry >= current price, or SHORT entry <= current price), it is converted to a market order
2. Otherwise, order status is set to `PENDING`, fee is set to 0 (deferred to fill)
3. `orderinsert_localdb` registers the order:
   - Risk engine: `RiskState::add_pending_order(position_type, entry_value)` -- adds to pending exposure
   - Open order list: adds to sorted set keyed by `entry_price * 10000` for price-triggered matching

### Limit Order Fill (`pending_order`)

When the market price reaches the limit entry price:

1. `pending_order` is called with the trigger price as the new entry price
2. All position fields are recalculated at the actual fill price
3. Order status changes to `FILLED`
4. Fee is now calculated and deducted from available margin
5. `orderinsert_localdb` handles the risk engine transition:
   - `RiskState::remove_pending_order(...)` -- removes from pending exposure
   - `RiskState::add_order(...)` -- adds to filled exposure
   - Moves from open order list to liquidation list

### Close/Settlement

#### Market Close

1. `check_for_settlement` is called with `OrderType::MARKET`
2. `calculatepayment_localdb` computes the settlement:
   - `margin_difference = available_margin - initial_margin` (captures prior fee/funding effects)
   - `unrealized_pnl` is calculated at the current price
   - Close fee is calculated
   - `payment = round(unrealized_pnl) + round(margin_difference) - close_fee`
   - `available_margin = round(initial_margin + payment)`
   - Order status set to `SETTLED`
3. `order_remove_from_localdb` cleans up:
   - Risk engine: `RiskState::remove_order(position_type, entry_value)`
   - Position size log: decrements
   - Removes from liquidation list
   - Removes any associated close limit, stop loss, and take profit orders

#### Limit Close

1. `check_for_settlement` is called with `OrderType::LIMIT` and an `execution_price`
2. If the limit close price would fill immediately (LONG: execution_price <= current_price, SHORT: execution_price >= current_price):
   - Settles at current price (same as market close)
3. Otherwise:
   - `set_execution_price_for_limit_order_localdb` registers the close price in the sorted set
   - If a close limit already exists for this order, it is replaced (updated)
   - The order remains `FILLED` until the price triggers settlement

#### SL/TP Close

1. `check_for_settlement` is called with `OrderType::SLTP` and a `SlTpOrder` containing `sl` and/or `tp` prices
2. **Stop Loss** trigger conditions:
   - LONG: `sl >= current_price` (price fell to or below stop loss) -> settles immediately
   - SHORT: `sl <= current_price` (price rose to or above stop loss) -> settles immediately
   - Otherwise: registers in the SL sorted set for future matching
3. **Take Profit** trigger conditions:
   - LONG: `tp <= current_price` (price rose to or above take profit) -> settles immediately
   - SHORT: `tp >= current_price` (price fell to or below take profit) -> settles immediately
   - Otherwise: registers in the TP sorted set for future matching
4. Both SL and TP are evaluated independently -- both can be set on the same order

### Cancel

#### Cancel Open Limit Order (`cancelorder_localdb`)

Only `LIMIT` orders in `PENDING` status can be cancelled.

1. Removes the order from the open order sorted set (`TRADER_LIMIT_OPEN_LONG/SHORT`)
2. Risk engine: `RiskState::remove_pending_order(position_type, entry_value)`
3. Order status set to `CANCELLED`

#### Cancel Close Limit Order (`cancel_close_limit_order`)

Removes a pending close limit price from a filled position.

1. Removes from `TRADER_LIMIT_CLOSE_LONG/SHORT` sorted set
2. Position remains `FILLED` (only the close trigger is removed)

#### Cancel SL/TP Order (`cancel_sltp_order`)

Removes stop loss and/or take profit from a filled position.

1. Based on the `SlTpOrderCancel` flags, removes from the appropriate sorted sets:
   - SL: `TRADER_SL_CLOSE_LONG/SHORT`
   - TP: `TRADER_TP_CLOSE_LONG/SHORT`
2. Position remains `FILLED`

## Sorted Set Databases

Prices are stored in sorted sets as integers (`price * 10000` cast to `i64`) for efficient range scanning. The sorted sets used:

| Sorted Set | Purpose | Key | Score |
|---|---|---|---|
| `TRADER_LP_LONG` | Long liquidation prices | order UUID | `liquidation_price * 10000` |
| `TRADER_LP_SHORT` | Short liquidation prices | order UUID | `liquidation_price * 10000` |
| `TRADER_LIMIT_OPEN_LONG` | Pending long limit open orders | order UUID | `entry_price * 10000` |
| `TRADER_LIMIT_OPEN_SHORT` | Pending short limit open orders | order UUID | `entry_price * 10000` |
| `TRADER_LIMIT_CLOSE_LONG` | Long close limit prices | order UUID | `execution_price * 10000` |
| `TRADER_LIMIT_CLOSE_SHORT` | Short close limit prices | order UUID | `execution_price * 10000` |
| `TRADER_SL_CLOSE_LONG` | Short stop loss prices (triggers long-side scan) | order UUID | `sl_price * 10000` |
| `TRADER_SL_CLOSE_SHORT` | Long stop loss prices (triggers short-side scan) | order UUID | `sl_price * 10000` |
| `TRADER_TP_CLOSE_LONG` | Long take profit prices | order UUID | `tp_price * 10000` |
| `TRADER_TP_CLOSE_SHORT` | Short take profit prices | order UUID | `tp_price * 10000` |

**Note on SL direction mapping:** A LONG position's stop loss is stored in `TRADER_SL_CLOSE_SHORT` (because it triggers when price falls -- same direction as short-side scanning), and a SHORT position's stop loss is stored in `TRADER_SL_CLOSE_LONG` (triggers when price rises).

## Risk Engine Integration

| Event | Risk Engine Call |
|---|---|
| Market order filled | `add_order(position_type, entry_value)` |
| Limit order placed (pending) | `add_pending_order(position_type, entry_value)` |
| Limit order triggered (fill) | `remove_pending_order(...)` then `add_order(...)` |
| Position closed/settled | `remove_order(position_type, entry_value)` |
| Limit order cancelled | `remove_pending_order(position_type, entry_value)` |

## Settlement Payment Flow

The payment returned from settlement flows to/from the lending pool:

```
payment > 0 -> trader profits, pool pays the trader (pool TLV decreases)
payment < 0 -> trader loses, pool receives from the trader (pool TLV increases)
payment = 0 -> breakeven (only fees are deducted)
```

The payment value is:

```
payment = round(unrealized_pnl) + round(margin_difference) - close_fee
```

Where `margin_difference = available_margin - initial_margin` captures any prior funding payments that increased or decreased the trader's available margin.

## Cleanup on Settlement

When a position is settled (closed), `order_remove_from_localdb` automatically cleans up all associated state:

1. Removes from position size tracking
2. Removes from risk engine exposure
3. Removes from liquidation price sorted set
4. Removes any close limit order (emits `CancelledLimitClose`)
5. Removes any stop loss order (emits `CancelledStopLoss`)
6. Removes any take profit order (emits `CancelledTakeProfit`)

Each cleanup emits the appropriate `SortedSetDBUpdate` and `TxHash` events for event sourcing.
