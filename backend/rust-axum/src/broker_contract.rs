//! Broker-neutral execution and instrument contract.
//!
//! Shared engines and risk policy operate on normalized instruments and
//! account-currency values. Provider-specific symbols, credentials, endpoints,
//! order flags and precision rules belong inside an adapter implementation.
//! No asset class, quote currency, venue or broker is privileged here.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

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
    pub unrealized_pnl: f64,
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
            self.daily_realized_pnl, self.unrealized_pnl];
        if values.iter().any(|value| !value.is_finite())
            || self.equity <= 0.0 || self.available_margin < 0.0 || self.gross_exposure < 0.0
        {
            return Err(ContractError::InvalidAccount("account snapshot contains invalid financial values".into()));
        }
        Ok(())
    }

    pub fn current_daily_loss(&self) -> Result<f64, ContractError> {
        self.validate()?;
        let pnl = self.daily_realized_pnl + self.unrealized_pnl;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let spec = instrument(AssetClass::Equity, "ABC", 1.0, 1.0);
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
            daily_realized_pnl: -1_000.0, unrealized_pnl: -500.0,
            open_positions: 2, observed_at_ms: 1,
        };
        assert_eq!(account.current_daily_loss().unwrap(), 1_500.0);
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
