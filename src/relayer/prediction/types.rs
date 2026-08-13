//! Core value types for the binary prediction market.
//!
//! Everything the matching engine reasons about is normalized into
//! **YES-price space**: a probability in the open interval `(0, 1)`, stored as
//! an integer number of `ticks` against a per-market `price_scale`
//! (e.g. `price_scale = 100` gives a tick size of `0.01`, ticks `1..=99`).
//!
//! A NO order at NO-price `q` is the mirror of a YES order at `1 - q`, so a
//! `BuyNo`/`SellNo` is reflected around the scale before it enters the book.

use serde_derive::{Deserialize, Serialize};

/// Integer price in YES-ticks. Valid resting prices are `1..price_scale`.
pub type Ticks = u32;

/// Which outcome share an order refers to.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Yes,
    No,
}

/// The buy/sell direction, expressed in the order's own outcome space.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// A fully-specified trading intent: a `(Side, Outcome)` pair.
///
/// This is what a user actually submits. It is reflected into a single unified
/// YES book via [`Intent::book_side`] / [`Intent::to_yes_ticks`].
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    BuyYes,
    SellYes,
    BuyNo,
    SellNo,
}

impl Intent {
    pub fn from_parts(side: Side, outcome: Outcome) -> Intent {
        match (side, outcome) {
            (Side::Buy, Outcome::Yes) => Intent::BuyYes,
            (Side::Sell, Outcome::Yes) => Intent::SellYes,
            (Side::Buy, Outcome::No) => Intent::BuyNo,
            (Side::Sell, Outcome::No) => Intent::SellNo,
        }
    }

    /// Which side of the unified YES book this intent rests on.
    /// Bids want to acquire YES exposure (BuyYes / SellNo); asks shed it.
    pub fn book_side(&self) -> BookSide {
        match self {
            Intent::BuyYes | Intent::SellNo => BookSide::Bid,
            Intent::SellYes | Intent::BuyNo => BookSide::Ask,
        }
    }

    /// True for buys (which lock cash); false for sells (which reserve shares).
    pub fn is_buy(&self) -> bool {
        matches!(self, Intent::BuyYes | Intent::BuyNo)
    }

    /// The outcome share this intent buys or sells.
    pub fn outcome(&self) -> Outcome {
        match self {
            Intent::BuyYes | Intent::SellYes => Outcome::Yes,
            Intent::BuyNo | Intent::SellNo => Outcome::No,
        }
    }

    /// Convert a price expressed in this intent's own outcome space into a
    /// YES-book tick. YES intents pass through; NO intents mirror around scale.
    pub fn to_yes_ticks(&self, own_ticks: Ticks, scale: Ticks) -> Ticks {
        match self.outcome() {
            Outcome::Yes => own_ticks,
            Outcome::No => scale - own_ticks,
        }
    }
}

/// Which side of the unified YES order book a resting order sits on.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSide {
    Bid,
    Ask,
}

/// Whether an order rests on the book (`Limit`) or crosses immediately and
/// cancels any unfilled remainder (`Market`).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

/// Lifecycle of a single order.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    /// Resting on the book with quantity remaining.
    Open,
    /// Some quantity filled, remainder resting.
    PartiallyFilled,
    /// Fully filled.
    Filled,
    /// Cancelled (by user, market halt, or unfilled market-order remainder).
    Cancelled,
    /// Rejected at placement (validation / insufficient collateral).
    Rejected,
}

/// Lifecycle of a market.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketStatus {
    /// Trading is open.
    Open,
    /// Trading halted (past close, or admin halt); no new fills.
    Halted,
    /// Resolved; shares have been redeemed.
    Resolved,
}

/// The outcome an oracle reports at resolution.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Yes,
    No,
    /// Neither side won; every share redeems at `0.5`, so a complete
    /// (YES+NO) set is still worth exactly `1.0`.
    Invalid,
}

/// Convert YES-ticks to an absolute probability in `(0, 1)`.
#[inline]
pub fn ticks_to_price(ticks: Ticks, scale: Ticks) -> f64 {
    ticks as f64 / scale as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_reflection_is_symmetric() {
        let scale = 100;
        // Buy NO @ 0.30 is a YES ask at 0.70.
        assert_eq!(Intent::BuyNo.to_yes_ticks(30, scale), 70);
        assert_eq!(Intent::BuyNo.book_side(), BookSide::Ask);
        // Sell NO @ 0.30 is a YES bid at 0.70.
        assert_eq!(Intent::SellNo.to_yes_ticks(30, scale), 70);
        assert_eq!(Intent::SellNo.book_side(), BookSide::Bid);
        // YES intents pass through untouched.
        assert_eq!(Intent::BuyYes.to_yes_ticks(62, scale), 62);
        assert_eq!(Intent::SellYes.to_yes_ticks(62, scale), 62);
    }

    #[test]
    fn book_sides_and_buy_flags() {
        assert_eq!(Intent::BuyYes.book_side(), BookSide::Bid);
        assert_eq!(Intent::SellYes.book_side(), BookSide::Ask);
        assert!(Intent::BuyYes.is_buy());
        assert!(Intent::BuyNo.is_buy());
        assert!(!Intent::SellYes.is_buy());
        assert!(!Intent::SellNo.is_buy());
    }

    #[test]
    fn from_parts_round_trips() {
        assert_eq!(Intent::from_parts(Side::Buy, Outcome::Yes), Intent::BuyYes);
        assert_eq!(Intent::from_parts(Side::Sell, Outcome::No), Intent::SellNo);
    }
}
