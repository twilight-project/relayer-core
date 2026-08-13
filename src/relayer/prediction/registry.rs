//! The market registry: the orchestration layer that ties the order book,
//! per-market positions, and cross-market cash together, and emits the domain
//! events that later phases will publish to the event log.
//!
//! Cash conservation invariant: for every account, `free + locked` only changes
//! via `deposit` and resolution payouts. Within trading, cash and share value
//! move between accounts and the per-market vault but are never created or
//! destroyed. The `conservation` tests assert this.

use super::book::{Fill, FillKind};
use super::market::{Market, Position};
use super::types::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A cross-market cash balance. `locked` backs resting buy orders.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Account {
    pub free: f64,
    pub locked: f64,
}

impl Account {
    pub fn total(&self) -> f64 {
        self.free + self.locked
    }
}

/// Why an order or lifecycle action was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    MarketNotFound,
    MarketNotOpen,
    BadPrice,
    BadQuantity,
    InsufficientCash,
    InsufficientShares,
    OrderNotFound,
    MarketNotResolvable,
}

/// A domain event produced by a state transition. Phase 2 wires these to the
/// Kafka event log; for now they make transitions observable and testable.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainEvent {
    MarketCreated { market_id: Uuid, question: String },
    OrderAccepted { order_id: Uuid, market_id: Uuid, account: String, status: OrderStatus },
    Trade(Fill),
    OrderCancelled { order_id: Uuid, market_id: Uuid },
    MarketHalted { market_id: Uuid },
    MarketResolved { market_id: Uuid, resolution: Resolution },
    Payout { market_id: Uuid, account: String, amount: f64 },
}

/// Outcome of admitting an order.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceReport {
    pub order_id: Uuid,
    pub status: OrderStatus,
    pub filled_qty: f64,
    pub resting_qty: f64,
    pub fills: Vec<Fill>,
    pub events: Vec<DomainEvent>,
}

#[derive(Default)]
pub struct MarketRegistry {
    markets: HashMap<Uuid, Market>,
    accounts: HashMap<String, Account>,
}

impl MarketRegistry {
    pub fn new() -> Self {
        MarketRegistry::default()
    }

    // ---- funding / inspection -------------------------------------------

    /// Credit an account with cash (deposit / test funding).
    pub fn deposit(&mut self, account: &str, amount: f64) {
        self.accounts.entry(account.to_string()).or_default().free += amount;
    }

    pub fn account(&self, account: &str) -> Account {
        self.accounts.get(account).cloned().unwrap_or_default()
    }

    pub fn market(&self, market_id: &Uuid) -> Option<&Market> {
        self.markets.get(market_id)
    }

    pub fn position(&self, market_id: &Uuid, account: &str) -> Position {
        self.markets
            .get(market_id)
            .map(|m| m.position(account))
            .unwrap_or_default()
    }

    // ---- lifecycle ------------------------------------------------------

    /// Create a new open market. `price_scale = 1/tick_size` (e.g. 100).
    pub fn create_market(&mut self, question: &str, price_scale: Ticks, min_qty: f64) -> (Uuid, DomainEvent) {
        let market_id = Uuid::new_v4();
        self.markets.insert(
            market_id,
            Market::new(market_id, question.to_string(), price_scale, min_qty),
        );
        (
            market_id,
            DomainEvent::MarketCreated { market_id, question: question.to_string() },
        )
    }

    /// Halt trading and cancel every resting order, releasing their collateral.
    pub fn halt_market(&mut self, market_id: &Uuid) -> Result<Vec<DomainEvent>, RejectReason> {
        let market = self.markets.get_mut(market_id).ok_or(RejectReason::MarketNotFound)?;
        if market.status == MarketStatus::Resolved {
            return Err(RejectReason::MarketNotOpen);
        }
        let scale = market.price_scale;
        let resting = market.book.drain_all();
        market.status = MarketStatus::Halted;
        let mut events = vec![DomainEvent::MarketHalted { market_id: *market_id }];
        for o in resting {
            Self::release_resting(self.accounts.entry(o.owner.clone()).or_default(), market, &o, scale);
            events.push(DomainEvent::OrderCancelled { order_id: o.order_id, market_id: *market_id });
        }
        Ok(events)
    }

    /// Resolve a market: cancel resting orders, then redeem all positions.
    pub fn resolve_market(
        &mut self,
        market_id: &Uuid,
        resolution: Resolution,
    ) -> Result<Vec<DomainEvent>, RejectReason> {
        {
            let market = self.markets.get(market_id).ok_or(RejectReason::MarketNotFound)?;
            if market.status == MarketStatus::Resolved {
                return Err(RejectReason::MarketNotResolvable);
            }
        }
        // Cancel resting orders first (release locked cash / reservations).
        let mut events = Vec::new();
        {
            let market = self.markets.get_mut(market_id).unwrap();
            let scale = market.price_scale;
            for o in market.book.drain_all() {
                Self::release_resting(self.accounts.entry(o.owner.clone()).or_default(), market, &o, scale);
                events.push(DomainEvent::OrderCancelled { order_id: o.order_id, market_id: *market_id });
            }
        }
        let market = self.markets.get_mut(market_id).unwrap();
        market.status = MarketStatus::Resolved;
        market.resolution = Some(resolution);
        events.push(DomainEvent::MarketResolved { market_id: *market_id, resolution });
        for (account, amount) in market.redeem_positions(resolution) {
            self.accounts.entry(account.clone()).or_default().free += amount;
            events.push(DomainEvent::Payout { market_id: *market_id, account, amount });
        }
        Ok(events)
    }

    // ---- order entry ----------------------------------------------------

    /// Place a limit or market order. Buys lock cash; sells reserve shares.
    #[allow(clippy::too_many_arguments)]
    pub fn place_order(
        &mut self,
        market_id: &Uuid,
        account: &str,
        side: Side,
        outcome: Outcome,
        price_ticks: Ticks,
        quantity: f64,
        order_type: OrderType,
    ) -> Result<PlaceReport, RejectReason> {
        let market = self.markets.get_mut(market_id).ok_or(RejectReason::MarketNotFound)?;
        if market.status != MarketStatus::Open {
            return Err(RejectReason::MarketNotOpen);
        }
        let scale = market.price_scale;
        if quantity <= 0.0 || quantity < market.min_qty {
            return Err(RejectReason::BadQuantity);
        }
        if order_type == OrderType::Limit && (price_ticks == 0 || price_ticks >= scale) {
            return Err(RejectReason::BadPrice);
        }

        let intent = Intent::from_parts(side, outcome);
        // Own-space limit price (probability the user quoted for their outcome).
        let own_price_abs = match order_type {
            OrderType::Limit => price_ticks as f64 / scale as f64,
            OrderType::Market if intent.is_buy() => 1.0, // willing to pay up to $1
            OrderType::Market => 0.0,                    // willing to sell down to $0
        };

        // Reserve collateral before touching the book.
        let acct = self.accounts.entry(account.to_string()).or_default();
        if intent.is_buy() {
            let cost = own_price_abs * quantity;
            if acct.free + 1e-9 < cost {
                return Err(RejectReason::InsufficientCash);
            }
            acct.free -= cost;
            acct.locked += cost;
        } else {
            if market.position(account).free(outcome) + 1e-9 < quantity {
                return Err(RejectReason::InsufficientShares);
            }
            market.reserve_shares(account, outcome, quantity);
        }

        // Crossing bound + resting price in YES space.
        let (cross_ticks, rest) = match order_type {
            OrderType::Limit => (intent.to_yes_ticks(price_ticks, scale), true),
            OrderType::Market => (if intent.book_side() == BookSide::Bid { scale } else { 0 }, false),
        };

        let order_id = Uuid::new_v4();
        let result = market
            .book
            .admit(order_id, account, intent, cross_ticks, quantity, rest);

        // Apply each fill to positions, cash, and the vault.
        for fill in &result.fills {
            self.apply_fill(market_id, fill, own_price_abs);
        }

        let status = if result.resting_qty > 0.0 {
            if result.filled_qty > 0.0 {
                OrderStatus::PartiallyFilled
            } else {
                OrderStatus::Open
            }
        } else if result.filled_qty >= quantity - 1e-9 {
            OrderStatus::Filled
        } else if result.filled_qty > 0.0 {
            OrderStatus::PartiallyFilled // market order, partial then dropped
        } else {
            OrderStatus::Cancelled // market order, nothing available
        };

        // A rejected/cancelled market sell with nothing filled must release its
        // share reservation; a cancelled market buy releases nothing (cost was
        // only locked for filled portion + none rested). Handle unfilled
        // market-order remainder cleanup.
        if !rest {
            let unfilled = quantity - result.filled_qty;
            if unfilled > 0.0 {
                if intent.is_buy() {
                    // Release the lock on the unfilled portion.
                    let acct = self.accounts.entry(account.to_string()).or_default();
                    let release = own_price_abs * unfilled;
                    acct.locked -= release;
                    acct.free += release;
                } else {
                    let market = self.markets.get_mut(market_id).unwrap();
                    market.release_reservation(account, outcome, unfilled);
                }
            }
        }

        let mut events = vec![DomainEvent::OrderAccepted {
            order_id,
            market_id: *market_id,
            account: account.to_string(),
            status,
        }];
        events.extend(result.fills.iter().cloned().map(DomainEvent::Trade));

        Ok(PlaceReport {
            order_id,
            status,
            filled_qty: result.filled_qty,
            resting_qty: result.resting_qty,
            fills: result.fills,
            events,
        })
    }

    /// Cancel a resting order, releasing its remaining collateral.
    pub fn cancel_order(
        &mut self,
        market_id: &Uuid,
        order_id: Uuid,
    ) -> Result<Vec<DomainEvent>, RejectReason> {
        let market = self.markets.get_mut(market_id).ok_or(RejectReason::MarketNotFound)?;
        let scale = market.price_scale;
        let order = market.book.cancel(order_id).ok_or(RejectReason::OrderNotFound)?;
        Self::release_resting(self.accounts.entry(order.owner.clone()).or_default(), market, &order, scale);
        Ok(vec![DomainEvent::OrderCancelled { order_id, market_id: *market_id }])
    }

    // ---- internals ------------------------------------------------------

    /// Apply a single fill's cash/share/vault effects. `taker_own_price` is the
    /// taker's submitted limit price (its own outcome space); the maker always
    /// fills at its own resting price (no price improvement).
    fn apply_fill(&mut self, market_id: &Uuid, fill: &Fill, taker_own_price: f64) {
        let scale = self.markets.get(market_id).unwrap().price_scale;
        let qty = fill.quantity;

        // Execution price for a leg, in that leg's own outcome space.
        let exec_own = |intent: Intent| -> f64 {
            match intent.outcome() {
                Outcome::Yes => fill.yes_ticks as f64 / scale as f64,
                Outcome::No => (scale - fill.yes_ticks) as f64 / scale as f64,
            }
        };

        // Maker leg: own price == execution price.
        let maker_exec = exec_own(fill.maker_intent);
        self.apply_leg(market_id, &fill.maker_owner, fill.maker_intent, maker_exec, maker_exec, qty);
        // Taker leg: fills at maker price, may have quoted a better own price.
        let taker_exec = exec_own(fill.taker_intent);
        self.apply_leg(market_id, &fill.taker_owner, fill.taker_intent, taker_own_price, taker_exec, qty);

        // Vault: minting a set locks $1/share; merging releases it.
        let market = self.markets.get_mut(market_id).unwrap();
        match fill.kind {
            FillKind::MintSet => market.vault += qty,
            FillKind::MergeSet => market.vault -= qty,
            FillKind::Transfer => {}
        }
    }

    fn apply_leg(
        &mut self,
        market_id: &Uuid,
        account: &str,
        intent: Intent,
        own_price: f64,
        exec_price: f64,
        qty: f64,
    ) {
        let market = self.markets.get_mut(market_id).unwrap();
        if intent.is_buy() {
            let spend = exec_price * qty;
            let refund = (own_price - exec_price) * qty; // >= 0
            market.credit_shares(account, intent.outcome(), qty);
            let acct = self.accounts.entry(account.to_string()).or_default();
            acct.locked -= spend + refund;
            acct.free += refund;
        } else {
            market.deliver_shares(account, intent.outcome(), qty);
            self.accounts.entry(account.to_string()).or_default().free += exec_price * qty;
        }
    }

    /// Release the collateral behind a resting order that is being removed.
    fn release_resting(acct: &mut Account, market: &mut Market, order: &super::book::BookOrder, scale: Ticks) {
        if order.intent.is_buy() {
            let own_price = match order.intent.outcome() {
                Outcome::Yes => order.yes_ticks as f64 / scale as f64,
                Outcome::No => (scale - order.yes_ticks) as f64 / scale as f64,
            };
            let release = own_price * order.remaining;
            acct.locked -= release;
            acct.free += release;
        } else {
            market.release_reservation(&order.owner, order.intent.outcome(), order.remaining);
        }
    }
}
