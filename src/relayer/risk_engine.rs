#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_camel_case_types)]
use crate::config::*;
use crate::db::*;
use crate::kafkalib::kafka_health::KAFKA_UNHEALTHY;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

pub static STALE_PRICE_PAUSED: AtomicBool = AtomicBool::new(false);

/// How long an in-flight reservation may live before it is considered leaked and
/// reclaimed. Reservations are created at admission (`validate_open_order`) and
/// released when the order commits to filled/pending or is rejected. This TTL is a
/// self-healing safety net for any code path that fails to release explicitly; it is
/// swept lazily inside `validate_open_order`, so no background thread is required.
const RESERVATION_TTL: std::time::Duration = std::time::Duration::from_secs(120);

lazy_static! {
    pub static ref RISK_ENGINE_STATE: Arc<Mutex<RiskState>> =
        Arc::new(Mutex::new(RiskState::new()));
    pub static ref RISK_PARAMS: Arc<Mutex<RiskParams>> =
        Arc::new(Mutex::new(RiskParams::from_env()));
}

// --- Risk Parameters (loaded from env with defaults) ---

fn default_mm_ratio() -> f64 {
    0.4
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskParams {
    pub max_oi_mult: f64,      // alpha: max OI (USD) / pool equity (USD)
    pub max_net_mult: f64,     // beta: max net exposure (USD) / pool equity (USD)
    pub max_position_pct: f64, // gamma: max single position (USD) / pool equity (USD)
    pub min_position_btc: f64, // min position size in BTC entry value (im*lev); 0 = disabled
    pub max_leverage: f64,     // max leverage (0 = use existing limit)
    #[serde(default = "default_mm_ratio")]
    pub mm_ratio: f64,         // maintenance margin ratio (0.4 = 40%)
}

impl RiskParams {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();
        RiskParams {
            max_oi_mult: std::env::var("RISK_MAX_OI_MULT")
                .unwrap_or("4.0".to_string())
                .parse::<f64>()
                .unwrap_or(4.0),
            max_net_mult: std::env::var("RISK_MAX_NET_MULT")
                .unwrap_or("0.8".to_string())
                .parse::<f64>()
                .unwrap_or(0.8),
            max_position_pct: std::env::var("RISK_MAX_POSITION_PCT")
                .unwrap_or("0.02".to_string())
                .parse::<f64>()
                .unwrap_or(0.02),
            min_position_btc: std::env::var("RISK_MIN_POSITION_BTC")
                .unwrap_or("0.0".to_string())
                .parse::<f64>()
                .unwrap_or(0.0),
            max_leverage: std::env::var("RISK_MAX_LEVERAGE")
                .unwrap_or("50.0".to_string())
                .parse::<f64>()
                .unwrap_or(50.0),
            mm_ratio: std::env::var("RISK_MM_RATIO")
                .unwrap_or("0.4".to_string())
                .parse::<f64>()
                .unwrap_or(0.4),
        }
    }
}

/// V6 and earlier snapshot format — RiskParams WITHOUT mm_ratio
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskParamsOld {
    pub max_oi_mult: f64,
    pub max_net_mult: f64,
    pub max_position_pct: f64,
    pub min_position_btc: f64,
    pub max_leverage: f64,
}

impl RiskParamsOld {
    pub fn migrate_to_new(&self) -> RiskParams {
        RiskParams {
            max_oi_mult: self.max_oi_mult,
            max_net_mult: self.max_net_mult,
            max_position_pct: self.max_position_pct,
            min_position_btc: self.min_position_btc,
            max_leverage: self.max_leverage,
            mm_ratio: 0.4, // default
        }
    }
}

// --- Market Status ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MarketStatus {
    HEALTHY,
    CLOSE_ONLY,
    HALT,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum StatusReason {
    ManualHalt,
    ManualCloseOnly,
    PoolEquityInvalid,
}

impl StatusReason {
    pub fn to_string(&self) -> String {
        match self {
            StatusReason::ManualHalt => "MANUAL_HALT".to_string(),
            StatusReason::ManualCloseOnly => "MANUAL_CLOSE_ONLY".to_string(),
            StatusReason::PoolEquityInvalid => "POOL_EQUITY_INVALID".to_string(),
        }
    }
}

// --- Rejection Reasons ---
//
// Note: exposure-bearing fields are USD notional (Q = im_btc * leverage * entry_price).
// `BelowMinSize` is gated on BTC entry value (im_btc * leverage), which is price-independent.

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RiskRejectionReason {
    Halt(String),
    CloseOnly(String),
    InvalidParams,
    LeverageTooHigh,
    BelowMinSize,
    SizeTooLarge {
        requested_usd: f64,
        max_pos_usd: f64,
    },
    OiLimitReached {
        requested_usd: f64,
        oi_headroom_usd: f64,
    },
    SkewLimitReached {
        requested_usd: f64,
        net_headroom_usd: f64,
    },
    LimitReached {
        requested_usd: f64,
        allowed_usd: f64,
    },
    PriceFeedPaused,
    KafkaUnhealthy,
}

impl RiskRejectionReason {
    pub fn to_rejection_string(&self) -> String {
        match self {
            RiskRejectionReason::Halt(reason) => format!("HALT:{}", reason),
            RiskRejectionReason::CloseOnly(reason) => format!("CLOSE_ONLY:{}", reason),
            RiskRejectionReason::InvalidParams => "INVALID_PARAMS".to_string(),
            RiskRejectionReason::LeverageTooHigh => "LEVERAGE_TOO_HIGH".to_string(),
            RiskRejectionReason::BelowMinSize => "BELOW_MIN_SIZE".to_string(),
            RiskRejectionReason::SizeTooLarge { .. } => "SIZE_TOO_LARGE".to_string(),
            RiskRejectionReason::OiLimitReached { .. } => "OI_LIMIT_REACHED".to_string(),
            RiskRejectionReason::SkewLimitReached { .. } => "SKEW_LIMIT_REACHED".to_string(),
            RiskRejectionReason::LimitReached { .. } => "LIMIT_REACHED".to_string(),
            RiskRejectionReason::PriceFeedPaused => "PRICE_FEED_PAUSED".to_string(),
            RiskRejectionReason::KafkaUnhealthy => "KAFKA_UNHEALTHY".to_string(),
        }
    }
}

// --- Risk State (authoritative, persisted via events/snapshots) ---

// Old RiskState (V4 snapshot format — no pause flags). Historical: exposure stored
// as BTC entry value. Retained only for snapshot deserialization; the values are
// reconciled to USD from the order table on startup (see orchestration reconcile).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskStateOld {
    pub total_long_btc: f64,
    pub total_short_btc: f64,
    pub manual_halt: bool,
    pub manual_close_only: bool,
}

impl RiskStateOld {
    pub fn migrate_to_new(&self) -> RiskStateOldV5 {
        RiskStateOldV5 {
            total_long_btc: self.total_long_btc,
            total_short_btc: self.total_short_btc,
            manual_halt: self.manual_halt,
            manual_close_only: self.manual_close_only,
            pause_funding: false,
            pause_price_feed: false,
        }
    }
}

// V5 RiskState (no pending exposure fields). Historical, BTC-denominated.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskStateOldV5 {
    pub total_long_btc: f64,
    pub total_short_btc: f64,
    pub manual_halt: bool,
    pub manual_close_only: bool,
    pub pause_funding: bool,
    pub pause_price_feed: bool,
}

impl RiskStateOldV5 {
    pub fn migrate_to_new(&self) -> RiskState {
        // NOTE: The old fields are BTC entry value summed across differing entry
        // prices and cannot be converted to USD notional from the snapshot alone.
        // We carry them forward positionally; the authoritative exposure is
        // recomputed in USD from the order table on startup (reconcile). See
        // `RiskState::recalculate_exposure` and the orchestration load path.
        RiskState {
            total_long_usd: self.total_long_btc,
            total_short_usd: self.total_short_btc,
            total_pending_long_usd: 0.0,
            total_pending_short_usd: 0.0,
            manual_halt: self.manual_halt,
            manual_close_only: self.manual_close_only,
            pause_funding: self.pause_funding,
            pause_price_feed: self.pause_price_feed,
            reservations: HashMap::new(),
        }
    }
}

/// An in-flight admission reservation. Created under the `RISK_ENGINE_STATE` lock in
/// `validate_open_order` so check-and-reserve is atomic, and released when the order
/// commits (filled/pending) or is rejected. Transient: never serialized or
/// event-sourced.
#[derive(Debug, Clone, PartialEq)]
pub struct Reservation {
    pub side: PositionType,
    pub usd: f64,
    pub at: Instant,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskState {
    pub total_long_usd: f64, // sum of USD notional (im*lev*entry_price) for filled LONG positions
    pub total_short_usd: f64, // sum of USD notional for filled SHORT positions
    /// Informational only: USD notional of resting LONG limit orders. NOT enforced in
    /// caps (resting limits do not reserve headroom). Updated for observability.
    pub total_pending_long_usd: f64,
    /// Informational only: USD notional of resting SHORT limit orders. NOT enforced.
    pub total_pending_short_usd: f64,
    pub manual_halt: bool,
    pub manual_close_only: bool,
    pub pause_funding: bool,
    pub pause_price_feed: bool,
    /// In-flight admission reservations, keyed by order uuid. Transient runtime state:
    /// not serialized (rebuilt empty on load) and not event-sourced.
    #[serde(skip)]
    pub reservations: HashMap<Uuid, Reservation>,
}

impl RiskState {
    pub fn new() -> Self {
        RiskState {
            total_long_usd: 0.0,
            total_short_usd: 0.0,
            total_pending_long_usd: 0.0,
            total_pending_short_usd: 0.0,
            manual_halt: false,
            manual_close_only: false,
            pause_funding: false,
            pause_price_feed: false,
            reservations: HashMap::new(),
        }
    }

    /// Sum of reserved USD notional on a given side (in-flight admissions).
    fn reserved_usd(&self, side: &PositionType) -> f64 {
        self.reservations
            .values()
            .filter(|r| &r.side == side)
            .map(|r| r.usd)
            .sum()
    }

    /// Drop reservations older than `RESERVATION_TTL` (leak safety net).
    fn sweep_stale_reservations(&mut self) {
        let now = Instant::now();
        let before = self.reservations.len();
        self.reservations
            .retain(|_, r| now.duration_since(r.at) < RESERVATION_TTL);
        let dropped = before - self.reservations.len();
        if dropped > 0 {
            crate::log_heartbeat!(
                warn,
                "RISK_ENGINE: swept {} stale reservation(s) (TTL exceeded)",
                dropped
            );
        }
    }

    // --- Static methods that operate on the global RISK_ENGINE_STATE ---

    /// Release an in-flight reservation for `order_id`. Idempotent: a no-op if the
    /// reservation was already released or never existed. Call on commit and on every
    /// rejection path after a successful `validate_open_order`.
    pub fn finalize_reservation(order_id: Uuid) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.reservations.remove(&order_id);
        drop(state);
    }

    /// Add filled exposure in USD notional.
    pub fn add_order(position_type: PositionType, notional_usd: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_long_usd += notional_usd;
            }
            PositionType::SHORT => {
                state.total_short_usd += notional_usd;
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::AddExposure(position_type, notional_usd),
                state.clone(),
            ),
            String::from("AddRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    /// Remove filled exposure in USD notional.
    pub fn remove_order(position_type: PositionType, notional_usd: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_long_usd -= notional_usd;
                if state.total_long_usd < 0.0 {
                    state.total_long_usd = 0.0;
                }
            }
            PositionType::SHORT => {
                state.total_short_usd -= notional_usd;
                if state.total_short_usd < 0.0 {
                    state.total_short_usd = 0.0;
                }
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::RemoveExposure(position_type, notional_usd),
                state.clone(),
            ),
            String::from("RemoveRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    /// Add informational pending (resting limit) exposure in USD notional.
    pub fn add_pending_order(position_type: PositionType, notional_usd: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_pending_long_usd += notional_usd;
            }
            PositionType::SHORT => {
                state.total_pending_short_usd += notional_usd;
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::AddPendingExposure(position_type, notional_usd),
                state.clone(),
            ),
            String::from("AddPendingRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    /// Remove informational pending (resting limit) exposure in USD notional.
    pub fn remove_pending_order(position_type: PositionType, notional_usd: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_pending_long_usd -= notional_usd;
                if state.total_pending_long_usd < 0.0 {
                    state.total_pending_long_usd = 0.0;
                }
            }
            PositionType::SHORT => {
                state.total_pending_short_usd -= notional_usd;
                if state.total_pending_short_usd < 0.0 {
                    state.total_pending_short_usd = 0.0;
                }
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::RemovePendingExposure(position_type, notional_usd),
                state.clone(),
            ),
            String::from("RemovePendingRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    /// Overwrite exposure totals (USD notional). Used by the startup reconcile and the
    /// admin RecalculateRiskState endpoint, both of which recompute from the order table.
    pub fn recalculate_exposure(
        total_long_usd: f64,
        total_short_usd: f64,
        total_pending_long_usd: f64,
        total_pending_short_usd: f64,
    ) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.total_long_usd = total_long_usd;
        state.total_short_usd = total_short_usd;
        state.total_pending_long_usd = total_pending_long_usd;
        state.total_pending_short_usd = total_pending_short_usd;
        // A full recompute supersedes any in-flight reservations.
        state.reservations.clear();
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::RecalculateExposure,
                state.clone(),
            ),
            String::from("RecalculateRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn set_manual_halt(enabled: bool) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.manual_halt = enabled;
        Event::new(
            Event::RiskEngineUpdate(RiskEngineCommand::SetManualHalt(enabled), state.clone()),
            String::from("SetManualHalt"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn set_manual_close_only(enabled: bool) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.manual_close_only = enabled;
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::SetManualCloseOnly(enabled),
                state.clone(),
            ),
            String::from("SetManualCloseOnly"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn set_pause_funding(enabled: bool) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.pause_funding = enabled;
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::SetPauseFunding(enabled),
                state.clone(),
            ),
            String::from("SetPauseFunding"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn set_pause_price_feed(enabled: bool) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.pause_price_feed = enabled;
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::SetPausePriceFeed(enabled),
                state.clone(),
            ),
            String::from("SetPausePriceFeed"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn update_risk_params(new_params: RiskParams) {
        let mut params = RISK_PARAMS.lock().unwrap();
        *params = new_params.clone();
        Event::new(
            Event::RiskParamsUpdate(new_params),
            String::from("UpdateRiskParams"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(params);
    }
}

// --- Computed Limits (USD notional) ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskLimits {
    pub oi_max_usd: f64,
    pub net_max_usd: f64,
    pub pos_max_usd: f64,
    pub x_oi: f64,
    pub x_net_long: f64,
    pub x_net_short: f64,
    pub x_pos: f64,
    pub max_long_usd: f64,
    pub max_short_usd: f64,
}

// --- Market Risk Stats (for API response) ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarketRiskStats {
    pub pool_equity_btc: f64,
    pub pool_equity_usd: f64,
    pub mark_price: f64,
    pub total_long_usd: f64,
    pub total_short_usd: f64,
    pub reserved_long_usd: f64,
    pub reserved_short_usd: f64,
    pub pending_long_usd: f64,
    pub pending_short_usd: f64,
    pub open_interest_usd: f64,
    pub net_exposure_usd: f64,
    pub long_pct: f64,
    pub short_pct: f64,
    pub utilization: f64,
    pub max_long_usd: f64,
    pub max_short_usd: f64,
    pub status: MarketStatus,
    pub status_reason: Option<String>,
    pub params: RiskParams,
    pub kafka_unhealthy: bool,
}

// --- Core Risk Engine Functions ---

pub fn compute_market_status(
    state: &RiskState,
    pool_equity_btc: f64,
) -> (MarketStatus, Option<StatusReason>) {
    if state.manual_halt {
        return (MarketStatus::HALT, Some(StatusReason::ManualHalt));
    }
    if state.manual_close_only {
        return (
            MarketStatus::CLOSE_ONLY,
            Some(StatusReason::ManualCloseOnly),
        );
    }
    if pool_equity_btc <= 0.0 {
        return (MarketStatus::HALT, Some(StatusReason::PoolEquityInvalid));
    }
    (MarketStatus::HEALTHY, None)
}

/// Compute directional headroom in USD notional.
///
/// Effective exposure used for caps is filled + reserved (in-flight admissions).
/// Resting limit (pending) notional is informational and intentionally excluded.
/// `pool_equity_usd` must already be expressed in USD (= pool_equity_btc * mark_price).
pub fn compute_limits(
    total_long_usd: f64,
    total_short_usd: f64,
    reserved_long_usd: f64,
    reserved_short_usd: f64,
    pool_equity_usd: f64,
    status: &MarketStatus,
    params: &RiskParams,
) -> RiskLimits {
    let eff_long = total_long_usd + reserved_long_usd;
    let eff_short = total_short_usd + reserved_short_usd;

    let oi_usd = eff_long + eff_short;
    let net_usd = eff_long - eff_short;

    // Absolute caps (USD notional)
    let oi_max_usd = params.max_oi_mult * pool_equity_usd;
    let net_max_usd = params.max_net_mult * pool_equity_usd;
    let pos_max_usd = params.max_position_pct * pool_equity_usd;

    // Headroom
    let x_oi = f64::max(0.0, oi_max_usd - oi_usd);

    // Directional net headroom
    let x_net_long = f64::max(0.0, net_max_usd - net_usd);
    let x_net_short = f64::max(0.0, net_max_usd + net_usd);

    // Per-position cap
    let x_pos = pos_max_usd;

    // Directional limits
    let mut max_long_usd = f64::min(x_oi, f64::min(x_net_long, x_pos));
    let mut max_short_usd = f64::min(x_oi, f64::min(x_net_short, x_pos));

    // Status gating: no new opens unless HEALTHY
    if *status != MarketStatus::HEALTHY {
        max_long_usd = 0.0;
        max_short_usd = 0.0;
    }

    RiskLimits {
        oi_max_usd,
        net_max_usd,
        pos_max_usd,
        x_oi,
        x_net_long,
        x_net_short,
        x_pos,
        max_long_usd,
        max_short_usd,
    }
}

/// Validate and atomically reserve headroom for a new open order.
///
/// `entry_price` is the price at which this order's notional is fixed (mark price for
/// a market order, limit price for a limit order). `mark_price` is the current index
/// from the price feed, used to convert pool equity to USD and to guard against a bad
/// price. On success the order's USD notional is reserved against the live state under
/// the same lock that validated it; the caller MUST release it via
/// `RiskState::finalize_reservation(order_id)` when the order commits or is rejected.
pub fn validate_open_order(
    position_type: &PositionType,
    order_id: Uuid,
    im_btc: f64,
    leverage: f64,
    entry_price: f64,
    mark_price: f64,
    pool_equity_btc: f64,
    params: &RiskParams,
) -> Result<f64, RiskRejectionReason> {
    // Reject all opens when Kafka is unhealthy (no event sourcing guarantee)
    if KAFKA_UNHEALTHY.load(Ordering::Relaxed) {
        return Err(RiskRejectionReason::KafkaUnhealthy);
    }

    let mut state = RISK_ENGINE_STATE.lock().unwrap();

    // Reject all opens when price feed is paused (admin halt or stale price)
    if state.pause_price_feed || STALE_PRICE_PAUSED.load(Ordering::Relaxed) {
        return Err(RiskRejectionReason::PriceFeedPaused);
    }

    // Price-feed validity guard: never size or divide against a bad price.
    if !mark_price.is_finite() || mark_price <= 0.0 || !entry_price.is_finite() || entry_price <= 0.0
    {
        return Err(RiskRejectionReason::PriceFeedPaused);
    }

    // Reclaim any leaked reservations before computing headroom.
    state.sweep_stale_reservations();

    // Compute market status
    let (status, status_reason) = compute_market_status(&state, pool_equity_btc);

    // Global gating
    if status == MarketStatus::HALT {
        let reason = status_reason.map_or("UNKNOWN".to_string(), |r| r.to_string());
        return Err(RiskRejectionReason::Halt(reason));
    }
    if status == MarketStatus::CLOSE_ONLY {
        let reason = status_reason.map_or("UNKNOWN".to_string(), |r| r.to_string());
        return Err(RiskRejectionReason::CloseOnly(reason));
    }

    // Basic validation
    if im_btc <= 0.0 || leverage <= 0.0 {
        return Err(RiskRejectionReason::InvalidParams);
    }

    // Max leverage check
    if params.max_leverage > 0.0 && leverage > params.max_leverage {
        return Err(RiskRejectionReason::LeverageTooHigh);
    }

    // BTC entry value (price-independent) — used only for the min-size floor.
    let x_btc = im_btc * leverage;

    // Min position check (BTC entry value)
    if params.min_position_btc > 0.0 && x_btc < params.min_position_btc {
        return Err(RiskRejectionReason::BelowMinSize);
    }

    // Canonical exposure unit: USD notional Q = im_btc * leverage * entry_price.
    let x_usd = x_btc * entry_price;

    // Pool equity in USD, at current mark, so exposure and equity share a unit.
    let pool_equity_usd = pool_equity_btc * mark_price;

    // Compute limits against filled + reserved exposure.
    let reserved_long = state.reserved_usd(&PositionType::LONG);
    let reserved_short = state.reserved_usd(&PositionType::SHORT);
    let limits = compute_limits(
        state.total_long_usd,
        state.total_short_usd,
        reserved_long,
        reserved_short,
        pool_equity_usd,
        &status,
        params,
    );

    // Directional selection
    let (allowed, x_net_side) = match position_type {
        PositionType::LONG => (limits.max_long_usd, limits.x_net_long),
        PositionType::SHORT => (limits.max_short_usd, limits.x_net_short),
    };

    // Rejection precedence (tightest constraint first)
    if x_usd > limits.x_pos {
        return Err(RiskRejectionReason::SizeTooLarge {
            requested_usd: x_usd,
            max_pos_usd: limits.x_pos,
        });
    }
    if x_usd > limits.x_oi {
        return Err(RiskRejectionReason::OiLimitReached {
            requested_usd: x_usd,
            oi_headroom_usd: limits.x_oi,
        });
    }
    if x_usd > x_net_side {
        return Err(RiskRejectionReason::SkewLimitReached {
            requested_usd: x_usd,
            net_headroom_usd: x_net_side,
        });
    }
    if x_usd > allowed {
        return Err(RiskRejectionReason::LimitReached {
            requested_usd: x_usd,
            allowed_usd: allowed,
        });
    }

    // Admission passed: reserve the headroom atomically (same critical section that
    // validated it) so no concurrent admission can observe overlapping capacity.
    state.reservations.insert(
        order_id,
        Reservation {
            side: position_type.clone(),
            usd: x_usd,
            at: Instant::now(),
        },
    );

    drop(state);

    // Log admission
    crate::log_heartbeat!(
        info,
        "RISK_ENGINE: ACCEPT side={:?} id={} im={} lev={} entry={} x_usd={} pool_eq_usd={} allowed={}",
        position_type,
        order_id,
        im_btc,
        leverage,
        entry_price,
        x_usd,
        pool_equity_usd,
        allowed
    );

    Ok(x_usd)
}

pub fn validate_close_cancel_order(
    pool_equity_btc: f64,
    order_type: &OrderType,
) -> Result<(), RiskRejectionReason> {
    // Reject all operations when Kafka is unhealthy (no event sourcing guarantee)
    if KAFKA_UNHEALTHY.load(Ordering::Relaxed) {
        return Err(RiskRejectionReason::KafkaUnhealthy);
    }

    let state = RISK_ENGINE_STATE.lock().unwrap();

    // Reject non-lend orders when price feed is paused (admin halt or stale price)
    if (state.pause_price_feed || STALE_PRICE_PAUSED.load(Ordering::Relaxed)) && *order_type != OrderType::LEND {
        return Err(RiskRejectionReason::PriceFeedPaused);
    }

    let (status, status_reason) = compute_market_status(&state, pool_equity_btc);
    drop(state);

    if status == MarketStatus::HALT {
        let reason = status_reason.map_or("UNKNOWN".to_string(), |r| r.to_string());
        return Err(RiskRejectionReason::Halt(reason));
    }

    Ok(())
}

pub fn get_market_stats(pool_equity_btc: f64, mark_price: f64) -> MarketRiskStats {
    let state = RISK_ENGINE_STATE.lock().unwrap();
    let params = RISK_PARAMS.lock().unwrap().clone();

    let (status, status_reason) = compute_market_status(&state, pool_equity_btc);

    let total_long = state.total_long_usd;
    let total_short = state.total_short_usd;
    let reserved_long = state.reserved_usd(&PositionType::LONG);
    let reserved_short = state.reserved_usd(&PositionType::SHORT);
    let pending_long = state.total_pending_long_usd;
    let pending_short = state.total_pending_short_usd;

    // Pool equity expressed in USD at the current mark, so it shares a unit with exposure.
    let pool_equity_usd = if mark_price.is_finite() && mark_price > 0.0 {
        pool_equity_btc * mark_price
    } else {
        0.0
    };

    let eff_long = total_long + reserved_long;
    let eff_short = total_short + reserved_short;
    let oi_usd = eff_long + eff_short;
    let net_usd = eff_long - eff_short;

    let (long_pct, short_pct) = if oi_usd > 0.0 {
        (eff_long / oi_usd, eff_short / oi_usd)
    } else {
        (0.0, 0.0)
    };

    let utilization = if pool_equity_usd > 0.0 {
        oi_usd / pool_equity_usd
    } else {
        0.0
    };

    let limits = compute_limits(
        total_long,
        total_short,
        reserved_long,
        reserved_short,
        pool_equity_usd,
        &status,
        &params,
    );

    drop(state);

    MarketRiskStats {
        pool_equity_btc,
        pool_equity_usd,
        mark_price,
        total_long_usd: total_long,
        total_short_usd: total_short,
        reserved_long_usd: reserved_long,
        reserved_short_usd: reserved_short,
        pending_long_usd: pending_long,
        pending_short_usd: pending_short,
        open_interest_usd: oi_usd,
        net_exposure_usd: net_usd,
        long_pct,
        short_pct,
        utilization,
        max_long_usd: limits.max_long_usd,
        max_short_usd: limits.max_short_usd,
        status,
        status_reason: status_reason.map(|r| r.to_string()),
        params: params,
        kafka_unhealthy: KAFKA_UNHEALTHY.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(alpha: f64, beta: f64, gamma: f64) -> RiskParams {
        RiskParams {
            max_oi_mult: alpha,
            max_net_mult: beta,
            max_position_pct: gamma,
            min_position_btc: 0.0,
            max_leverage: 0.0,
            mm_ratio: 0.4,
        }
    }

    #[test]
    fn test_compute_limits_basic() {
        let params = make_params(4.0, 0.8, 0.02);
        let pool_equity_usd = 100.0; // 100 USD pool equity (already converted at mark)
        let status = MarketStatus::HEALTHY;

        let limits = compute_limits(0.0, 0.0, 0.0, 0.0, pool_equity_usd, &status, &params);

        assert_eq!(limits.oi_max_usd, 400.0);
        assert_eq!(limits.net_max_usd, 80.0);
        assert_eq!(limits.pos_max_usd, 2.0);
        assert_eq!(limits.x_oi, 400.0);
        assert_eq!(limits.x_net_long, 80.0);
        assert_eq!(limits.x_net_short, 80.0);
        // max_long = min(400, 80, 2) = 2
        assert_eq!(limits.max_long_usd, 2.0);
        assert_eq!(limits.max_short_usd, 2.0);
    }

    #[test]
    fn test_compute_limits_with_existing_positions() {
        let params = make_params(4.0, 0.8, 0.02);
        let pool_equity_usd = 100.0;
        let total_long = 120.0;
        let total_short = 80.0;
        let status = MarketStatus::HEALTHY;
        // OI = 200, Net = 40

        let limits = compute_limits(
            total_long,
            total_short,
            0.0,
            0.0,
            pool_equity_usd,
            &status,
            &params,
        );

        assert_eq!(limits.x_oi, 200.0); // 400 - 200
        assert_eq!(limits.x_net_long, 40.0); // 80 - 40
        assert_eq!(limits.x_net_short, 120.0); // 80 + 40
                                               // max_long = min(200, 40, 2) = 2
        assert_eq!(limits.max_long_usd, 2.0);
        // max_short = min(200, 120, 2) = 2
        assert_eq!(limits.max_short_usd, 2.0);
    }

    #[test]
    fn test_compute_limits_close_only() {
        let params = make_params(4.0, 0.8, 0.02);
        let status = MarketStatus::CLOSE_ONLY;

        let limits = compute_limits(0.0, 0.0, 0.0, 0.0, 100.0, &status, &params);

        assert_eq!(limits.max_long_usd, 0.0);
        assert_eq!(limits.max_short_usd, 0.0);
    }

    /// Headline case from the brief: a LONG opened at a low price and a SHORT opened
    /// at a higher price net to ZERO in BTC-at-entry but are NOT hedged in USD. Netting
    /// in USD notional must surface the residual pool-long delta.
    #[test]
    fn test_net_exposure_is_nonzero_in_usd() {
        // 0.2 BTC long @ $45,000 -> $9,000 notional; 0.2 BTC short @ $65,000 -> $13,000.
        let long_usd = 0.2 * 45_000.0; // 9_000
        let short_usd = 0.2 * 65_000.0; // 13_000

        // Old BTC netting would be 0.2 - 0.2 = 0 (falsely "hedged").
        let btc_net = 0.2_f64 - 0.2_f64;
        assert_eq!(btc_net, 0.0);

        // USD netting reveals the real residual delta.
        let net_usd = long_usd - short_usd;
        assert_eq!(net_usd, -4_000.0); // net SHORT by $4,000 in this long-minus-short convention

        // Via compute_limits: directional net headroom must reflect the $4,000 skew.
        let params = make_params(4.0, 0.8, 0.02);
        let pool_equity_usd = 100_000.0;
        let status = MarketStatus::HEALTHY;
        let limits = compute_limits(
            long_usd,
            short_usd,
            0.0,
            0.0,
            pool_equity_usd,
            &status,
            &params,
        );
        let net_max = 0.8 * pool_equity_usd; // 80_000
                                             // net = -4_000 -> x_net_long = net_max - net = 84_000; x_net_short = net_max + net = 76_000
        assert_eq!(limits.x_net_long, net_max - net_usd);
        assert_eq!(limits.x_net_short, net_max + net_usd);
        assert!(limits.x_net_long > limits.x_net_short); // book is pool-long-biased headroom
    }

    /// compute_limits must re-mark when the pool-equity-in-USD changes with price.
    #[test]
    fn test_caps_remark_with_mark_price() {
        let params = make_params(4.0, 0.8, 0.02);
        let status = MarketStatus::HEALTHY;
        let pool_equity_btc = 2.0;

        // At mark = $50k, pool equity = $100k -> oi cap = $400k.
        let limits_lo = compute_limits(
            0.0,
            0.0,
            0.0,
            0.0,
            pool_equity_btc * 50_000.0,
            &status,
            &params,
        );
        assert_eq!(limits_lo.oi_max_usd, 4.0 * 100_000.0);

        // At mark = $60k, pool equity = $120k -> oi cap = $480k. Caps must scale.
        let limits_hi = compute_limits(
            0.0,
            0.0,
            0.0,
            0.0,
            pool_equity_btc * 60_000.0,
            &status,
            &params,
        );
        assert_eq!(limits_hi.oi_max_usd, 4.0 * 120_000.0);
        assert!(limits_hi.oi_max_usd > limits_lo.oi_max_usd);
    }

    /// Reserved (in-flight) notional consumes headroom so concurrent admissions can't
    /// both pass against the same capacity.
    #[test]
    fn test_reserved_consumes_headroom() {
        let params = make_params(4.0, 0.8, 0.02);
        let status = MarketStatus::HEALTHY;
        let pool_equity_usd = 100_000.0; // pos cap = 2_000

        // No reservation: full per-position headroom.
        let l0 = compute_limits(0.0, 0.0, 0.0, 0.0, pool_equity_usd, &status, &params);
        assert_eq!(l0.x_oi, 4.0 * pool_equity_usd);

        // $300k reserved long eats OI headroom (cap 400k -> 100k remains).
        let l1 = compute_limits(0.0, 0.0, 300_000.0, 0.0, pool_equity_usd, &status, &params);
        assert_eq!(l1.x_oi, 4.0 * pool_equity_usd - 300_000.0);
        assert!(l1.x_oi < l0.x_oi);
    }

    #[test]
    fn test_compute_market_status_healthy() {
        let state = RiskState::new();
        let (status, reason) = compute_market_status(&state, 100.0);
        assert_eq!(status, MarketStatus::HEALTHY);
        assert!(reason.is_none());
    }

    #[test]
    fn test_compute_market_status_manual_halt() {
        let mut state = RiskState::new();
        state.manual_halt = true;
        let (status, reason) = compute_market_status(&state, 100.0);
        assert_eq!(status, MarketStatus::HALT);
        assert_eq!(reason.unwrap(), StatusReason::ManualHalt);
    }

    #[test]
    fn test_compute_market_status_zero_equity() {
        let state = RiskState::new();
        let (status, reason) = compute_market_status(&state, 0.0);
        assert_eq!(status, MarketStatus::HALT);
        assert_eq!(reason.unwrap(), StatusReason::PoolEquityInvalid);
    }

    #[test]
    fn test_compute_market_status_manual_close_only() {
        let mut state = RiskState::new();
        state.manual_close_only = true;
        let (status, reason) = compute_market_status(&state, 100.0);
        assert_eq!(status, MarketStatus::CLOSE_ONLY);
        assert_eq!(reason.unwrap(), StatusReason::ManualCloseOnly);
    }
}
