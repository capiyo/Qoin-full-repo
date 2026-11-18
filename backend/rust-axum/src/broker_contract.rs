//! Broker-neutral execution and instrument contract.
//!
//! Shared engines and risk policy operate on normalized instruments and
//! account-currency values. Provider-specific symbols, credentials, endpoints,
//! order flags and precision rules belong inside an adapter implementation.
//! No asset class, quote currency, venue or broker is privileged here.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, sync::{Arc, RwLock}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Forex,
    Crypto,
    Equity,
    Etf,
    Commodity,
    Index,
    FixedIncome,
    Derivative,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantityUnit {
    BaseUnits,
    Lots,
    Shares,
    Contracts,
    Units,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide { Buy, Sell }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType { Market, Limit, Stop, StopLimit }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentSpec {
    /// Stable Qoin instrument key, independent of broker ticker syntax.
    pub instrument_id: String,
    pub broker_id: String,
    pub venue_id: String,
    pub broker_symbol: String,
    pub asset_class: AssetClass,
    pub base_currency: Option<String>,
    pub quote_currency: String,
    pub settlement_currency: String,
    pub quantity_unit: QuantityUnit,
    /// Converts one native quantity unit into base exposure (or contract value).
    pub contract_multiplier: f64,
    pub tick_size: f64,
    pub quantity_step: f64,
    pub minimum_quantity: f64,
    pub minimum_notional: Option<f64>,
    pub price_currency_to_account_rate: f64,
    pub conversion_rate_observed_at_ms: i64,
}

impl InstrumentSpec {
    pub fn validate(&self) -> Result<(), ContractError> {
        for (name, value) in [
            ("contract_multiplier", self.contract_multiplier),
            ("tick_size", self.tick_size),
            ("quantity_step", self.quantity_step),
            ("minimum_quantity", self.minimum_quantity),
            ("price_currency_to_account_rate", self.price_currency_to_account_rate),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ContractError::InvalidInstrument(format!("{name} must be finite and positive")));
            }
        }
        if self.conversion_rate_observed_at_ms <= 0 {
            return Err(ContractError::InvalidInstrument("currency conversion observation time is required".into()));
        }
        if self.instrument_id.trim().is_empty()
            || self.broker_id.trim().is_empty()
            || self.venue_id.trim().is_empty()
            || self.broker_symbol.trim().is_empty()
            || self.quote_currency.trim().is_empty()
            || self.settlement_currency.trim().is_empty()
        {
            return Err(ContractError::InvalidInstrument("instrument identity and currencies are required".into()));
        }
        if self.minimum_notional.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return Err(ContractError::InvalidInstrument("minimum_notional must be finite and non-negative".into()));
        }
        Ok(())
    }

    pub fn notional_in_account_currency(&self, quantity: f64, price: f64) -> Result<f64, ContractError> {
        self.validate()?;
        if !quantity.is_finite() || quantity <= 0.0 || !price.is_finite() || price <= 0.0 {
            return Err(ContractError::InvalidOrder("quantity and price must be finite and positive".into()));
        }
        let notional = quantity * self.contract_multiplier * price * self.price_currency_to_account_rate;
        if !notional.is_finite() || notional <= 0.0 {
            return Err(ContractError::InvalidOrder("converted notional is invalid".into()));
        }
        Ok(notional)
    }

    pub fn validate_quantity(&self, quantity: f64) -> Result<(), ContractError> {
        self.validate()?;
        if !quantity.is_finite() || quantity < self.minimum_quantity {
            return Err(ContractError::InvalidOrder("quantity is below the instrument minimum or invalid".into()));
        }
        let steps = quantity / self.quantity_step;
        if !steps.is_finite() || (steps - steps.round()).abs() > 1e-8 {
            return Err(ContractError::InvalidOrder("quantity does not conform to instrument step size".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerAccountSnapshot {
    pub broker_id: String,
    pub account_id: String,
    pub account_currency: String,
    pub equity: f64,
    pub available_margin: f64,
    pub gross_exposure: f64,
    pub daily_realized_pnl: f64,
    pub daily_unrealized_pnl: f64,
    pub open_positions: usize,
    pub observed_at_ms: i64,
}

impl BrokerAccountSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.broker_id.trim().is_empty() || self.account_id.trim().is_empty()
            || self.account_currency.trim().is_empty() || self.observed_at_ms <= 0
        {
            return Err(ContractError::InvalidAccount("account identity, currency and observation time are required".into()));
        }
        let values = [self.equity, self.available_margin, self.gross_exposure,
            self.daily_realized_pnl, self.daily_unrealized_pnl];
        if values.iter().any(|value| !value.is_finite())
            || self.equity <= 0.0 || self.available_margin < 0.0 || self.gross_exposure < 0.0
        {
            return Err(ContractError::InvalidAccount("account snapshot contains invalid financial values".into()));
        }
        Ok(())
    }

    pub fn current_daily_loss(&self) -> Result<f64, ContractError> {
        self.validate()?;
        let pnl = self.daily_realized_pnl + self.daily_unrealized_pnl;
        if !pnl.is_finite() {
            return Err(ContractError::InvalidAccount("daily P&L overflow".into()));
        }
        Ok((-pnl).max(0.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedOrder {
    pub client_order_id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub time_in_force: Option<String>,
    pub reduce_only: bool,
}

impl NormalizedOrder {
    pub fn validate(&self, instrument: &InstrumentSpec) -> Result<(), ContractError> {
        if self.client_order_id.trim().is_empty() || self.client_order_id.len() > 128
            || self.account_id.trim().is_empty() || self.instrument_id != instrument.instrument_id
        {
            return Err(ContractError::InvalidOrder("order identity does not match selected account and instrument".into()));
        }
        instrument.validate_quantity(self.quantity)?;
        match self.order_type {
            OrderType::Limit if !self.limit_price.is_some_and(|value| value.is_finite() && value > 0.0) =>
                return Err(ContractError::InvalidOrder("limit orders require a positive limit price".into())),
            OrderType::Stop if !self.stop_price.is_some_and(|value| value.is_finite() && value > 0.0) =>
                return Err(ContractError::InvalidOrder("stop orders require a positive stop price".into())),
            OrderType::StopLimit if !self.stop_price.is_some_and(|value| value.is_finite() && value > 0.0)
                || !self.limit_price.is_some_and(|value| value.is_finite() && value > 0.0) =>
                return Err(ContractError::InvalidOrder("stop-limit orders require positive stop and limit prices".into())),
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerCapabilities {
    pub market_orders: bool,
    pub limit_orders: bool,
    pub stop_orders: bool,
    pub stop_limit_orders: bool,
    pub native_protective_orders: bool,
    pub amend_orders: bool,
    pub cancel_orders: bool,
    pub fractional_quantity: bool,
    pub streaming_execution_reports: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrokerOrderReceipt {
    pub broker_id: String,
    pub account_id: String,
    pub client_order_id: String,
    pub broker_order_id: String,
    pub status: String,
    pub accepted_at_ms: i64,
    pub raw_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerExecutionReport {
    pub broker_id: String,
    pub account_id: String,
    pub broker_order_id: String,
    pub execution_id: String,
    pub cumulative_quantity: f64,
    pub last_fill_quantity: f64,
    pub last_fill_price: f64,
    pub fee_amount: Option<f64>,
    pub fee_currency: Option<String>,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    InvalidInstrument(String),
    InvalidAccount(String),
    InvalidOrder(String),
    Broker(String),
    Unsupported(String),
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInstrument(message) => write!(formatter, "invalid instrument: {message}"),
            Self::InvalidAccount(message) => write!(formatter, "invalid broker account: {message}"),
            Self::InvalidOrder(message) => write!(formatter, "invalid normalized order: {message}"),
            Self::Broker(message) => write!(formatter, "broker adapter error: {message}"),
            Self::Unsupported(message) => write!(formatter, "unsupported broker capability: {message}"),
        }
    }
}

impl std::error::Error for ContractError {}

#[derive(Debug, Clone)]
pub struct PortfolioRiskPolicy {
    pub max_risk_fraction: f64,
    pub max_order_notional_fraction: f64,
    pub max_gross_leverage: f64,
    pub max_daily_loss_fraction: f64,
    pub max_open_positions: usize,
    pub max_market_data_age_ms: i64,
}
impl Default for PortfolioRiskPolicy {
    fn default() -> Self { Self { max_risk_fraction: 0.0025, max_order_notional_fraction: 0.25,
        max_gross_leverage: 2.0, max_daily_loss_fraction: 0.02, max_open_positions: 5,
        max_market_data_age_ms: 5_000 } }
}
#[derive(Debug, Clone)]
pub struct RiskCheckInput {
    pub reference_price: f64,
    pub protective_stop_price: Option<f64>,
    pub now_ms: i64,
    pub reference_price_observed_at_ms: i64,
    pub protective_order_supported: bool,
    /// Broker-calculated margin requirement, normalized to account currency.
    pub required_margin_account_currency: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RiskApproval {
    pub notional_account_currency: f64,
    pub maximum_loss_account_currency: f64,
    pub remaining_daily_loss_budget: f64,
    pub projected_gross_exposure: f64,
}
/// Shared pre-trade gate. All amounts are normalized to account currency;
/// no USD/USDT assumption is made.
pub fn validate_portfolio_order(policy: &PortfolioRiskPolicy, account: &BrokerAccountSnapshot,
    instrument: &InstrumentSpec, order: &NormalizedOrder, input: &RiskCheckInput)
    -> Result<RiskApproval, ContractError> {
    account.validate()?;
    order.validate(instrument)?;
    if account.broker_id != instrument.broker_id || order.account_id != account.account_id {
        return Err(ContractError::InvalidOrder("broker/account does not match selected instrument".into()));
    }
    let fractions = [policy.max_risk_fraction, policy.max_order_notional_fraction, policy.max_daily_loss_fraction];
    if fractions.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 1.0)
        || !policy.max_gross_leverage.is_finite() || policy.max_gross_leverage <= 0.0
        || policy.max_open_positions == 0 || policy.max_market_data_age_ms < 0 {
        return Err(ContractError::InvalidOrder("portfolio risk policy is invalid".into()));
    }
    let account_age_ms = input.now_ms.saturating_sub(account.observed_at_ms);
    let quote_age_ms = input.now_ms.saturating_sub(input.reference_price_observed_at_ms);
    let conversion_age_ms = input.now_ms.saturating_sub(instrument.conversion_rate_observed_at_ms);
    if input.now_ms <= 0 || input.reference_price_observed_at_ms <= 0
        || account.observed_at_ms > input.now_ms || input.reference_price_observed_at_ms > input.now_ms
        || instrument.conversion_rate_observed_at_ms > input.now_ms
        || account_age_ms < 0 || account_age_ms > policy.max_market_data_age_ms
        || !input.reference_price.is_finite() || input.reference_price <= 0.0
        || quote_age_ms < 0 || quote_age_ms > policy.max_market_data_age_ms
        || conversion_age_ms < 0 || conversion_age_ms > policy.max_market_data_age_ms
        || !input.required_margin_account_currency.is_finite() || input.required_margin_account_currency < 0.0 {
        return Err(ContractError::InvalidOrder("market price is invalid or stale".into()));
    }
    if !input.protective_order_supported && !order.reduce_only {
        return Err(ContractError::Unsupported("selected broker/instrument cannot place a protective order".into()));
    }
    if instrument.quote_currency.eq_ignore_ascii_case(&account.account_currency)
        && (instrument.price_currency_to_account_rate - 1.0).abs() > 1e-8 {
        return Err(ContractError::InvalidInstrument("same-currency conversion rate must equal one".into()));
    }
    let execution_price = order.limit_price.unwrap_or(input.reference_price);
    let notional = instrument.notional_in_account_currency(order.quantity, execution_price)?;
    let projected_gross_exposure = account.gross_exposure + notional;
    let notional_cap = account.equity * policy.max_order_notional_fraction;
    let leverage_cap = account.equity * policy.max_gross_leverage;
    if !projected_gross_exposure.is_finite() || !notional_cap.is_finite() || !leverage_cap.is_finite() {
        return Err(ContractError::InvalidOrder("portfolio exposure calculation overflow".into()));
    }
    if instrument.minimum_notional.is_some_and(|minimum| order.quantity * instrument.contract_multiplier * execution_price < minimum) {
        return Err(ContractError::InvalidOrder("order is below instrument minimum notional".into()));
    }
    if notional > notional_cap { return Err(ContractError::InvalidOrder("order exceeds account-currency notional budget".into())); }
    if input.required_margin_account_currency > account.available_margin {
        return Err(ContractError::InvalidOrder("insufficient available margin in broker account".into()));
    }
    if projected_gross_exposure > leverage_cap { return Err(ContractError::InvalidOrder("projected gross exposure exceeds leverage limit".into())); }
    if account.open_positions >= policy.max_open_positions && !order.reduce_only {
        return Err(ContractError::InvalidOrder("open-position limit reached".into()));
    }
    let daily_loss = account.current_daily_loss()?;
    let daily_cap = account.equity * policy.max_daily_loss_fraction;
    if daily_loss >= daily_cap { return Err(ContractError::InvalidOrder("daily loss limit reached".into())); }
    let remaining_daily_loss_budget = (daily_cap - daily_loss).max(0.0);
    let maximum_loss = if order.reduce_only { 0.0 } else {
        let stop = input.protective_stop_price.ok_or_else(|| ContractError::Unsupported(
            "opening orders require a broker-supported protective stop".into()))?;
        let valid = match order.side {
            OrderSide::Buy => stop.is_finite() && stop > 0.0 && stop < execution_price,
            OrderSide::Sell => stop.is_finite() && stop > execution_price,
        };
        if !valid { return Err(ContractError::InvalidOrder("protective stop is invalid for order direction".into())); }
        let loss = instrument.notional_in_account_currency(order.quantity, (execution_price-stop).abs())?;
        if loss > account.equity * policy.max_risk_fraction {
            return Err(ContractError::InvalidOrder("maximum loss at stop exceeds risk budget".into()));
        }
        loss
    };
    Ok(RiskApproval { notional_account_currency:notional, maximum_loss_account_currency:maximum_loss,
        remaining_daily_loss_budget, projected_gross_exposure })
}

#[derive(Debug, Clone)]
pub struct ExecutionPreflight {
    pub reference_price: f64,
    pub reference_price_observed_at_ms: i64,
    pub protective_stop_price: Option<f64>,
    pub now_ms: i64,
    /// The selected broker calculates margin using its own account mode,
    /// instrument rules and current portfolio state.
    pub required_margin_account_currency: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiskCheckedReceipt {
    pub risk: RiskApproval,
    pub broker_receipt: BrokerOrderReceipt,
}

fn validate_capabilities(order: &NormalizedOrder, capabilities: &BrokerCapabilities) -> Result<(), ContractError> {
    let supported = match order.order_type {
        OrderType::Market => capabilities.market_orders,
        OrderType::Limit => capabilities.limit_orders,
        OrderType::Stop => capabilities.stop_orders,
        OrderType::StopLimit => capabilities.stop_limit_orders,
    };
    if !supported {
        return Err(ContractError::Unsupported("selected broker does not support this order type".into()));
    }
    if !order.reduce_only && !capabilities.native_protective_orders {
        return Err(ContractError::Unsupported("selected broker cannot guarantee native protective orders".into()));
    }
    if !capabilities.cancel_orders {
        return Err(ContractError::Unsupported("selected broker does not support order cancellation".into()));
    }
    Ok(())
}

/// Shared adapter invocation sequence. Callers must persist the client order
/// intent before invoking this function and persist the returned receipt before
/// reporting success. Any timeout/ambiguous adapter error requires reconciliation,
/// never blind resubmission.
pub async fn submit_risk_checked(
    adapter: &dyn BrokerAdapter,
    policy: &PortfolioRiskPolicy,
    order: NormalizedOrder,
    preflight: ExecutionPreflight,
) -> Result<RiskCheckedReceipt, ContractError> {
    if adapter.broker_id().trim().is_empty() {
        return Err(ContractError::Broker("adapter has no broker identity".into()));
    }
    let account = adapter.account_snapshot(&order.account_id).await?;
    let instrument = adapter.instrument_spec(&order.account_id, &order.instrument_id).await?;
    if account.broker_id != adapter.broker_id() || instrument.broker_id != adapter.broker_id() {
        return Err(ContractError::InvalidAccount("adapter returned account or instrument for another broker".into()));
    }
    let capabilities = adapter.capabilities(&order.account_id, &order.instrument_id).await?;
    validate_capabilities(&order, &capabilities)?;
    let risk = validate_portfolio_order(policy, &account, &instrument, &order, &RiskCheckInput {
        reference_price: preflight.reference_price,
        protective_stop_price: preflight.protective_stop_price,
        now_ms: preflight.now_ms,
        reference_price_observed_at_ms: preflight.reference_price_observed_at_ms,
        protective_order_supported: capabilities.native_protective_orders,
        required_margin_account_currency: preflight.required_margin_account_currency,
    })?;
    let broker_receipt = adapter.submit_order(order.clone()).await?;
    if broker_receipt.broker_id != adapter.broker_id()
        || broker_receipt.account_id != order.account_id
        || broker_receipt.client_order_id != order.client_order_id
        || broker_receipt.broker_order_id.trim().is_empty()
        || broker_receipt.accepted_at_ms <= 0
        || broker_receipt.status.trim().is_empty()
    {
        return Err(ContractError::Broker("broker returned an invalid or mismatched acknowledgement; reconciliation is required".into()));
    }
    Ok(RiskCheckedReceipt { risk, broker_receipt })
}

/// Runtime broker registry. Frontend broker selection resolves by stable
/// broker_id; the selected adapter owns all provider-specific behavior.
#[derive(Clone, Default)]
pub struct BrokerRegistry {
    adapters: Arc<RwLock<HashMap<String, Arc<dyn BrokerAdapter>>>>,
}

impl BrokerRegistry {
    pub fn register(&self, adapter: Arc<dyn BrokerAdapter>) -> Result<(), ContractError> {
        let broker_id = adapter.broker_id().trim().to_ascii_lowercase();
        if broker_id.is_empty() {
            return Err(ContractError::Broker("cannot register adapter without broker identity".into()));
        }
        let mut adapters = self.adapters.write()
            .map_err(|_| ContractError::Broker("broker registry lock is poisoned".into()))?;
        if adapters.contains_key(&broker_id) {
            return Err(ContractError::Broker(format!("broker adapter {broker_id} is already registered")));
        }
        adapters.insert(broker_id, adapter);
        Ok(())
    }

    pub fn resolve(&self, broker_id: &str) -> Result<Arc<dyn BrokerAdapter>, ContractError> {
        let key = broker_id.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(ContractError::InvalidOrder("selected broker_id is required".into()));
        }
        self.adapters.read()
            .map_err(|_| ContractError::Broker("broker registry lock is poisoned".into()))?
            .get(&key).cloned()
            .ok_or_else(|| ContractError::Unsupported(format!("selected broker {key} is not configured on this deployment")))
    }

    pub fn configured_brokers(&self) -> Result<Vec<String>, ContractError> {
        let mut ids = self.adapters.read()
            .map_err(|_| ContractError::Broker("broker registry lock is poisoned".into()))?
            .keys().cloned().collect::<Vec<_>>();
        ids.sort();
        Ok(ids)
    }
}

/// Every broker implements the same account/order surface. Credentials and
/// broker-specific payload translation remain inside the concrete adapter.
#[async_trait]
pub trait BrokerAdapter: Send + Sync {
    fn broker_id(&self) -> &'static str;
    async fn capabilities(&self, account_id: &str, instrument_id: &str) -> Result<BrokerCapabilities, ContractError>;
    async fn account_snapshot(&self, account_id: &str) -> Result<BrokerAccountSnapshot, ContractError>;
    async fn instrument_spec(&self, account_id: &str, instrument_id: &str) -> Result<InstrumentSpec, ContractError>;
    async fn submit_order(&self, order: NormalizedOrder) -> Result<BrokerOrderReceipt, ContractError>;
    async fn order_status(&self, account_id: &str, broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError>;
    async fn cancel_order(&self, account_id: &str, broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instrument(asset_class: AssetClass, symbol: &str, multiplier: f64, rate: f64) -> InstrumentSpec {
        InstrumentSpec {
            instrument_id: format!("{}:{}", asset_class as u8, symbol),
            broker_id: "selected-broker".into(),
            venue_id: "selected-venue".into(),
            broker_symbol: symbol.into(),
            asset_class,
            base_currency: None,
            quote_currency: "EUR".into(),
            settlement_currency: "EUR".into(),
            quantity_unit: QuantityUnit::Units,
            contract_multiplier: multiplier,
            tick_size: 0.0001,
            quantity_step: 0.01,
            minimum_quantity: 0.01,
            minimum_notional: None,
            price_currency_to_account_rate: rate,
            conversion_rate_observed_at_ms: 10_000,
        }
    }

    #[test]
    fn supports_non_usdt_forex_equity_and_crypto_instruments() {
        let cases = [
            (instrument(AssetClass::Forex, "EURUSD", 100_000.0, 0.92), 0.01, 1.1),
            (instrument(AssetClass::Equity, "AAPL", 1.0, 0.92), 2.0, 190.0),
            (instrument(AssetClass::Crypto, "BTC-EUR", 1.0, 1.0), 0.01, 60_000.0),
        ];
        for (spec, quantity, price) in cases {
            let notional = spec.notional_in_account_currency(quantity, price).unwrap();
            assert!(notional.is_finite() && notional > 0.0);
        }
    }

    #[test]
    fn quantity_step_and_minimum_are_enforced_per_instrument() {
        let mut spec = instrument(AssetClass::Equity, "ABC", 1.0, 1.0);
        spec.quantity_step = 0.05;
        assert!(spec.validate_quantity(1.25).is_ok());
        assert!(spec.validate_quantity(1.23).is_err());
        assert!(spec.validate_quantity(0.001).is_err());
    }

    #[test]
    fn account_loss_uses_realized_and_unrealized_pnl() {
        let account = BrokerAccountSnapshot {
            broker_id: "broker".into(), account_id: "account".into(),
            account_currency: "KES".into(), equity: 100_000.0,
            available_margin: 50_000.0, gross_exposure: 20_000.0,
            daily_realized_pnl: -1_000.0, daily_unrealized_pnl: -500.0,
            open_positions: 2, observed_at_ms: 1,
        };
        assert_eq!(account.current_daily_loss().unwrap(), 1_500.0);
    }

    #[test]
    fn portfolio_risk_is_account_currency_and_asset_class_agnostic() {
        let mut spec = instrument(AssetClass::Forex, "EURUSD", 100_000.0, 0.92);
        spec.quote_currency = "USD".into();
        let account = BrokerAccountSnapshot { broker_id:"selected-broker".into(), account_id:"acct".into(),
            account_currency:"EUR".into(), equity:10_000.0, available_margin:5_000.0, gross_exposure:0.0,
            daily_realized_pnl:0.0, daily_unrealized_pnl:0.0, open_positions:0, observed_at_ms:10_000 };
        let order = NormalizedOrder { client_order_id:"fx-order-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Buy, order_type:OrderType::Market,
            quantity:0.01, limit_price:None, stop_price:Some(1.09), time_in_force:None, reduce_only:false };
        let approved = validate_portfolio_order(&PortfolioRiskPolicy::default(), &account, &spec, &order,
            &RiskCheckInput { reference_price:1.1, protective_stop_price:Some(1.09),
                now_ms:10_100, reference_price_observed_at_ms:10_000, protective_order_supported:true, required_margin_account_currency:100.0 }).unwrap();
        assert!((approved.notional_account_currency-1_012.0).abs()<1e-8);
        assert!(approved.maximum_loss_account_currency>0.0);
    }

    #[test]
    fn portfolio_risk_rejects_stale_quotes_and_missing_protection() {
        let spec = instrument(AssetClass::Equity, "ABC", 1.0, 1.0);
        let account = BrokerAccountSnapshot { broker_id:"selected-broker".into(), account_id:"acct".into(),
            account_currency:"USD".into(), equity:10_000.0, available_margin:5_000.0, gross_exposure:0.0,
            daily_realized_pnl:0.0, daily_unrealized_pnl:0.0, open_positions:0, observed_at_ms:10_000 };
        let order = NormalizedOrder { client_order_id:"equity-order-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Buy, order_type:OrderType::Market,
            quantity:1.0, limit_price:None, stop_price:Some(95.0), time_in_force:None, reduce_only:false };
        let policy = PortfolioRiskPolicy::default();
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:100.0, protective_stop_price:Some(95.0), now_ms:16_100, reference_price_observed_at_ms:10_000,
            protective_order_supported:true, required_margin_account_currency:100.0 }).is_err());
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:100.0, protective_stop_price:Some(95.0), now_ms:10_100, reference_price_observed_at_ms:10_000,
            protective_order_supported:false, required_margin_account_currency:100.0 }).is_err());
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:100.0, protective_stop_price:Some(95.0), now_ms:16_100, reference_price_observed_at_ms:10_000,
            protective_order_supported:true, required_margin_account_currency:100.0 }).is_err());
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:100.0, protective_stop_price:Some(95.0), now_ms:10_100, reference_price_observed_at_ms:10_000,
            protective_order_supported:true, required_margin_account_currency:6_000.0 }).is_err());
    }

    #[test]
    fn broker_registry_fails_closed_for_unconfigured_selection() {
        let registry = BrokerRegistry::default();
        assert!(registry.resolve("   ").is_err());
        assert!(registry.resolve("selected-broker").is_err());
        assert!(registry.configured_brokers().unwrap().is_empty());
    }

    #[test]
    fn broker_capabilities_gate_order_types_and_protective_orders() {
        let order = NormalizedOrder {
            client_order_id: "capability-order".into(), account_id: "acct".into(),
            instrument_id: "instrument".into(), side: OrderSide::Buy, order_type: OrderType::Market,
            quantity: 1.0, limit_price: None, stop_price: Some(90.0), time_in_force: None,
            reduce_only: false,
        };
        let capabilities = BrokerCapabilities {
            market_orders: true, limit_orders: false, stop_orders: false, stop_limit_orders: false,
            native_protective_orders: true, amend_orders: false, cancel_orders: true,
            fractional_quantity: false, streaming_execution_reports: false,
        };
        assert!(validate_capabilities(&order, &capabilities).is_ok());
        let mut unsupported = order.clone();
        unsupported.order_type = OrderType::Limit;
        assert!(validate_capabilities(&unsupported, &capabilities).is_err());
        let mut no_protection = capabilities;
        no_protection.native_protective_orders = false;
        assert!(validate_capabilities(&order, &no_protection).is_err());
    }

    #[test]
    fn normalized_order_checks_instrument_identity_and_type_requirements() {
        let spec = instrument(AssetClass::Equity, "AAPL", 1.0, 1.0);
        let order = NormalizedOrder {
            client_order_id: "qoin-unique-1".into(), account_id: "acct".into(),
            instrument_id: spec.instrument_id.clone(), side: OrderSide::Buy,
            order_type: OrderType::Limit, quantity: 1.0, limit_price: Some(190.0),
            stop_price: None, time_in_force: Some("day".into()), reduce_only: false,
        };
        assert!(order.validate(&spec).is_ok());
    }
}
