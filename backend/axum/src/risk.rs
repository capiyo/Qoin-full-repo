//! Independent, fail-closed portfolio risk gate.
//! This module only evaluates an entry proposal. It never submits orders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiskLimits {
    pub max_equity_risk_fraction: f64,
    pub max_leverage: f64,
    pub max_daily_loss_fraction: f64,
    pub max_weekly_loss_fraction: f64,
    pub max_drawdown_fraction: f64,
    pub max_spread_bps: f64,
    pub max_feed_age_ms: i64,
    pub max_positions: usize,
    pub max_correlated_exposure_fraction: f64,
}
impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_equity_risk_fraction: 0.0025,
            max_leverage: 2.0,
            max_daily_loss_fraction: 0.02,
            max_weekly_loss_fraction: 0.05,
            max_drawdown_fraction: 0.10,
            max_spread_bps: 10.0,
            max_feed_age_ms: 2_000,
            max_positions: 3,
            max_correlated_exposure_fraction: 0.50,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct EntryRiskContext {
    pub equity: f64,
    pub entry_price: f64,
    pub stop_price: f64,
    pub notional: f64,
    pub leverage: f64,
    pub daily_loss_fraction: f64,
    pub weekly_loss_fraction: f64,
    pub drawdown_fraction: f64,
    pub spread_bps: f64,
    pub feed_age_ms: i64,
    pub open_positions: usize,
    pub correlated_exposure_fraction: f64,
    pub kill_switch: bool,
    pub sequence_gap: bool,
    pub duplicate_or_out_of_order: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskVeto {
    KillSwitch,
    InvalidOrMissingContext,
    FeedStale,
    SequenceGap,
    DuplicateOrOutOfOrder,
    SpreadTooWide,
    PerTradeRiskExceeded,
    LeverageExceeded,
    DailyLossLimit,
    WeeklyLossLimit,
    DrawdownCircuitBreaker,
    PositionCountExceeded,
    CorrelatedExposureExceeded,
}
pub fn evaluate_entry(ctx: Option<&EntryRiskContext>, limits: RiskLimits) -> Result<(), RiskVeto> {
    let c = ctx.ok_or(RiskVeto::InvalidOrMissingContext)?;
    if c.kill_switch { return Err(RiskVeto::KillSwitch); }
    if c.sequence_gap { return Err(RiskVeto::SequenceGap); }
    if c.duplicate_or_out_of_order { return Err(RiskVeto::DuplicateOrOutOfOrder); }
    let values = [c.equity, c.entry_price, c.stop_price, c.notional, c.leverage,
        c.daily_loss_fraction, c.weekly_loss_fraction, c.drawdown_fraction,
        c.spread_bps, c.correlated_exposure_fraction];
    if values.iter().any(|v| !v.is_finite()) || c.equity <= 0.0 || c.entry_price <= 0.0
        || c.stop_price <= 0.0 || c.notional <= 0.0 || c.leverage <= 0.0
        || c.stop_price == c.entry_price || c.feed_age_ms < 0 {
        return Err(RiskVeto::InvalidOrMissingContext);
    }
    if c.feed_age_ms > limits.max_feed_age_ms { return Err(RiskVeto::FeedStale); }
    if c.spread_bps < 0.0 || c.spread_bps > limits.max_spread_bps { return Err(RiskVeto::SpreadTooWide); }
    let risk_fraction = ((c.entry_price - c.stop_price).abs() / c.entry_price) * (c.notional / c.equity);
    if risk_fraction > limits.max_equity_risk_fraction { return Err(RiskVeto::PerTradeRiskExceeded); }
    if c.leverage > limits.max_leverage || c.notional / c.equity > limits.max_leverage {
        return Err(RiskVeto::LeverageExceeded);
    }
    if c.daily_loss_fraction >= limits.max_daily_loss_fraction { return Err(RiskVeto::DailyLossLimit); }
    if c.weekly_loss_fraction >= limits.max_weekly_loss_fraction { return Err(RiskVeto::WeeklyLossLimit); }
    if c.drawdown_fraction >= limits.max_drawdown_fraction { return Err(RiskVeto::DrawdownCircuitBreaker); }
    if c.open_positions >= limits.max_positions { return Err(RiskVeto::PositionCountExceeded); }
    if c.correlated_exposure_fraction > limits.max_correlated_exposure_fraction {
        return Err(RiskVeto::CorrelatedExposureExceeded);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn safe() -> EntryRiskContext {
        EntryRiskContext { equity: 10_000.0, entry_price: 100.0, stop_price: 99.0,
            notional: 2_000.0, leverage: 1.0, daily_loss_fraction: 0.0,
            weekly_loss_fraction: 0.0, drawdown_fraction: 0.0, spread_bps: 2.0,
            feed_age_ms: 100, open_positions: 0, correlated_exposure_fraction: 0.1,
            kill_switch: false, sequence_gap: false, duplicate_or_out_of_order: false }
    }
    #[test] fn missing_context_fails_closed() {
        assert_eq!(evaluate_entry(None, RiskLimits::default()), Err(RiskVeto::InvalidOrMissingContext));
    }
    #[test] fn kill_switch_vetoes() {
        let mut c = safe(); c.kill_switch = true;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::KillSwitch));
    }
    #[test] fn stale_feed_vetoes() {
        let mut c = safe(); c.feed_age_ms = 2_001;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::FeedStale));
    }
    #[test] fn oversized_stop_risk_vetoes() {
        let mut c = safe(); c.stop_price = 90.0;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::PerTradeRiskExceeded));
    }
    #[test] fn leverage_vetoes() {
        let mut c = safe(); c.leverage = 3.0;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::LeverageExceeded));
    }
    #[test] fn loss_limits_veto() {
        let mut c = safe(); c.daily_loss_fraction = 0.02;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::DailyLossLimit));
    }
    #[test] fn position_count_vetoes() {
        let mut c = safe(); c.open_positions = 3;
        assert_eq!(evaluate_entry(Some(&c), RiskLimits::default()), Err(RiskVeto::PositionCountExceeded));
    }
}
