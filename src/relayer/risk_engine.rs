#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_camel_case_types)]
use crate::config::*;
use crate::db::*;
use crate::relayer::*;
use serde_derive::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

lazy_static! {
    pub static ref RISK_ENGINE_STATE: Arc<Mutex<RiskState>> =
        Arc::new(Mutex::new(RiskState::new()));
    pub static ref RISK_PARAMS: Arc<Mutex<RiskParams>> =
        Arc::new(Mutex::new(RiskParams::from_env()));
}

// --- Risk Parameters (loaded from env with defaults) ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskParams {
    pub max_oi_mult: f64,      // alpha: max OI / pool equity
    pub max_net_mult: f64,     // beta: max net exposure / pool equity
    pub max_position_pct: f64, // gamma: max single position / pool equity
    pub min_position_btc: f64, // min position size in sats (0 = disabled)
    pub max_leverage: f64,     // max leverage (0 = use existing limit)
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RiskRejectionReason {
    Halt(String),
    CloseOnly(String),
    InvalidParams,
    LeverageTooHigh,
    BelowMinSize,
    SizeTooLarge {
        requested_btc: f64,
        max_pos_btc: f64,
    },
    OiLimitReached {
        requested_btc: f64,
        oi_headroom_btc: f64,
    },
    SkewLimitReached {
        requested_btc: f64,
        net_headroom_btc: f64,
    },
    LimitReached {
        requested_btc: f64,
        allowed_btc: f64,
    },
    PriceFeedPaused,
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
        }
    }
}

// --- Risk State (authoritative, persisted via events/snapshots) ---

// Old RiskState (V4 snapshot format — no pause flags)
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

// V5 RiskState (no pending exposure fields)
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
        RiskState {
            total_long_btc: self.total_long_btc,
            total_short_btc: self.total_short_btc,
            total_pending_long_btc: 0.0,
            total_pending_short_btc: 0.0,
            manual_halt: self.manual_halt,
            manual_close_only: self.manual_close_only,
            pause_funding: self.pause_funding,
            pause_price_feed: self.pause_price_feed,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskState {
    pub total_long_btc: f64, // sum of entry_value (sats) for all filled LONG positions
    pub total_short_btc: f64, // sum of entry_value (sats) for all filled SHORT positions
    pub total_pending_long_btc: f64, // sum of entry_value (sats) for all pending LONG limit orders
    pub total_pending_short_btc: f64, // sum of entry_value (sats) for all pending SHORT limit orders
    pub manual_halt: bool,
    pub manual_close_only: bool,
    pub pause_funding: bool,
    pub pause_price_feed: bool,
}

impl RiskState {
    pub fn new() -> Self {
        RiskState {
            total_long_btc: 0.0,
            total_short_btc: 0.0,
            total_pending_long_btc: 0.0,
            total_pending_short_btc: 0.0,
            manual_halt: false,
            manual_close_only: false,
            pause_funding: false,
            pause_price_feed: false,
        }
    }

    // --- Static methods that operate on the global RISK_ENGINE_STATE ---

    pub fn add_order(position_type: PositionType, entry_value: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_long_btc += entry_value;
            }
            PositionType::SHORT => {
                state.total_short_btc += entry_value;
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::AddExposure(position_type, entry_value),
                state.clone(),
            ),
            String::from("AddRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn remove_order(position_type: PositionType, entry_value: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_long_btc -= entry_value;
                if state.total_long_btc < 0.0 {
                    state.total_long_btc = 0.0;
                }
            }
            PositionType::SHORT => {
                state.total_short_btc -= entry_value;
                if state.total_short_btc < 0.0 {
                    state.total_short_btc = 0.0;
                }
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::RemoveExposure(position_type, entry_value),
                state.clone(),
            ),
            String::from("RemoveRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn add_pending_order(position_type: PositionType, entry_value: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_pending_long_btc += entry_value;
            }
            PositionType::SHORT => {
                state.total_pending_short_btc += entry_value;
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::AddPendingExposure(position_type, entry_value),
                state.clone(),
            ),
            String::from("AddPendingRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn remove_pending_order(position_type: PositionType, entry_value: f64) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        match position_type {
            PositionType::LONG => {
                state.total_pending_long_btc -= entry_value;
                if state.total_pending_long_btc < 0.0 {
                    state.total_pending_long_btc = 0.0;
                }
            }
            PositionType::SHORT => {
                state.total_pending_short_btc -= entry_value;
                if state.total_pending_short_btc < 0.0 {
                    state.total_pending_short_btc = 0.0;
                }
            }
        }
        Event::new(
            Event::RiskEngineUpdate(
                RiskEngineCommand::RemovePendingExposure(position_type, entry_value),
                state.clone(),
            ),
            String::from("RemovePendingRiskExposure"),
            CORE_EVENT_LOG.clone().to_string(),
        );
        drop(state);
    }

    pub fn recalculate_exposure(
        total_long_btc: f64,
        total_short_btc: f64,
        total_pending_long_btc: f64,
        total_pending_short_btc: f64,
    ) {
        let mut state = RISK_ENGINE_STATE.lock().unwrap();
        state.total_long_btc = total_long_btc;
        state.total_short_btc = total_short_btc;
        state.total_pending_long_btc = total_pending_long_btc;
        state.total_pending_short_btc = total_pending_short_btc;
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

// --- Computed Limits ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RiskLimits {
    pub oi_max_btc: f64,
    pub net_max_btc: f64,
    pub pos_max_btc: f64,
    pub x_oi: f64,
    pub x_net_long: f64,
    pub x_net_short: f64,
    pub x_pos: f64,
    pub max_long_btc: f64,
    pub max_short_btc: f64,
}

// --- Market Risk Stats (for API response) ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarketRiskStats {
    pub pool_equity_btc: f64,
    pub total_long_btc: f64,
    pub total_short_btc: f64,
    pub open_interest_btc: f64,
    pub net_exposure_btc: f64,
    pub long_pct: f64,
    pub short_pct: f64,
    pub utilization: f64,
    pub max_long_btc: f64,
    pub max_short_btc: f64,
    pub status: MarketStatus,
    pub status_reason: Option<String>,
    pub params: RiskParams,
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

pub fn compute_limits(
    total_long_btc: f64,
    total_short_btc: f64,
    pool_equity_btc: f64,
    status: &MarketStatus,
    params: &RiskParams,
) -> RiskLimits {
    let oi_btc = total_long_btc + total_short_btc;
    let net_btc = total_long_btc - total_short_btc;

    // Absolute caps (BTC/sats)
    let oi_max_btc = params.max_oi_mult * pool_equity_btc;
    let net_max_btc = params.max_net_mult * pool_equity_btc;
    let pos_max_btc = params.max_position_pct * pool_equity_btc;

    // Headroom
    let x_oi = f64::max(0.0, oi_max_btc - oi_btc);

    // Directional net headroom
    let x_net_long = f64::max(0.0, net_max_btc - net_btc);
    let x_net_short = f64::max(0.0, net_max_btc + net_btc);

    // Per-position cap
    let x_pos = pos_max_btc;

    // Directional limits
    let mut max_long_btc = f64::min(x_oi, f64::min(x_net_long, x_pos));
    let mut max_short_btc = f64::min(x_oi, f64::min(x_net_short, x_pos));

    // Status gating: no new opens unless HEALTHY
    if *status != MarketStatus::HEALTHY {
        max_long_btc = 0.0;
        max_short_btc = 0.0;
    }

    RiskLimits {
        oi_max_btc,
        net_max_btc,
        pos_max_btc,
        x_oi,
        x_net_long,
        x_net_short,
        x_pos,
        max_long_btc,
        max_short_btc,
    }
}

pub fn validate_open_order(
    position_type: &PositionType,
    im_btc: f64,
    leverage: f64,
    pool_equity_btc: f64,
    params: &RiskParams,
) -> Result<f64, RiskRejectionReason> {
    let state = RISK_ENGINE_STATE.lock().unwrap();

    // Reject all opens when price feed is paused
    if state.pause_price_feed {
        return Err(RiskRejectionReason::PriceFeedPaused);
    }

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

    // Canonical size: entry_value = IM * leverage (sats)
    let x_btc = im_btc * leverage;

    // Min position check
    if params.min_position_btc > 0.0 && x_btc < params.min_position_btc {
        return Err(RiskRejectionReason::BelowMinSize);
    }

    // Compute limits
    let limits = compute_limits(
        state.total_long_btc,
        state.total_short_btc,
        pool_equity_btc,
        &status,
        params,
    );

    drop(state);

    // Directional selection
    let (allowed, x_net_side) = match position_type {
        PositionType::LONG => (limits.max_long_btc, limits.x_net_long),
        PositionType::SHORT => (limits.max_short_btc, limits.x_net_short),
    };

    // Rejection precedence (tightest constraint first)
    if x_btc > limits.x_pos {
        return Err(RiskRejectionReason::SizeTooLarge {
            requested_btc: x_btc,
            max_pos_btc: limits.x_pos,
        });
    }
    if x_btc > limits.x_oi {
        return Err(RiskRejectionReason::OiLimitReached {
            requested_btc: x_btc,
            oi_headroom_btc: limits.x_oi,
        });
    }
    if x_btc > x_net_side {
        return Err(RiskRejectionReason::SkewLimitReached {
            requested_btc: x_btc,
            net_headroom_btc: x_net_side,
        });
    }
    if x_btc > allowed {
        return Err(RiskRejectionReason::LimitReached {
            requested_btc: x_btc,
            allowed_btc: allowed,
        });
    }

    // Log admission
    crate::log_heartbeat!(
        info,
        "RISK_ENGINE: ACCEPT side={:?} im={} lev={} x_btc={} pool_eq={} allowed={}",
        position_type,
        im_btc,
        leverage,
        x_btc,
        pool_equity_btc,
        allowed
    );

    Ok(x_btc)
}

pub fn validate_close_cancel_order(
    pool_equity_btc: f64,
    order_type: &OrderType,
) -> Result<(), RiskRejectionReason> {
    let state = RISK_ENGINE_STATE.lock().unwrap();

    // Reject non-lend orders when price feed is paused
    if state.pause_price_feed && *order_type != OrderType::LEND {
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

    let total_long = state.total_long_btc;
    let total_short = state.total_short_btc;

    let oi_btc = total_long + total_short;
    let net_btc = total_long - total_short;

    let (long_pct, short_pct) = if oi_btc > 0.0 {
        (total_long / oi_btc, total_short / oi_btc)
    } else {
        (0.0, 0.0)
    };

    let utilization = if pool_equity_btc > 0.0 {
        oi_btc / pool_equity_btc
    } else {
        0.0
    };

    let limits = compute_limits(total_long, total_short, pool_equity_btc, &status, &params);

    drop(state);

    MarketRiskStats {
        pool_equity_btc,
        total_long_btc: total_long,
        total_short_btc: total_short,
        open_interest_btc: oi_btc,
        net_exposure_btc: net_btc,
        long_pct,
        short_pct,
        utilization,
        max_long_btc: limits.max_long_btc,
        max_short_btc: limits.max_short_btc,
        status,
        status_reason: status_reason.map(|r| r.to_string()),
        params: params,
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
        }
    }

    #[test]
    fn test_compute_limits_basic() {
        let params = make_params(4.0, 0.8, 0.02);
        let pool_equity = 100.0; // 100 sats pool equity
        let total_long = 0.0;
        let total_short = 0.0;
        let status = MarketStatus::HEALTHY;

        let limits = compute_limits(total_long, total_short, pool_equity, &status, &params);

        assert_eq!(limits.oi_max_btc, 400.0);
        assert_eq!(limits.net_max_btc, 80.0);
        assert_eq!(limits.pos_max_btc, 2.0);
        assert_eq!(limits.x_oi, 400.0);
        assert_eq!(limits.x_net_long, 80.0);
        assert_eq!(limits.x_net_short, 80.0);
        // max_long = min(400, 80, 2) = 2
        assert_eq!(limits.max_long_btc, 2.0);
        assert_eq!(limits.max_short_btc, 2.0);
    }

    #[test]
    fn test_compute_limits_with_existing_positions() {
        let params = make_params(4.0, 0.8, 0.02);
        let pool_equity = 100.0;
        let total_long = 120.0;
        let total_short = 80.0;
        let status = MarketStatus::HEALTHY;
        // OI = 200, Net = 40

        let limits = compute_limits(total_long, total_short, pool_equity, &status, &params);

        assert_eq!(limits.x_oi, 200.0); // 400 - 200
        assert_eq!(limits.x_net_long, 40.0); // 80 - 40
        assert_eq!(limits.x_net_short, 120.0); // 80 + 40
                                               // max_long = min(200, 40, 2) = 2
        assert_eq!(limits.max_long_btc, 2.0);
        // max_short = min(200, 120, 2) = 2
        assert_eq!(limits.max_short_btc, 2.0);
    }

    #[test]
    fn test_compute_limits_close_only() {
        let params = make_params(4.0, 0.8, 0.02);
        let status = MarketStatus::CLOSE_ONLY;

        let limits = compute_limits(0.0, 0.0, 100.0, &status, &params);

        assert_eq!(limits.max_long_btc, 0.0);
        assert_eq!(limits.max_short_btc, 0.0);
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
