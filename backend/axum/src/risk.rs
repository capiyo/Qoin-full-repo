//! Independent, fail-closed risk gate. This module is not part of signal scoring.
//! Missing account, exposure, feed, or execution-protection data always vetoes entry.

#[derive(Debug, Clone, PartialEq)]
pub struct RiskLimits {
    pub risk_per_trade_fraction: f64,
    pub max_leverage: f64,
    pub max_positions: usize,
    pub max_correlated_exposure_fraction: f64,
    pub max_daily_loss_fraction: f64,
    pub max_weekly_loss_fraction: f64,
    pub max_drawdown_fraction: f64,
    pub max_spread_bps: f64,
    pub max_slippage_bps: f64,
    pub max_feed_age_ms: u64,
    pub min_state_quality: f64,
}

impl RiskLimits {
    /// Policy values explicitly supplied by the project owner. Unknown caps are
    /// deliberately not filled with invented production values.
    pub fn target_policy() -> Self {
        Self {
            risk_per_trade_fraction: 0.0025,
            max_leverage: 2.0,
            max_positions: 0,
            max_correlated_exposure_fraction: f64::NAN,
            max_daily_loss_fraction: f64::NAN,
            max_weekly_loss_fraction: f64::NAN,
            max_drawdown_fraction: f64::NAN,
            max_spread_bps: f64::NAN,
            max_slippage_bps: f64::NAN,
            max_feed_age_ms: 0,
            min_state_quality: f64::NAN,
        }
    }

    /// Load owner-supplied caps. Unknown limits are required, never guessed.
    pub fn from_env() -> Result<Self, String> {
        let number = |name: &str| -> Result<f64, String> {
            let raw = std::env::var(name).map_err(|_| format!("missing required risk setting {name}"))?;
            raw.parse::<f64>().map_err(|_| format!("risk setting {name} must be numeric"))
        };
        let count = std::env::var("QOIN_MAX_POSITIONS")
            .map_err(|_| "missing required risk setting QOIN_MAX_POSITIONS".to_string())?
            .parse::<usize>().map_err(|_| "QOIN_MAX_POSITIONS must be an integer".to_string())?;
        let max_feed_age_ms = std::env::var("QOIN_MAX_FEED_AGE_MS")
            .map_err(|_| "missing required risk setting QOIN_MAX_FEED_AGE_MS".to_string())?
            .parse::<u64>().map_err(|_| "QOIN_MAX_FEED_AGE_MS must be an integer".to_string())?;
        let policy = Self {
            risk_per_trade_fraction: 0.0025,
            max_leverage: 2.0,
            max_positions: count,
            max_correlated_exposure_fraction: number("QOIN_MAX_CORRELATED_EXPOSURE_FRACTION")?,
            max_daily_loss_fraction: number("QOIN_MAX_DAILY_LOSS_FRACTION")?,
            max_weekly_loss_fraction: number("QOIN_MAX_WEEKLY_LOSS_FRACTION")?,
            max_drawdown_fraction: number("QOIN_MAX_DRAWDOWN_FRACTION")?,
            max_spread_bps: number("QOIN_RISK_MAX_SPREAD_BPS")?,
            max_slippage_bps: number("QOIN_RISK_MAX_SLIPPAGE_BPS")?,
            max_feed_age_ms,
            min_state_quality: number("QOIN_RISK_MIN_STATE_QUALITY")?,
        };
        policy.validate().map_err(str::to_owned)?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.risk_per_trade_fraction.is_finite()
            || self.risk_per_trade_fraction <= 0.0
            || self.risk_per_trade_fraction > 0.0025
            || !self.max_leverage.is_finite()
            || self.max_leverage <= 0.0
            || self.max_leverage > 2.0
            || self.max_positions == 0
            || !self.max_correlated_exposure_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.max_correlated_exposure_fraction)
            || !self.max_daily_loss_fraction.is_finite()
            || !(0.0..1.0).contains(&self.max_daily_loss_fraction)
            || !self.max_weekly_loss_fraction.is_finite()
            || !(0.0..1.0).contains(&self.max_weekly_loss_fraction)
            || !self.max_drawdown_fraction.is_finite()
            || !(0.0..1.0).contains(&self.max_drawdown_fraction)
            || !self.max_spread_bps.is_finite()
            || self.max_spread_bps <= 0.0
            || !self.max_slippage_bps.is_finite()
            || self.max_slippage_bps <= 0.0
            || self.max_feed_age_ms == 0
            || !self.min_state_quality.is_finite()
            || !(0.0..=1.0).contains(&self.min_state_quality)
        {
            return Err("risk policy is incomplete or violates hard policy limits");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct RiskContext {
    pub equity: Option<f64>,
    pub entry_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub proposed_notional: Option<f64>,
    pub gross_exposure: Option<f64>,
    pub correlated_exposure: Option<f64>,
    pub open_positions: Option<usize>,
    /// Losses and drawdown are non-negative fractions of equity/peak equity.
    pub daily_loss_fraction: Option<f64>,
    pub weekly_loss_fraction: Option<f64>,
    pub drawdown_fraction: Option<f64>,
    pub spread_bps: Option<f64>,
    pub estimated_slippage_bps: Option<f64>,
    pub feed_age_ms: Option<u64>,
    pub state_quality: Option<f64>,
    pub sequence_gap: bool,
    pub duplicate_or_out_of_order: bool,
    pub kill_switch: bool,
    pub protective_stop_confirmed: bool,
    pub exchange_reconciled: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RiskDecision {
    pub allowed: bool,
    pub reason: &'static str,
    pub estimated_trade_risk_fraction: Option<f64>,
    pub resulting_leverage: Option<f64>,
}

fn finite_positive(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > 0.0)
}

pub fn evaluate_entry(limits: &RiskLimits, context: &RiskContext) -> RiskDecision {
    let veto = |reason| RiskDecision {
        allowed: false, reason, estimated_trade_risk_fraction: None, resulting_leverage: None,
    };
    if context.kill_switch { return veto("global_kill_switch"); }
    if context.sequence_gap { return veto("feed_sequence_gap"); }
    if context.duplicate_or_out_of_order { return veto("duplicate_or_out_of_order_event"); }
    if limits.validate().is_err() { return veto("risk_policy_incomplete"); }
    if !context.protective_stop_confirmed { return veto("protective_stop_not_confirmed"); }
    if !context.exchange_reconciled { return veto("exchange_state_not_reconciled"); }

    let equity = match finite_positive(context.equity) { Some(v) => v, None => return veto("equity_unavailable") };
    let entry = match finite_positive(context.entry_price) { Some(v) => v, None => return veto("entry_price_unavailable") };
    let stop = match finite_positive(context.stop_price) { Some(v) if v != entry => v, _ => return veto("valid_stop_required") };
    let notional = match finite_positive(context.proposed_notional) { Some(v) => v, None => return veto("proposed_notional_unavailable") };
    let gross = match context.gross_exposure.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("gross_exposure_unavailable") };
    let correlated = match context.correlated_exposure.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("correlated_exposure_unavailable") };
    let positions = match context.open_positions { Some(v) => v, None => return veto("position_count_unavailable") };
    let daily_loss = match context.daily_loss_fraction.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("daily_loss_unavailable") };
    let weekly_loss = match context.weekly_loss_fraction.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("weekly_loss_unavailable") };
    let drawdown = match context.drawdown_fraction.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("drawdown_unavailable") };
    let spread = match context.spread_bps.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("spread_unavailable") };
    let slippage = match context.estimated_slippage_bps.filter(|v| v.is_finite() && *v >= 0.0) { Some(v) => v, None => return veto("slippage_estimate_unavailable") };
    let age = match context.feed_age_ms { Some(v) => v, None => return veto("feed_age_unavailable") };
    let quality = match context.state_quality.filter(|v| v.is_finite() && (0.0..=1.0).contains(v)) { Some(v) => v, None => return veto("state_quality_unavailable") };

    if positions >= limits.max_positions { return veto("position_count_cap"); }
    if correlated + notional > equity * limits.max_correlated_exposure_fraction { return veto("correlated_exposure_cap"); }
    if daily_loss >= limits.max_daily_loss_fraction { return veto("daily_loss_limit"); }
    if weekly_loss >= limits.max_weekly_loss_fraction { return veto("weekly_loss_limit"); }
    if drawdown >= limits.max_drawdown_fraction { return veto("drawdown_circuit_breaker"); }
    if spread > limits.max_spread_bps { return veto("spread_cap"); }
    if slippage > limits.max_slippage_bps { return veto("slippage_cap"); }
    if age > limits.max_feed_age_ms { return veto("stale_feed"); }
    if quality < limits.min_state_quality { return veto("low_state_quality"); }

    let trade_risk_fraction = notional * ((entry - stop).abs() / entry) / equity;
    if !trade_risk_fraction.is_finite() || trade_risk_fraction > limits.risk_per_trade_fraction {
        return veto("per_trade_risk_cap");
    }
    let resulting_leverage = (gross + notional) / equity;
    if !resulting_leverage.is_finite() || resulting_leverage > limits.max_leverage {
        return veto("leverage_cap");
    }
    RiskDecision {
        allowed: true, reason: "all_independent_risk_checks_passed",
        estimated_trade_risk_fraction: Some(trade_risk_fraction),
        resulting_leverage: Some(resulting_leverage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_limits() -> RiskLimits {
        RiskLimits {
            risk_per_trade_fraction: 0.0025, max_leverage: 2.0, max_positions: 3,
            max_correlated_exposure_fraction: 0.5, max_daily_loss_fraction: 0.02,
            max_weekly_loss_fraction: 0.05, max_drawdown_fraction: 0.10,
            max_spread_bps: 10.0, max_slippage_bps: 10.0, max_feed_age_ms: 5_000,
            min_state_quality: 0.5,
        }
    }
    fn safe_context() -> RiskContext {
        RiskContext {
            equity: Some(10_000.0), entry_price: Some(100.0), stop_price: Some(99.0),
            proposed_notional: Some(2_000.0), gross_exposure: Some(1_000.0),
            correlated_exposure: Some(0.0), open_positions: Some(0),
            daily_loss_fraction: Some(0.0), weekly_loss_fraction: Some(0.0),
            drawdown_fraction: Some(0.0), spread_bps: Some(1.0),
            estimated_slippage_bps: Some(1.0), feed_age_ms: Some(100),
            state_quality: Some(0.9), sequence_gap: false, duplicate_or_out_of_order: false,
            kill_switch: false, protective_stop_confirmed: true, exchange_reconciled: true,
        }
    }
    #[test]
    fn incomplete_policy_fails_closed() {
        let result = evaluate_entry(&RiskLimits::target_policy(), &safe_context());
        assert!(!result.allowed);
        assert_eq!(result.reason, "risk_policy_incomplete");
    }
    #[test]
    fn safe_complete_context_passes_only_the_risk_checks() {
        let result = evaluate_entry(&complete_limits(), &safe_context());
        assert!(result.allowed);
        assert!(result.estimated_trade_risk_fraction.unwrap() <= 0.0025);
        assert!(result.resulting_leverage.unwrap() <= 2.0);
    }
    #[test]
    fn kill_switch_vetoes_even_a_valid_candidate() {
        let mut context = safe_context(); context.kill_switch = true;
        assert_eq!(evaluate_entry(&complete_limits(), &context).reason, "global_kill_switch");
    }
    #[test]
    fn missing_protective_stop_vetoes_entry() {
        let mut context = safe_context(); context.protective_stop_confirmed = false;
        assert_eq!(evaluate_entry(&complete_limits(), &context).reason, "protective_stop_not_confirmed");
    }
    #[test]
    fn per_trade_risk_is_calculated_from_stop_distance() {
        let mut context = safe_context(); context.stop_price = Some(90.0);
        assert_eq!(evaluate_entry(&complete_limits(), &context).reason, "per_trade_risk_cap");
    }
    #[test]
    fn leverage_cap_vetoes_entry() {
        let mut context = safe_context(); context.gross_exposure = Some(20_000.0);
        assert_eq!(evaluate_entry(&complete_limits(), &context).reason, "leverage_cap");
    }
    #[test]
    fn stale_feed_vetoes_entry() {
        let mut context = safe_context(); context.feed_age_ms = Some(5_001);
        assert_eq!(evaluate_entry(&complete_limits(), &context).reason, "stale_feed");
    }
}
