//! Fail-closed pre-trade risk validation primitives.
//!
//! This module deliberately does not submit orders or access credentials. It is
//! the deterministic policy boundary that an execution adapter must call before
//! every new order and retry. Live execution remains disabled until a separately
//! reviewed adapter, durable idempotency store, reconciliation loop and kill
//! switch are connected.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingMode {
    Disabled,
    Paper,
    Testnet,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct RiskPolicy {
    pub max_risk_fraction: f64,
    pub max_leverage: f64,
    pub max_daily_loss_fraction: f64,
    pub max_positions: usize,
    pub max_market_data_age_ms: i64,
    pub max_order_notional_fraction: f64,
}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            max_risk_fraction: 0.0025,
            max_leverage: 2.0,
            max_daily_loss_fraction: 0.02,
            max_positions: 5,
            max_market_data_age_ms: 5_000,
            max_order_notional_fraction: 0.25,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderIntent {
    pub client_order_id: String,
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub reference_price: f64,
    pub stop_price: f64,
    pub equity: f64,
    pub current_gross_exposure: f64,
    pub current_daily_loss: f64,
    pub open_positions: usize,
    pub market_data_age_ms: i64,
    pub exchange_protective_stop_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    TradingDisabled,
    InvalidIdentity,
    InvalidPolicy,
    InvalidNumericInput,
    InvalidStopDirection,
    RiskBudgetExceeded,
    OrderNotionalExceeded,
    LeverageExceeded,
    DailyLossLimitReached,
    PositionLimitReached,
    StaleMarketData,
    ProtectiveStopUnavailable,
    LiveModeNotApproved,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApprovedOrder {
    pub client_order_id: String,
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub reference_price: f64,
    pub stop_price: f64,
    pub notional: f64,
    pub maximum_loss_at_stop: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LiveApproval {
    /// Must be set only by deployment configuration after an independent review.
    pub human_approved: bool,
    pub exchange_account_verified: bool,
    pub withdrawals_disabled: bool,
    pub kill_switch_healthy: bool,
    pub reconciliation_healthy: bool,
}

impl LiveApproval {
    fn permits_live(&self) -> bool {
        self.human_approved
            && self.exchange_account_verified
            && self.withdrawals_disabled
            && self.kill_switch_healthy
            && self.reconciliation_healthy
    }
}

/// Validate an order before any adapter call. All failure conditions reject.
pub fn validate_order(
    mode: TradingMode,
    policy: &RiskPolicy,
    approval: &LiveApproval,
    intent: &OrderIntent,
) -> Result<ApprovedOrder, Rejection> {
    if mode == TradingMode::Disabled {
        return Err(Rejection::TradingDisabled);
    }
    if mode == TradingMode::Live && !approval.permits_live() {
        return Err(Rejection::LiveModeNotApproved);
    }
    // Reject malformed policy at the boundary. A misconfigured fraction above
    // 100%, negative freshness window, or zero position cap must never widen
    // the effective risk envelope silently.
    if !policy.max_risk_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.max_risk_fraction)
        || !policy.max_leverage.is_finite()
        || policy.max_leverage <= 0.0
        || !policy.max_daily_loss_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.max_daily_loss_fraction)
        || !policy.max_order_notional_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.max_order_notional_fraction)
        || policy.max_positions == 0
        || policy.max_market_data_age_ms < 0
    {
        return Err(Rejection::InvalidPolicy);
    }
    if intent.client_order_id.trim().is_empty()
        || intent.client_order_id.len() > 128
        || intent.symbol.trim().is_empty()
        || intent.symbol.len() > 64
    {
        return Err(Rejection::InvalidIdentity);
    }

    let numbers = [
        policy.max_risk_fraction,
        policy.max_leverage,
        policy.max_daily_loss_fraction,
        policy.max_order_notional_fraction,
        intent.quantity,
        intent.reference_price,
        intent.stop_price,
        intent.equity,
        intent.current_gross_exposure,
        intent.current_daily_loss,
    ];
    if numbers.iter().any(|v| !v.is_finite() || *v < 0.0)
        || policy.max_risk_fraction <= 0.0
        || policy.max_leverage <= 0.0
        || policy.max_daily_loss_fraction <= 0.0
        || policy.max_order_notional_fraction <= 0.0
        || intent.quantity <= 0.0
        || intent.reference_price <= 0.0
        || intent.equity <= 0.0
    {
        return Err(Rejection::InvalidNumericInput);
    }

    let stop_distance = match intent.side {
        Side::Buy if intent.stop_price < intent.reference_price =>
            intent.reference_price - intent.stop_price,
        Side::Sell if intent.stop_price > intent.reference_price =>
            intent.stop_price - intent.reference_price,
        _ => return Err(Rejection::InvalidStopDirection),
    };
    if stop_distance <= 0.0 {
        return Err(Rejection::InvalidStopDirection);
    }

    let notional = intent.quantity * intent.reference_price;
    let maximum_loss_at_stop = intent.quantity * stop_distance;
    if !notional.is_finite() || !maximum_loss_at_stop.is_finite() {
        return Err(Rejection::InvalidNumericInput);
    }
    if maximum_loss_at_stop > intent.equity * policy.max_risk_fraction {
        return Err(Rejection::RiskBudgetExceeded);
    }
    if notional > intent.equity * policy.max_order_notional_fraction {
        return Err(Rejection::OrderNotionalExceeded);
    }
    if intent.current_gross_exposure + notional > intent.equity * policy.max_leverage {
        return Err(Rejection::LeverageExceeded);
    }
    if intent.current_daily_loss >= intent.equity * policy.max_daily_loss_fraction {
        return Err(Rejection::DailyLossLimitReached);
    }
    if intent.open_positions >= policy.max_positions {
        return Err(Rejection::PositionLimitReached);
    }
    if intent.market_data_age_ms < 0
        || intent.market_data_age_ms > policy.max_market_data_age_ms
    {
        return Err(Rejection::StaleMarketData);
    }
    if !intent.exchange_protective_stop_supported {
        return Err(Rejection::ProtectiveStopUnavailable);
    }

    Ok(ApprovedOrder {
        client_order_id: intent.client_order_id.clone(),
        symbol: intent.symbol.clone(),
        side: intent.side,
        quantity: intent.quantity,
        reference_price: intent.reference_price,
        stop_price: intent.stop_price,
        notional,
        maximum_loss_at_stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent() -> OrderIntent {
        OrderIntent {
            client_order_id: "qoin-test-0001".into(),
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.001,
            reference_price: 60_000.0,
            stop_price: 59_900.0,
            equity: 10_000.0,
            current_gross_exposure: 0.0,
            current_daily_loss: 0.0,
            open_positions: 0,
            market_data_age_ms: 100,
            exchange_protective_stop_supported: true,
        }
    }

    #[test]
    fn disabled_mode_never_approves() {
        assert_eq!(
            validate_order(TradingMode::Disabled, &RiskPolicy::default(), &LiveApproval::default(), &intent()),
            Err(Rejection::TradingDisabled)
        );
    }

    #[test]
    fn paper_order_within_all_limits_is_approved() {
        let approved = validate_order(
            TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &intent()
        ).unwrap();
        assert_eq!(approved.notional, 60.0);
        assert_eq!(approved.maximum_loss_at_stop, 0.1);
    }

    #[test]
    fn live_mode_requires_every_operational_approval() {
        let mut approval = LiveApproval::default();
        assert_eq!(
            validate_order(TradingMode::Live, &RiskPolicy::default(), &approval, &intent()),
            Err(Rejection::LiveModeNotApproved)
        );
        approval.human_approved = true;
        approval.exchange_account_verified = true;
        approval.withdrawals_disabled = true;
        approval.kill_switch_healthy = true;
        approval.reconciliation_healthy = true;
        assert!(validate_order(TradingMode::Live, &RiskPolicy::default(), &approval, &intent()).is_ok());
    }

    #[test]
    fn rejects_unsafe_policy_configuration() {
        let mut policy = RiskPolicy::default();
        policy.max_risk_fraction = 1.5;
        assert_eq!(
            validate_order(TradingMode::Paper, &policy, &LiveApproval::default(), &intent()),
            Err(Rejection::InvalidPolicy)
        );
        policy = RiskPolicy::default();
        policy.max_market_data_age_ms = -1;
        assert_eq!(
            validate_order(TradingMode::Paper, &policy, &LiveApproval::default(), &intent()),
            Err(Rejection::InvalidPolicy)
        );
        policy = RiskPolicy::default();
        policy.max_positions = 0;
        assert_eq!(
            validate_order(TradingMode::Paper, &policy, &LiveApproval::default(), &intent()),
            Err(Rejection::InvalidPolicy)
        );
    }

    #[test]
    fn rejects_risk_budget_overrun() {
        let mut order = intent();
        order.quantity = 1.0;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::RiskBudgetExceeded)
        );
    }

    #[test]
    fn rejects_wrong_side_stop() {
        let mut order = intent();
        order.stop_price = 60_100.0;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::InvalidStopDirection)
        );
    }

    #[test]
    fn rejects_stale_data_and_missing_protective_stop() {
        let mut order = intent();
        order.market_data_age_ms = 5_001;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::StaleMarketData)
        );
        order.market_data_age_ms = 100;
        order.exchange_protective_stop_supported = false;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::ProtectiveStopUnavailable)
        );
    }

    #[test]
    fn rejects_daily_loss_and_position_caps() {
        let mut order = intent();
        order.current_daily_loss = 200.0;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::DailyLossLimitReached)
        );
        order.current_daily_loss = 0.0;
        order.open_positions = 5;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::PositionLimitReached)
        );
    }

    #[test]
    fn rejects_non_finite_and_duplicate_identity_inputs() {
        let mut order = intent();
        order.quantity = f64::NAN;
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::InvalidNumericInput)
        );
        order = intent();
        order.client_order_id.clear();
        assert_eq!(
            validate_order(TradingMode::Paper, &RiskPolicy::default(), &LiveApproval::default(), &order),
            Err(Rejection::InvalidIdentity)
        );
    }
}
