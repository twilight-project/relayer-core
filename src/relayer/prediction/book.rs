//! The unified YES order book and its price-time-priority matching engine.
//!
//! All orders — YES and NO, buy and sell — are reflected into a single book in
//! YES-price space (see [`super::types::Intent`]). Bids are sorted
//! high-to-low, asks low-to-high; a taker crosses the opposite side, filling at
//! the resting **maker** price under strict price-then-time priority.
//!
//! The book is pure: it knows order ids, owners, prices and quantities, and
//! nothing about cash, collateral, or positions. Settlement effects are derived
//! from each [`Fill`] by the ledger layer.

use super::types::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

/// An order resting on (or crossing) the unified YES book.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BookOrder {
    pub order_id: Uuid,
    pub owner: String,
    /// The user's original intent (retained so fills can be classified and
    /// positions updated in the right outcome space).
    pub intent: Intent,
    /// Price on the unified YES book, in ticks.
    pub yes_ticks: Ticks,
    /// Quantity still open (shares).
    pub remaining: f64,
    /// Monotonic sequence number establishing time priority.
    pub seq: u64,
}

impl BookOrder {
    pub fn book_side(&self) -> BookSide {
        self.intent.book_side()
    }
}

/// How a single fill reshapes the two counterparties' inventories.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillKind {
    /// A YES buyer and a NO buyer each fund part of `$1`; the protocol mints a
    /// fresh complete set. Increases open interest.
    MintSet,
    /// A YES seller and a NO seller surrender a complete set, which is merged
    /// back into `$1`. Decreases open interest.
    MergeSet,
    /// Existing shares change hands (YES↔YES or NO↔NO). Open interest unchanged.
    Transfer,
}

/// One matched execution between a resting maker and an incoming taker.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Fill {
    pub maker_order: Uuid,
    pub taker_order: Uuid,
    pub maker_owner: String,
    pub taker_owner: String,
    pub maker_intent: Intent,
    pub taker_intent: Intent,
    /// Execution price on the YES book (the maker's resting price).
    pub yes_ticks: Ticks,
    pub quantity: f64,
    pub kind: FillKind,
}

impl Fill {
    /// Classify a fill from the two intents. A fill always pairs one bid-intent
    /// (`BuyYes`/`SellNo`) with one ask-intent (`SellYes`/`BuyNo`).
    fn classify(bid_intent: Intent, ask_intent: Intent) -> FillKind {
        match (bid_intent, ask_intent) {
            (Intent::BuyYes, Intent::BuyNo) => FillKind::MintSet,
            (Intent::SellNo, Intent::SellYes) => FillKind::MergeSet,
            _ => FillKind::Transfer,
        }
    }
}

/// Result of admitting a taker order to the book.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    pub fills: Vec<Fill>,
    /// Quantity that filled immediately.
    pub filled_qty: f64,
    /// Quantity left resting on the book (0 for market orders).
    pub resting_qty: f64,
}

/// A single market's order book: two price ladders in YES space.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct OrderBook {
    /// Bid ladder, keyed by YES-tick. Consumed best (highest) price first.
    bids: BTreeMap<Ticks, VecDeque<BookOrder>>,
    /// Ask ladder, keyed by YES-tick. Consumed best (lowest) price first.
    asks: BTreeMap<Ticks, VecDeque<BookOrder>>,
    next_seq: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook::default()
    }

    /// Best (highest) bid tick, if any.
    pub fn best_bid(&self) -> Option<Ticks> {
        self.bids.keys().next_back().copied()
    }

    /// Best (lowest) ask tick, if any.
    pub fn best_ask(&self) -> Option<Ticks> {
        self.asks.keys().next().copied()
    }

    /// Total resting quantity across both ladders (for tests / stats).
    pub fn total_resting(&self) -> f64 {
        let sum = |m: &BTreeMap<Ticks, VecDeque<BookOrder>>| {
            m.values()
                .flat_map(|q| q.iter())
                .map(|o| o.remaining)
                .sum::<f64>()
        };
        sum(&self.bids) + sum(&self.asks)
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq += 1;
        s
    }

    /// Admit a taker order, crossing the opposite ladder, then (for limit
    /// orders) resting any remainder. Self-trades are prevented by skipping
    /// resting orders from the same owner.
    ///
    /// `limit_ticks` is the taker's worst acceptable YES price; for a market
    /// order pass the extreme (`scale` for a bid, `0` for an ask) and
    /// `rest = false`.
    pub fn admit(
        &mut self,
        order_id: Uuid,
        owner: &str,
        intent: Intent,
        limit_ticks: Ticks,
        quantity: f64,
        rest: bool,
    ) -> MatchResult {
        let side = intent.book_side();
        let mut remaining = quantity;
        let mut fills = Vec::new();

        // Cross the opposite ladder while price permits.
        loop {
            if remaining <= 0.0 {
                break;
            }
            let maker_ticks = match side {
                // A bid lifts asks priced at or below its limit, cheapest first.
                BookSide::Bid => match self.best_ask() {
                    Some(a) if a <= limit_ticks => a,
                    _ => break,
                },
                // An ask hits bids priced at or above its limit, richest first.
                BookSide::Ask => match self.best_bid() {
                    Some(b) if b >= limit_ticks => b,
                    _ => break,
                },
            };

            let opposite = match side {
                BookSide::Bid => &mut self.asks,
                BookSide::Ask => &mut self.bids,
            };
            let level = opposite.get_mut(&maker_ticks).expect("level exists");

            let mut level_emptied = false;
            while remaining > 0.0 {
                let maker = match level.front_mut() {
                    Some(m) => m,
                    None => {
                        level_emptied = true;
                        break;
                    }
                };
                // Self-trade prevention: skip the maker without filling it.
                // Rotate it to the back so we can reach makers behind it.
                if maker.owner == owner {
                    // If every remaining maker at this level is self-owned we
                    // must not loop forever: detect a full rotation.
                    if level.iter().all(|m| m.owner == owner) {
                        break;
                    }
                    let skipped = level.pop_front().unwrap();
                    level.push_back(skipped);
                    continue;
                }

                let traded = remaining.min(maker.remaining);
                let (bid_intent, ask_intent) = match side {
                    BookSide::Bid => (intent, maker.intent),
                    BookSide::Ask => (maker.intent, intent),
                };
                fills.push(Fill {
                    maker_order: maker.order_id,
                    taker_order: order_id,
                    maker_owner: maker.owner.clone(),
                    taker_owner: owner.to_string(),
                    maker_intent: maker.intent,
                    taker_intent: intent,
                    yes_ticks: maker_ticks,
                    quantity: traded,
                    kind: Fill::classify(bid_intent, ask_intent),
                });
                maker.remaining -= traded;
                remaining -= traded;
                if maker.remaining <= 0.0 {
                    level.pop_front();
                }
            }

            if level_emptied || level.is_empty() {
                opposite.remove(&maker_ticks);
            } else {
                // Remaining depth at this level is entirely self-owned; stop.
                if level.iter().all(|m| m.owner == owner) {
                    break;
                }
            }
        }

        let filled_qty = quantity - remaining;
        let mut resting_qty = 0.0;
        if rest && remaining > 0.0 {
            let seq = self.next_seq();
            let ladder = match side {
                BookSide::Bid => &mut self.bids,
                BookSide::Ask => &mut self.asks,
            };
            ladder
                .entry(limit_ticks)
                .or_default()
                .push_back(BookOrder {
                    order_id,
                    owner: owner.to_string(),
                    intent,
                    yes_ticks: limit_ticks,
                    remaining,
                    seq,
                });
            resting_qty = remaining;
        }

        MatchResult {
            fills,
            filled_qty,
            resting_qty,
        }
    }

    /// Remove a resting order by id. Returns its unfilled remainder if found.
    pub fn cancel(&mut self, order_id: Uuid) -> Option<BookOrder> {
        for ladder in [&mut self.bids, &mut self.asks] {
            let mut found_key = None;
            for (tick, level) in ladder.iter_mut() {
                if let Some(pos) = level.iter().position(|o| o.order_id == order_id) {
                    let order = level.remove(pos).unwrap();
                    if level.is_empty() {
                        found_key = Some(*tick);
                    }
                    if let Some(k) = found_key {
                        ladder.remove(&k);
                    }
                    return Some(order);
                }
            }
        }
        None
    }

    /// Drain every resting order (used on market halt/resolution).
    pub fn drain_all(&mut self) -> Vec<BookOrder> {
        let mut out = Vec::new();
        for ladder in [&mut self.bids, &mut self.asks] {
            for (_, level) in std::mem::take(ladder) {
                out.extend(level);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn yes_bid_lifts_yes_ask_transfer_at_maker_price() {
        let mut b = OrderBook::new();
        // Maker: someone sells YES @ 0.60.
        let maker = id();
        b.admit(maker, "alice", Intent::SellYes, 60, 10.0, true);
        // Taker: buy YES @ 0.65 -> crosses, fills at maker's 0.60.
        let taker = id();
        let r = b.admit(taker, "bob", Intent::BuyYes, 65, 10.0, true);
        assert_eq!(r.filled_qty, 10.0);
        assert_eq!(r.resting_qty, 0.0);
        assert_eq!(r.fills.len(), 1);
        assert_eq!(r.fills[0].yes_ticks, 60);
        assert_eq!(r.fills[0].kind, FillKind::Transfer);
        assert_eq!(b.total_resting(), 0.0);
    }

    #[test]
    fn yes_buy_and_no_buy_mint_a_set() {
        // `admit` takes YES-space ticks; the registry reflects NO prices before
        // calling it, so here a "Buy NO @ 0.40" is passed as its YES ask @ 0.60.
        let mut b = OrderBook::new();
        b.admit(id(), "alice", Intent::BuyNo, 60, 5.0, true);
        // Buy YES @ 0.60 -> YES bid @ 0.60, crosses the ask -> MintSet.
        let r = b.admit(id(), "bob", Intent::BuyYes, 60, 5.0, true);
        assert_eq!(r.fills.len(), 1);
        assert_eq!(r.fills[0].kind, FillKind::MintSet);
        assert_eq!(r.fills[0].yes_ticks, 60);
        assert_eq!(r.fills[0].quantity, 5.0);
    }

    #[test]
    fn no_price_orders_cross_via_reflection() {
        // NO prices arrive already reflected into YES space: 0.30 NO -> 0.70 YES.
        let mut b = OrderBook::new();
        b.admit(id(), "alice", Intent::BuyNo, 70, 8.0, true);
        // Sell NO @ 0.30 -> YES bid @ 0.70. Crosses -> NO transfers.
        let r = b.admit(id(), "bob", Intent::SellNo, 70, 8.0, true);
        assert_eq!(r.filled_qty, 8.0);
        assert_eq!(r.fills[0].kind, FillKind::Transfer);
        assert_eq!(r.fills[0].yes_ticks, 70);
    }

    #[test]
    fn sell_yes_and_sell_no_merge_a_set() {
        let mut b = OrderBook::new();
        // Sell NO @ 0.40 -> YES bid @ 0.60 (already reflected).
        b.admit(id(), "alice", Intent::SellNo, 60, 3.0, true);
        // Sell YES @ 0.60 -> YES ask @ 0.60. Crosses the bid -> MergeSet.
        let r = b.admit(id(), "bob", Intent::SellYes, 60, 3.0, true);
        assert_eq!(r.fills.len(), 1);
        assert_eq!(r.fills[0].kind, FillKind::MergeSet);
    }

    #[test]
    fn no_cross_when_spread_is_open() {
        let mut b = OrderBook::new();
        b.admit(id(), "alice", Intent::SellYes, 70, 10.0, true); // ask 0.70
        let r = b.admit(id(), "bob", Intent::BuyYes, 60, 10.0, true); // bid 0.60
        assert_eq!(r.filled_qty, 0.0);
        assert_eq!(r.resting_qty, 10.0);
        assert_eq!(b.best_bid(), Some(60));
        assert_eq!(b.best_ask(), Some(70));
    }

    #[test]
    fn partial_fill_rests_remainder() {
        let mut b = OrderBook::new();
        b.admit(id(), "alice", Intent::SellYes, 50, 4.0, true);
        let r = b.admit(id(), "bob", Intent::BuyYes, 50, 10.0, true);
        assert_eq!(r.filled_qty, 4.0);
        assert_eq!(r.resting_qty, 6.0);
        assert_eq!(b.best_bid(), Some(50));
    }

    #[test]
    fn price_time_priority_best_price_then_fifo() {
        let mut b = OrderBook::new();
        // Two asks at 0.55 (FIFO) and one better at 0.50.
        let a_cheap = id();
        b.admit(a_cheap, "alice", Intent::SellYes, 50, 2.0, true);
        let a_first = id();
        b.admit(a_first, "carol", Intent::SellYes, 55, 2.0, true);
        let a_second = id();
        b.admit(a_second, "dave", Intent::SellYes, 55, 2.0, true);
        // Taker sweeps 5 units: 0.50 first, then 0.55 FIFO.
        let r = b.admit(id(), "bob", Intent::BuyYes, 55, 5.0, true);
        assert_eq!(r.fills.len(), 3);
        assert_eq!(r.fills[0].maker_order, a_cheap);
        assert_eq!(r.fills[0].yes_ticks, 50);
        assert_eq!(r.fills[1].maker_order, a_first);
        assert_eq!(r.fills[2].maker_order, a_second);
        assert_eq!(r.fills[2].quantity, 1.0); // only 1 of dave's 2 filled
    }

    #[test]
    fn self_trade_is_prevented() {
        let mut b = OrderBook::new();
        // Alice rests an ask, then Alice tries to lift it.
        b.admit(id(), "alice", Intent::SellYes, 50, 5.0, true);
        let r = b.admit(id(), "alice", Intent::BuyYes, 55, 5.0, true);
        assert_eq!(r.filled_qty, 0.0); // did not self-trade
        assert_eq!(r.resting_qty, 5.0); // rested instead
        // Both alice orders now on the book (one ask, one bid).
        assert_eq!(b.best_ask(), Some(50));
        assert_eq!(b.best_bid(), Some(55));
    }

    #[test]
    fn self_owned_maker_skipped_to_reach_others() {
        let mut b = OrderBook::new();
        // Alice's ask is first in time at 0.50, carol's behind it.
        b.admit(id(), "alice", Intent::SellYes, 50, 5.0, true);
        let carol = id();
        b.admit(carol, "carol", Intent::SellYes, 50, 5.0, true);
        // Alice buys: skips her own resting ask, fills carol's.
        let r = b.admit(id(), "alice", Intent::BuyYes, 50, 5.0, false);
        assert_eq!(r.filled_qty, 5.0);
        assert_eq!(r.fills.len(), 1);
        assert_eq!(r.fills[0].maker_order, carol);
    }

    #[test]
    fn market_order_does_not_rest_remainder() {
        let mut b = OrderBook::new();
        b.admit(id(), "alice", Intent::SellYes, 50, 3.0, true);
        // Market buy for 10 at the extreme; only 3 available, 7 dropped.
        let r = b.admit(id(), "bob", Intent::BuyYes, 100, 10.0, false);
        assert_eq!(r.filled_qty, 3.0);
        assert_eq!(r.resting_qty, 0.0);
        assert_eq!(b.total_resting(), 0.0);
    }

    #[test]
    fn cancel_removes_resting_order() {
        let mut b = OrderBook::new();
        let o = id();
        b.admit(o, "alice", Intent::BuyYes, 40, 5.0, true);
        assert_eq!(b.best_bid(), Some(40));
        let cancelled = b.cancel(o).unwrap();
        assert_eq!(cancelled.remaining, 5.0);
        assert_eq!(b.best_bid(), None);
        assert!(b.cancel(o).is_none());
    }
}
