//! Binary prediction market — self-contained domain core (Phase 1).
//!
//! A user-to-user central limit order book for binary (YES/NO) outcome shares.
//! Users trade shares priced in `(0, 1)`; at resolution the winning outcome's
//! shares redeem for `$1` and the losing outcome's for `$0`.
//!
//! This module is deliberately decoupled from Kafka, the ZKOS chain, and the
//! legacy perps SDK: it is pure Rust (`std` + `serde` + `uuid`) so the matching
//! and settlement logic can be exhaustively unit-tested off-chain before the
//! transport and on-chain layers are wired in (Phases 2–4).
//!
//! Layers:
//! - [`types`]    — value types and YES-price-space normalization.
//! - [`book`]     — the unified order book + price-time matching engine.
//! - [`market`]   — one market's positions, vault, and resolution payout.
//! - [`registry`] — cash/collateral orchestration + domain events.

pub mod book;
pub mod market;
pub mod registry;
pub mod types;

pub use book::{Fill, FillKind, MatchResult, OrderBook};
pub use market::{Market, Position};
pub use registry::{Account, DomainEvent, MarketRegistry, PlaceReport, RejectReason};
pub use types::{
    ticks_to_price, BookSide, Intent, MarketStatus, OrderStatus, OrderType, Outcome, Resolution,
    Side, Ticks,
};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    const SCALE: Ticks = 100; // tick size 0.01

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
    }

    /// Total cash in the system across all accounts plus every market vault.
    /// This may only change via deposits — never through trading.
    fn system_cash(reg: &MarketRegistry, accounts: &[&str], markets: &[Uuid]) -> f64 {
        let acct: f64 = accounts.iter().map(|a| reg.account(a).total()).sum();
        let vault: f64 = markets.iter().filter_map(|m| reg.market(m)).map(|m| m.vault).sum();
        acct + vault
    }

    #[test]
    fn mint_set_then_resolve_pays_the_winner() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("Will it rain tomorrow?", SCALE, 1.0);
        reg.deposit("alice", 100.0);
        reg.deposit("bob", 100.0);

        // Alice buys YES @ 0.60, Bob buys NO @ 0.40 -> mint 10 complete sets.
        reg.place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 60, 10.0, OrderType::Limit).unwrap();
        let r = reg
            .place_order(&mkt, "bob", Side::Buy, Outcome::No, 40, 10.0, OrderType::Limit)
            .unwrap();
        assert_eq!(r.status, OrderStatus::Filled);
        assert_eq!(r.fills[0].kind, FillKind::MintSet);

        approx(reg.position(&mkt, "alice").yes, 10.0);
        approx(reg.position(&mkt, "bob").no, 10.0);
        approx(reg.market(&mkt).unwrap().vault, 10.0); // $1 x 10 sets
        // Alice spent 6, Bob spent 4.
        approx(reg.account("alice").free, 94.0);
        approx(reg.account("bob").free, 96.0);

        // Resolve YES: Alice's 10 YES pay $10, Bob's NO pay $0.
        reg.resolve_market(&mkt, Resolution::Yes).unwrap();
        approx(reg.account("alice").free, 104.0);
        approx(reg.account("bob").free, 96.0);
        approx(reg.market(&mkt).unwrap().vault, 0.0);
        // System cash conserved at the $200 deposited.
        approx(system_cash(&reg, &["alice", "bob"], &[mkt]), 200.0);
    }

    #[test]
    fn taker_gets_price_improvement_refund() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("maker", 100.0);
        reg.deposit("taker", 100.0);

        // Maker sells YES @ 0.50 (must own shares first: give via a mint).
        // Give maker YES shares by minting against a helper.
        reg.deposit("helper", 100.0);
        reg.place_order(&mkt, "maker", Side::Buy, Outcome::Yes, 50, 10.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "helper", Side::Buy, Outcome::No, 50, 10.0, OrderType::Limit).unwrap();
        approx(reg.position(&mkt, "maker").yes, 10.0);

        // Maker rests a sell YES @ 0.50.
        reg.place_order(&mkt, "maker", Side::Sell, Outcome::Yes, 50, 10.0, OrderType::Limit).unwrap();
        // Taker buys YES with a generous limit of 0.70 -> fills at 0.50, refunds 0.20.
        let before = reg.account("taker").free;
        let r = reg
            .place_order(&mkt, "taker", Side::Buy, Outcome::Yes, 70, 10.0, OrderType::Limit)
            .unwrap();
        assert_eq!(r.status, OrderStatus::Filled);
        // Taker paid 0.50 x 10 = 5, not 7. Free dropped by exactly 5.
        approx(reg.account("taker").free, before - 5.0);
        approx(reg.position(&mkt, "taker").yes, 10.0);
        // Maker received 0.50 x 10 = 5 cash for the shares sold.
    }

    #[test]
    fn cancel_releases_locked_cash() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("alice", 100.0);
        let r = reg
            .place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 30, 10.0, OrderType::Limit)
            .unwrap();
        assert_eq!(r.status, OrderStatus::Open);
        approx(reg.account("alice").free, 97.0); // 3 locked
        approx(reg.account("alice").locked, 3.0);
        reg.cancel_order(&mkt, r.order_id).unwrap();
        approx(reg.account("alice").free, 100.0);
        approx(reg.account("alice").locked, 0.0);
    }

    #[test]
    fn insufficient_cash_is_rejected() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("alice", 2.0);
        // Buy YES @ 0.60 x 10 = 6 cost, only 2 available.
        let err = reg
            .place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 60, 10.0, OrderType::Limit)
            .unwrap_err();
        assert_eq!(err, RejectReason::InsufficientCash);
    }

    #[test]
    fn cannot_sell_shares_you_dont_have() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("alice", 100.0);
        let err = reg
            .place_order(&mkt, "alice", Side::Sell, Outcome::Yes, 60, 5.0, OrderType::Limit)
            .unwrap_err();
        assert_eq!(err, RejectReason::InsufficientShares);
    }

    #[test]
    fn merge_set_returns_collateral_to_both_sellers() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        for who in ["a", "b"] {
            reg.deposit(who, 100.0);
        }
        // a holds YES, b holds NO by minting a set at 0.50 each.
        reg.place_order(&mkt, "a", Side::Buy, Outcome::Yes, 50, 10.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "b", Side::Buy, Outcome::No, 50, 10.0, OrderType::Limit).unwrap();
        approx(reg.market(&mkt).unwrap().vault, 10.0);

        // b sells NO @ 0.30 (rests as YES bid 0.70); a sells YES @ 0.70 -> merge.
        reg.place_order(&mkt, "b", Side::Sell, Outcome::No, 30, 10.0, OrderType::Limit).unwrap();
        let r = reg
            .place_order(&mkt, "a", Side::Sell, Outcome::Yes, 70, 10.0, OrderType::Limit)
            .unwrap();
        assert_eq!(r.fills[0].kind, FillKind::MergeSet);
        // Vault drained back to zero; both flattened their share holdings.
        approx(reg.market(&mkt).unwrap().vault, 0.0);
        approx(reg.position(&mkt, "a").yes, 0.0);
        approx(reg.position(&mkt, "b").no, 0.0);
        // Cash conserved.
        approx(system_cash(&reg, &["a", "b"], &[mkt]), 200.0);
    }

    #[test]
    fn invalid_resolution_makes_a_set_worth_one_dollar() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("alice", 100.0);
        reg.deposit("bob", 100.0);
        reg.place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 60, 10.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "bob", Side::Buy, Outcome::No, 40, 10.0, OrderType::Limit).unwrap();
        reg.resolve_market(&mkt, Resolution::Invalid).unwrap();
        // Each share pays 0.5 regardless of entry price. Alice bought YES at
        // 0.60 (overpaid vs fair 0.50) so nets -0.10/share; Bob bought NO at
        // 0.40 (underpaid) so nets +0.10/share. The pair nets to zero.
        approx(reg.account("alice").free, 99.0); // 94 + 5
        approx(reg.account("bob").free, 101.0); // 96 + 5
        approx(system_cash(&reg, &["alice", "bob"], &[mkt]), 200.0);
    }

    #[test]
    fn halted_market_rejects_new_orders_and_refunds_resting() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        reg.deposit("alice", 100.0);
        let r = reg
            .place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 30, 10.0, OrderType::Limit)
            .unwrap();
        reg.halt_market(&mkt).unwrap();
        approx(reg.account("alice").free, 100.0); // resting buy refunded
        let err = reg
            .place_order(&mkt, "alice", Side::Buy, Outcome::Yes, 30, 10.0, OrderType::Limit)
            .unwrap_err();
        assert_eq!(err, RejectReason::MarketNotOpen);
        let _ = r;
    }

    #[test]
    fn full_book_conserves_cash_through_a_trading_session() {
        let mut reg = MarketRegistry::new();
        let (mkt, _) = reg.create_market("q", SCALE, 1.0);
        let who = ["a", "b", "c", "d"];
        for w in who {
            reg.deposit(w, 1000.0);
        }
        // A messy sequence of crossing and resting orders.
        reg.place_order(&mkt, "a", Side::Buy, Outcome::Yes, 55, 20.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "b", Side::Buy, Outcome::No, 40, 15.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "c", Side::Buy, Outcome::No, 50, 30.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "d", Side::Buy, Outcome::Yes, 60, 25.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "a", Side::Sell, Outcome::Yes, 45, 5.0, OrderType::Limit).unwrap();
        reg.place_order(&mkt, "b", Side::Buy, Outcome::Yes, 70, 10.0, OrderType::Market).unwrap();
        approx(system_cash(&reg, &who, &[mkt]), 4000.0);
        reg.resolve_market(&mkt, Resolution::No).unwrap();
        approx(system_cash(&reg, &who, &[mkt]), 4000.0);
        // No locked cash should remain after resolution.
        let locked: f64 = who.iter().map(|w| reg.account(w).locked).sum();
        approx(locked, 0.0);
    }
}
