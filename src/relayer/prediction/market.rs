//! A single market: its metadata, order book, per-account share positions, and
//! the collateral vault backing outstanding complete sets.
//!
//! Cash balances live at the registry level (they are cross-market); share
//! holdings live here because they are market-specific.

use super::book::OrderBook;
use super::types::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Per-account holdings within one market. `*_reserved` shares are committed to
/// resting sell orders and cannot be sold again until those orders fill/cancel.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Position {
    pub yes: f64,
    pub no: f64,
    pub yes_reserved: f64,
    pub no_reserved: f64,
}

impl Position {
    /// Shares of `outcome` available to sell (holdings not already reserved).
    pub fn free(&self, outcome: Outcome) -> f64 {
        match outcome {
            Outcome::Yes => self.yes - self.yes_reserved,
            Outcome::No => self.no - self.no_reserved,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.yes == 0.0 && self.no == 0.0
    }
}

/// A binary market with its book, positions, and collateral vault.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Market {
    pub market_id: Uuid,
    pub question: String,
    /// `1 / tick_size`; valid resting prices are ticks `1..price_scale`.
    pub price_scale: Ticks,
    pub min_qty: f64,
    pub status: MarketStatus,
    pub resolution: Option<Resolution>,
    pub book: OrderBook,
    pub positions: HashMap<String, Position>,
    /// Cash held to back outstanding complete sets. Equals the number of
    /// outstanding sets (in `$`). Mints add to it; merges and payouts drain it.
    pub vault: f64,
}

impl Market {
    pub fn new(market_id: Uuid, question: String, price_scale: Ticks, min_qty: f64) -> Self {
        Market {
            market_id,
            question,
            price_scale,
            min_qty,
            status: MarketStatus::Open,
            resolution: None,
            book: OrderBook::new(),
            positions: HashMap::new(),
            vault: 0.0,
        }
    }

    pub fn position(&self, account: &str) -> Position {
        self.positions.get(account).cloned().unwrap_or_default()
    }

    fn position_mut(&mut self, account: &str) -> &mut Position {
        self.positions.entry(account.to_string()).or_default()
    }

    /// Reserve `qty` shares of `outcome` for a resting sell order.
    pub fn reserve_shares(&mut self, account: &str, outcome: Outcome, qty: f64) {
        let p = self.position_mut(account);
        match outcome {
            Outcome::Yes => p.yes_reserved += qty,
            Outcome::No => p.no_reserved += qty,
        }
    }

    /// Release a share reservation (on cancel / halt) without moving holdings.
    pub fn release_reservation(&mut self, account: &str, outcome: Outcome, qty: f64) {
        let p = self.position_mut(account);
        match outcome {
            Outcome::Yes => p.yes_reserved -= qty,
            Outcome::No => p.no_reserved -= qty,
        }
    }

    /// Credit bought shares to a buyer.
    pub fn credit_shares(&mut self, account: &str, outcome: Outcome, qty: f64) {
        let p = self.position_mut(account);
        match outcome {
            Outcome::Yes => p.yes += qty,
            Outcome::No => p.no += qty,
        }
    }

    /// Deliver sold shares from a seller (reduces holdings and reservation).
    pub fn deliver_shares(&mut self, account: &str, outcome: Outcome, qty: f64) {
        let p = self.position_mut(account);
        match outcome {
            Outcome::Yes => {
                p.yes -= qty;
                p.yes_reserved -= qty;
            }
            Outcome::No => {
                p.no -= qty;
                p.no_reserved -= qty;
            }
        }
    }

    /// Redeem every position at `resolution`, returning `(account, payout)`
    /// pairs. Winning shares pay `$1`, losing `$0`, `Invalid` pays `$0.5` to
    /// both so a complete set is still worth `$1`. Drains the vault.
    pub fn redeem_positions(&mut self, resolution: Resolution) -> Vec<(String, f64)> {
        let mut payouts = Vec::new();
        for (account, pos) in self.positions.iter() {
            let payout = match resolution {
                Resolution::Yes => pos.yes,
                Resolution::No => pos.no,
                Resolution::Invalid => 0.5 * (pos.yes + pos.no),
            };
            if payout != 0.0 {
                payouts.push((account.clone(), payout));
            }
        }
        let total: f64 = payouts.iter().map(|(_, v)| *v).sum();
        self.vault -= total;
        payouts
    }
}
