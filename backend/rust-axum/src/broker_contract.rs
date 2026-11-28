//! Broker-neutral execution and instrument contract.
//!
//! Shared engines and risk policy operate on normalized instruments and
//! account-currency values. Provider-specific symbols, credentials, endpoints,
//! order flags and precision rules belong inside an adapter implementation.
//! No asset class, quote currency, venue or broker is privileged here.

use async_trait::async_trait;
use futures_util::future::join_all;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fmt, sync::{Arc, RwLock}};

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
    pub contract_multiplier: Decimal,
    pub tick_size: Decimal,
    pub quantity_step: Decimal,
    pub minimum_quantity: Decimal,
    pub minimum_notional: Option<Decimal>,
    pub price_currency_to_account_rate: Decimal,
    pub conversion_rate_observed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerQuote {
    pub broker_id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub observed_at_ms: i64,
}

impl BrokerQuote {
    pub fn execution_price(&self, side: OrderSide) -> Result<Decimal, ContractError> {
        if self.broker_id.trim().is_empty() || self.account_id.trim().is_empty()
            || self.instrument_id.trim().is_empty() || self.bid <= Decimal::ZERO
            || self.ask < self.bid || self.observed_at_ms <= 0
        {
            return Err(ContractError::InvalidInstrument("broker quote identity, bid/ask or timestamp is invalid".into()));
        }
        Ok(match side { OrderSide::Buy => self.ask, OrderSide::Sell => self.bid })
    }
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
            if value <= Decimal::ZERO {
                return Err(ContractError::InvalidInstrument(format!("{name} must be positive")));
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
            || self.base_currency.as_ref().is_some_and(|currency| currency.trim().is_empty())
        {
            return Err(ContractError::InvalidInstrument("instrument identity and currencies are required".into()));
        }
        if self.minimum_notional.is_some_and(|value| value < Decimal::ZERO) {
            return Err(ContractError::InvalidInstrument("minimum_notional must be non-negative".into()));
        }
        Ok(())
    }

    pub fn notional_in_account_currency(&self, quantity: Decimal, price: Decimal) -> Result<Decimal, ContractError> {
        self.validate()?;
        if quantity <= Decimal::ZERO || price <= Decimal::ZERO {
            return Err(ContractError::InvalidOrder("quantity and price must be positive".into()));
        }
        let notional = quantity
            .checked_mul(self.contract_multiplier)
            .and_then(|value| value.checked_mul(price))
            .and_then(|value| value.checked_mul(self.price_currency_to_account_rate))
            .ok_or_else(|| ContractError::InvalidOrder("converted notional arithmetic overflow".into()))?;
        if notional <= Decimal::ZERO {
            return Err(ContractError::InvalidOrder("converted notional is invalid".into()));
        }
        Ok(notional)
    }

    pub fn validate_price(&self, price: Decimal) -> Result<(), ContractError> {
        self.validate()?;
        if price <= Decimal::ZERO {
            return Err(ContractError::InvalidOrder("price must be positive".into()));
        }
        let remainder = price.checked_rem(self.tick_size)
            .ok_or_else(|| ContractError::InvalidOrder("price tick calculation overflow".into()))?;
        if remainder != Decimal::ZERO {
            return Err(ContractError::InvalidOrder("price does not conform to instrument tick size".into()));
        }
        Ok(())
    }

    pub fn validate_quantity(&self, quantity: Decimal) -> Result<(), ContractError> {
        self.validate()?;
        if quantity < self.minimum_quantity {
            return Err(ContractError::InvalidOrder("quantity is below the instrument minimum or invalid".into()));
        }
        let remainder = quantity.checked_rem(self.quantity_step)
            .ok_or_else(|| ContractError::InvalidOrder("quantity step calculation overflow".into()))?;
        if remainder != Decimal::ZERO {
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
    pub account_healthy: bool,
    pub trading_enabled: bool,
    pub equity: Decimal,
    pub available_margin: Decimal,
    pub gross_exposure: Decimal,
    pub daily_realized_pnl: Decimal,
    pub daily_unrealized_pnl: Decimal,
    pub open_positions: usize,
    pub observed_at_ms: i64,
}

impl BrokerAccountSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.broker_id.trim().is_empty() || self.account_id.trim().is_empty()
            || self.account_currency.trim().is_empty() || self.observed_at_ms <= 0
            || !self.account_healthy || !self.trading_enabled
        {
            return Err(ContractError::InvalidAccount("account identity, currency and observation time are required".into()));
        }
        if self.equity <= Decimal::ZERO
            || self.available_margin < Decimal::ZERO
            || self.gross_exposure < Decimal::ZERO
        {
            return Err(ContractError::InvalidAccount("account snapshot contains invalid financial values".into()));
        }
        Ok(())
    }

    pub fn current_daily_loss(&self) -> Result<Decimal, ContractError> {
        self.validate()?;
        let pnl = self.daily_realized_pnl.checked_add(self.daily_unrealized_pnl)
            .ok_or_else(|| ContractError::InvalidAccount("daily P&L arithmetic overflow".into()))?;
        if pnl < Decimal::ZERO {
            Decimal::ZERO.checked_sub(pnl)
                .ok_or_else(|| ContractError::InvalidAccount("daily loss arithmetic overflow".into()))
        } else {
            Ok(Decimal::ZERO)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedOrder {
    pub client_order_id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub limit_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: Option<String>,
    pub reduce_only: bool,
}

impl NormalizedOrder {
    /// Price used for margin estimation, aligned with the risk gate's order-type semantics.
    pub fn margin_reference_price(&self, quote: &BrokerQuote) -> Result<Decimal, ContractError> {
        let market_price = quote.execution_price(self.side)?;
        let price = match self.order_type {
            OrderType::Market => market_price,
            OrderType::Limit | OrderType::StopLimit => self.limit_price
                .ok_or_else(|| ContractError::InvalidOrder("limit execution price is required".into()))?,
            OrderType::Stop => self.stop_price
                .ok_or_else(|| ContractError::InvalidOrder("stop trigger price is required".into()))?,
        };
        if price <= Decimal::ZERO {
            return Err(ContractError::InvalidOrder("margin reference price must be positive".into()));
        }
        Ok(price)
    }

    pub fn validate(&self, instrument: &InstrumentSpec) -> Result<(), ContractError> {
        if self.client_order_id.trim().is_empty() || self.client_order_id.len() > 128
            || self.account_id.trim().is_empty() || self.instrument_id != instrument.instrument_id
        {
            return Err(ContractError::InvalidOrder("order identity does not match selected account and instrument".into()));
        }
        instrument.validate_quantity(self.quantity)?;
        match self.order_type {
            OrderType::Limit => {
                let price = self.limit_price.ok_or_else(|| ContractError::InvalidOrder("limit orders require a limit price".into()))?;
                instrument.validate_price(price)?;
            }
            OrderType::Stop => {
                let price = self.stop_price.ok_or_else(|| ContractError::InvalidOrder("stop orders require a stop price".into()))?;
                instrument.validate_price(price)?;
            }
            OrderType::StopLimit => {
                let stop = self.stop_price.ok_or_else(|| ContractError::InvalidOrder("stop-limit orders require a stop price".into()))?;
                let limit = self.limit_price.ok_or_else(|| ContractError::InvalidOrder("stop-limit orders require a limit price".into()))?;
                instrument.validate_price(stop)?;
                instrument.validate_price(limit)?;
            }
            OrderType::Market => {}
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
    pub cumulative_quantity: Decimal,
    pub last_fill_quantity: Decimal,
    pub last_fill_price: Decimal,
    pub fee_amount: Option<Decimal>,
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
    pub max_risk_fraction: Decimal,
    pub max_order_notional_fraction: Decimal,
    pub max_gross_leverage: Decimal,
    pub max_daily_loss_fraction: Decimal,
    pub max_open_positions: usize,
    pub max_market_data_age_ms: i64,
}
impl Default for PortfolioRiskPolicy {
    fn default() -> Self {
        Self {
            max_risk_fraction: Decimal::new(25, 4),
            max_order_notional_fraction: Decimal::new(25, 2),
            max_gross_leverage: Decimal::from(2),
            max_daily_loss_fraction: Decimal::new(2, 2),
            max_open_positions: 5,
            max_market_data_age_ms: 5_000,
        }
    }
}
#[derive(Debug, Clone)]
pub struct RiskCheckInput {
    pub reference_price: Decimal,
    pub protective_stop_price: Option<Decimal>,
    pub now_ms: i64,
    pub reference_price_observed_at_ms: i64,
    pub protective_order_supported: bool,
    /// Broker-calculated margin requirement, normalized to account currency.
    pub required_margin_account_currency: Decimal,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskApproval {
    pub notional_account_currency: Decimal,
    pub maximum_loss_account_currency: Decimal,
    pub remaining_daily_loss_budget: Decimal,
    pub projected_gross_exposure: Decimal,
}
fn current_time_ms() -> Result<i64, ContractError> {
    let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ContractError::InvalidOrder("system clock is before Unix epoch".into()))?;
    i64::try_from(duration.as_millis())
        .map_err(|_| ContractError::InvalidOrder("system clock timestamp overflow".into()))
}

/// Shared pre-trade gate. All amounts are normalized to account currency;
/// no USD/USDT assumption is made.
pub fn validate_portfolio_order(
    policy: &PortfolioRiskPolicy,
    account: &BrokerAccountSnapshot,
    instrument: &InstrumentSpec,
    order: &NormalizedOrder,
    input: &RiskCheckInput,
) -> Result<RiskApproval, ContractError> {
    account.validate()?;
    order.validate(instrument)?;
    if account.broker_id != instrument.broker_id || order.account_id != account.account_id {
        return Err(ContractError::InvalidOrder("broker/account does not match selected instrument".into()));
    }
    let fractions = [
        policy.max_risk_fraction,
        policy.max_order_notional_fraction,
        policy.max_daily_loss_fraction,
    ];
    if fractions.iter().any(|value| *value <= Decimal::ZERO || *value > Decimal::ONE)
        || policy.max_gross_leverage <= Decimal::ZERO
        || policy.max_open_positions == 0
        || policy.max_market_data_age_ms < 0
    {
        return Err(ContractError::InvalidOrder("portfolio risk policy is invalid".into()));
    }

    let account_age_ms = input.now_ms.saturating_sub(account.observed_at_ms);
    let quote_age_ms = input.now_ms.saturating_sub(input.reference_price_observed_at_ms);
    let conversion_age_ms = input.now_ms.saturating_sub(instrument.conversion_rate_observed_at_ms);
    if input.now_ms <= 0
        || input.reference_price_observed_at_ms <= 0
        || account.observed_at_ms > input.now_ms
        || input.reference_price_observed_at_ms > input.now_ms
        || instrument.conversion_rate_observed_at_ms > input.now_ms
        || account_age_ms < 0
        || account_age_ms > policy.max_market_data_age_ms
        || input.reference_price <= Decimal::ZERO
        || quote_age_ms < 0
        || quote_age_ms > policy.max_market_data_age_ms
        || conversion_age_ms < 0
        || conversion_age_ms > policy.max_market_data_age_ms
        || input.required_margin_account_currency < Decimal::ZERO
    {
        return Err(ContractError::InvalidOrder("market price, conversion, margin or observation time is invalid or stale".into()));
    }
    if !input.protective_order_supported && !order.reduce_only {
        return Err(ContractError::Unsupported("selected broker/instrument cannot place a protective order".into()));
    }
    if instrument.quote_currency.eq_ignore_ascii_case(&account.account_currency)
        && instrument.price_currency_to_account_rate != Decimal::ONE
    {
        return Err(ContractError::InvalidInstrument("same-currency conversion rate must equal one".into()));
    }

    let execution_price = match order.order_type {
        OrderType::Market => input.reference_price,
        OrderType::Limit | OrderType::StopLimit => order.limit_price
            .ok_or_else(|| ContractError::InvalidOrder("limit execution price is required".into()))?,
        OrderType::Stop => order.stop_price
            .ok_or_else(|| ContractError::InvalidOrder("stop trigger price is required".into()))?,
    };
    if execution_price <= Decimal::ZERO {
        return Err(ContractError::InvalidOrder("execution price must be positive".into()));
    }
    let notional = instrument.notional_in_account_currency(order.quantity, execution_price)?;
    // A broker-enforced reduce-only instruction cannot add exposure. The
    // adapter must preserve that flag; position-level close sizing remains
    // the broker's authoritative responsibility.
    let projected_gross_exposure = if order.reduce_only {
        account.gross_exposure
    } else {
        account.gross_exposure.checked_add(notional)
            .ok_or_else(|| ContractError::InvalidOrder("projected exposure arithmetic overflow".into()))?
    };
    let notional_cap = account.equity.checked_mul(policy.max_order_notional_fraction)
        .ok_or_else(|| ContractError::InvalidOrder("notional budget arithmetic overflow".into()))?;
    let leverage_cap = account.equity.checked_mul(policy.max_gross_leverage)
        .ok_or_else(|| ContractError::InvalidOrder("leverage budget arithmetic overflow".into()))?;
    if !order.reduce_only && notional > notional_cap {
        return Err(ContractError::InvalidOrder("order exceeds account-currency notional budget".into()));
    }
    if input.required_margin_account_currency > account.available_margin {
        return Err(ContractError::InvalidOrder("insufficient available margin in broker account".into()));
    }
    if !order.reduce_only && projected_gross_exposure > leverage_cap {
        return Err(ContractError::InvalidOrder("projected gross exposure exceeds leverage limit".into()));
    }
    if account.open_positions >= policy.max_open_positions && !order.reduce_only {
        return Err(ContractError::InvalidOrder("open-position limit reached".into()));
    }

    if let Some(minimum) = instrument.minimum_notional {
        let quote_notional = order.quantity
            .checked_mul(instrument.contract_multiplier)
            .and_then(|value| value.checked_mul(execution_price))
            .ok_or_else(|| ContractError::InvalidOrder("minimum-notional calculation overflow".into()))?;
        if quote_notional < minimum {
            return Err(ContractError::InvalidOrder("order is below instrument minimum notional".into()));
        }
    }

    let daily_loss = account.current_daily_loss()?;
    let daily_cap = account.equity.checked_mul(policy.max_daily_loss_fraction)
        .ok_or_else(|| ContractError::InvalidOrder("daily-loss budget arithmetic overflow".into()))?;
    if !order.reduce_only && daily_loss >= daily_cap {
        return Err(ContractError::InvalidOrder("daily loss limit reached".into()));
    }
    let remaining_daily_loss_budget = if daily_loss >= daily_cap {
        Decimal::ZERO
    } else {
        daily_cap.checked_sub(daily_loss)
            .ok_or_else(|| ContractError::InvalidOrder("remaining daily-loss budget arithmetic overflow".into()))?
    };

    let maximum_loss = if order.reduce_only {
        Decimal::ZERO
    } else {
        let stop = input.protective_stop_price.ok_or_else(|| ContractError::Unsupported(
            "opening orders require a broker-supported protective stop".into()
        ))?;
        let valid = match order.side {
            OrderSide::Buy => stop > Decimal::ZERO && stop < execution_price,
            OrderSide::Sell => stop > execution_price,
        };
        if !valid {
            return Err(ContractError::InvalidOrder("protective stop is invalid for order direction".into()));
        }
        let distance = if execution_price >= stop {
            execution_price.checked_sub(stop)
        } else {
            stop.checked_sub(execution_price)
        }.ok_or_else(|| ContractError::InvalidOrder("stop-distance arithmetic overflow".into()))?;
        let loss = instrument.notional_in_account_currency(order.quantity, distance)?;
        let risk_cap = account.equity.checked_mul(policy.max_risk_fraction)
            .ok_or_else(|| ContractError::InvalidOrder("maximum-risk budget arithmetic overflow".into()))?;
        if loss > risk_cap {
            return Err(ContractError::InvalidOrder("maximum loss at stop exceeds risk budget".into()));
        }
        if loss > remaining_daily_loss_budget {
            return Err(ContractError::InvalidOrder("maximum loss at stop exceeds remaining daily-loss budget".into()));
        }
        loss
    };

    Ok(RiskApproval {
        notional_account_currency: notional,
        maximum_loss_account_currency: maximum_loss,
        remaining_daily_loss_budget,
        projected_gross_exposure,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreflight {
    /// Requested native protective stop. The adapter must submit it atomically
    /// with the entry order or reject the request; it is never a post-fill best effort.
    pub protective_stop_price: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
pub struct SelectedBrokerOrder {
    pub broker_id: String,
    pub order: NormalizedOrder,
    pub preflight: ExecutionPreflight,
}

#[derive(Debug, Clone)]
pub struct PreparedBrokerOrder {
    pub broker_id: String,
    pub order: NormalizedOrder,
    pub protective_stop_price: Option<Decimal>,
    pub risk: RiskApproval,
}

/// Fetch and validate every selected account concurrently without submitting
/// any order. A fan-out coordinator must complete this entire preflight for
/// all selected accounts, then persist parent and child intents, before it
/// begins any child submission.
pub async fn preflight_selected_orders(
    registry: &BrokerRegistry,
    owner_uid: &str,
    policy: &PortfolioRiskPolicy,
    selected: Vec<SelectedBrokerOrder>,
) -> Result<Vec<PreparedBrokerOrder>, ContractError> {
    if owner_uid.trim().is_empty() {
        return Err(ContractError::InvalidAccount("verified owner identity is required".into()));
    }
    if selected.is_empty() || selected.len() > 20 {
        return Err(ContractError::InvalidOrder("select between 1 and 20 broker accounts".into()));
    }

    let mut account_keys = HashSet::with_capacity(selected.len());
    let mut client_order_ids = HashSet::with_capacity(selected.len());
    let mut resolved = Vec::with_capacity(selected.len());
    for item in selected {
        let broker_id = item.broker_id.trim().to_ascii_lowercase();
        let account_id = item.order.account_id.trim().to_string();
        if broker_id.is_empty() || account_id.is_empty()
            || item.order.client_order_id.trim().is_empty()
            || !account_keys.insert((broker_id.clone(), account_id.clone()))
            || !client_order_ids.insert(item.order.client_order_id.clone())
        {
            return Err(ContractError::InvalidOrder("selected broker/account and client-order identities must be non-empty and unique".into()));
        }
        let adapter = registry.resolve(&broker_id)?;
        if !adapter.broker_id().eq_ignore_ascii_case(&broker_id) {
            return Err(ContractError::Broker("registry returned an adapter for a different broker".into()));
        }
        resolved.push((broker_id, adapter, item.order, item.preflight));
    }

    let checks = resolved.into_iter().map(|(broker_id, adapter, order, input)| {
        // Copy the shared reference into each future; the policy value is not Copy.
        let policy = policy;
        async move {
            let account = adapter.account_snapshot(owner_uid, &order.account_id).await?;
            let instrument = adapter.instrument_spec(owner_uid, &order.account_id, &order.instrument_id).await?;
            let quote = adapter.latest_quote(owner_uid, &order.account_id, &order.instrument_id).await?;
            let now_ms = current_time_ms()?;
            if account.broker_id != adapter.broker_id()
                || account.account_id != order.account_id
                || instrument.broker_id != adapter.broker_id()
                || instrument.instrument_id != order.instrument_id
                || quote.broker_id != adapter.broker_id()
                || quote.account_id != order.account_id
                || quote.instrument_id != order.instrument_id
            {
                return Err(ContractError::InvalidAccount("adapter returned an account or instrument that does not match the selected order".into()));
            }
            let capabilities = adapter.capabilities(owner_uid, &order.account_id, &order.instrument_id).await?;
            validate_capabilities(&order, &capabilities)?;
            let reference_price = order.margin_reference_price(&quote)?;
            let required_margin = adapter.required_margin_account_currency(owner_uid, &order, reference_price).await?;
            let risk = validate_portfolio_order(policy, &account, &instrument, &order, &RiskCheckInput {
                reference_price,
                protective_stop_price: input.protective_stop_price,
                now_ms,
                reference_price_observed_at_ms: quote.observed_at_ms,
                protective_order_supported: capabilities.native_protective_orders,
                required_margin_account_currency: required_margin,
            })?;
            Ok(PreparedBrokerOrder { broker_id, order, protective_stop_price: input.protective_stop_price, risk })
        }
    });
    // join_all waits for every account check. No submit method is called here;
    // a single failed preflight invalidates the complete selected-account set.
    join_all(checks).await.into_iter().collect()
}

/// Shared adapter invocation sequence. Callers must persist the client order
/// intent before invoking this function and persist the returned receipt before
/// reporting success. Any timeout/ambiguous adapter error requires reconciliation,
/// never blind resubmission.
pub async fn submit_risk_checked(
    adapter: &dyn BrokerAdapter,
    owner_uid: &str,
    policy: &PortfolioRiskPolicy,
    order: NormalizedOrder,
    preflight: ExecutionPreflight,
) -> Result<RiskCheckedReceipt, ContractError> {
    if owner_uid.trim().is_empty() {
        return Err(ContractError::InvalidAccount("verified owner identity is required".into()));
    }
    if adapter.broker_id().trim().is_empty() {
        return Err(ContractError::Broker("adapter has no broker identity".into()));
    }
    // Before submitting, prove that this client order ID is absent from the
    // provider's authoritative order history. Unsupported or incomplete
    // lookups fail closed; an existing order must be reconciled, never resent.
    if lookup_order_by_client_order_id(
        adapter,
        owner_uid,
        &order.account_id,
        &order.client_order_id,
    ).await?.is_some() {
        return Err(ContractError::Broker(
            "client_order_id already exists at the broker; reconcile the existing order instead of submitting again".into()
        ));
    }
    let account = adapter.account_snapshot(owner_uid, &order.account_id).await?;
    let instrument = adapter.instrument_spec(owner_uid, &order.account_id, &order.instrument_id).await?;
    let quote = adapter.latest_quote(owner_uid, &order.account_id, &order.instrument_id).await?;
    let now_ms = current_time_ms()?;
    if account.broker_id != adapter.broker_id() || account.account_id != order.account_id
        || instrument.broker_id != adapter.broker_id() || instrument.instrument_id != order.instrument_id
        || quote.broker_id != adapter.broker_id() || quote.account_id != order.account_id
        || quote.instrument_id != order.instrument_id {
        return Err(ContractError::InvalidAccount("adapter returned account, instrument or quote for another broker/account".into()));
    }
    let capabilities = adapter.capabilities(owner_uid, &order.account_id, &order.instrument_id).await?;
    validate_capabilities(&order, &capabilities)?;
    let reference_price = order.margin_reference_price(&quote)?;
    let required_margin = adapter.required_margin_account_currency(owner_uid, &order, reference_price).await?;
    let risk = validate_portfolio_order(policy, &account, &instrument, &order, &RiskCheckInput {
        reference_price,
        protective_stop_price: preflight.protective_stop_price,
        now_ms,
        reference_price_observed_at_ms: quote.observed_at_ms,
        protective_order_supported: capabilities.native_protective_orders,
        required_margin_account_currency: required_margin,
    })?;
    let broker_receipt = adapter.submit_order(owner_uid, order.clone(), preflight.protective_stop_price).await?;
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
    async fn capabilities(&self, owner_uid: &str, account_id: &str, instrument_id: &str) -> Result<BrokerCapabilities, ContractError>;
    async fn account_snapshot(&self, owner_uid: &str, account_id: &str) -> Result<BrokerAccountSnapshot, ContractError>;
    async fn instrument_spec(&self, owner_uid: &str, account_id: &str, instrument_id: &str) -> Result<InstrumentSpec, ContractError>;
    /// Fresh provider quote for the authenticated user's connected account.
    async fn latest_quote(&self, owner_uid: &str, account_id: &str, instrument_id: &str) -> Result<BrokerQuote, ContractError>;
    /// Broker-native initial margin for this exact order, normalized to account currency.
    async fn required_margin_account_currency(&self, owner_uid: &str, order: &NormalizedOrder, reference_price: Decimal) -> Result<Decimal, ContractError>;
    /// Submit entry and protective stop as one native/bracket transaction.
    /// Opening orders without native attached protection are rejected upstream.
    async fn submit_order(&self, owner_uid: &str, order: NormalizedOrder, protective_stop_price: Option<Decimal>) -> Result<BrokerOrderReceipt, ContractError>;
    /// Find an order by the stable client ID supplied before submission.
    /// A missing result is authoritative only if the adapter searched the
    /// venue's complete relevant order history, including closed/rejected orders.
    /// Adapters without that guarantee must return Unsupported, never None.
    async fn find_order_by_client_order_id(
        &self,
        _owner_uid: &str,
        _account_id: &str,
        _client_order_id: &str,
    ) -> Result<Option<BrokerOrderReceipt>, ContractError> {
        Err(ContractError::Unsupported(
            "adapter does not support authoritative client-order-id lookup".into()
        ))
    }
    async fn order_status(&self, owner_uid: &str, account_id: &str, broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError>;
    async fn cancel_order(&self, owner_uid: &str, account_id: &str, broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError>;
}

/// Look up an ambiguous submission and reject receipts that do not match the
/// exact persisted child identity. A missing result is authoritative only
/// under the BrokerAdapter contract.
pub async fn lookup_order_by_client_order_id(
    adapter: &dyn BrokerAdapter,
    owner_uid: &str,
    account_id: &str,
    client_order_id: &str,
) -> Result<Option<BrokerOrderReceipt>, ContractError> {
    if owner_uid.trim().is_empty() || account_id.trim().is_empty() || client_order_id.trim().is_empty() {
        return Err(ContractError::InvalidOrder("account and client-order IDs are required for reconciliation".into()));
    }
    let result = adapter.find_order_by_client_order_id(owner_uid, account_id, client_order_id).await?;
    if let Some(receipt) = &result {
        if !receipt.broker_id.eq_ignore_ascii_case(adapter.broker_id())
            || receipt.account_id != account_id
            || receipt.client_order_id != client_order_id
            || receipt.broker_order_id.trim().is_empty()
            || receipt.accepted_at_ms <= 0
            || receipt.status.trim().is_empty()
        {
            return Err(ContractError::Broker(
                "client-order lookup returned a mismatched or incomplete receipt".into()
            ));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(value: &str) -> Decimal { value.parse().expect("valid test decimal") }

    fn instrument(asset_class: AssetClass, symbol: &str, multiplier: &str, rate: &str) -> InstrumentSpec {
        InstrumentSpec {
            instrument_id: format!("{:?}:{}", asset_class, symbol),
            broker_id: "selected-broker".into(),
            venue_id: "selected-venue".into(),
            broker_symbol: symbol.into(),
            asset_class,
            base_currency: None,
            quote_currency: "EUR".into(),
            settlement_currency: "EUR".into(),
            quantity_unit: QuantityUnit::Units,
            contract_multiplier: d(multiplier),
            tick_size: d("0.0001"),
            quantity_step: d("0.01"),
            minimum_quantity: d("0.01"),
            minimum_notional: None,
            price_currency_to_account_rate: d(rate),
            conversion_rate_observed_at_ms: current_time_ms().expect("test clock"),
        }
    }

    fn instrument_at(asset_class: AssetClass, symbol: &str, multiplier: &str, rate: &str, observed_at_ms: i64) -> InstrumentSpec {
        let mut spec = instrument(asset_class, symbol, multiplier, rate);
        spec.conversion_rate_observed_at_ms = observed_at_ms;
        spec
    }

    #[test]
    fn broker_quote_uses_ask_for_buys_and_bid_for_sells() {
        let quote = BrokerQuote {
            broker_id: "adapter".into(), account_id: "account".into(),
            instrument_id: "EURUSD".into(), bid: d("1.0841"), ask: d("1.0843"),
            observed_at_ms: 1,
        };
        assert_eq!(quote.execution_price(OrderSide::Buy).unwrap(), d("1.0843"));
        assert_eq!(quote.execution_price(OrderSide::Sell).unwrap(), d("1.0841"));
    }

    #[test]
    fn margin_reference_price_matches_market_limit_and_stop_order_semantics() {
        let quote = BrokerQuote {
            broker_id: "adapter".into(), account_id: "account".into(),
            instrument_id: "EURUSD".into(), bid: d("1.0841"), ask: d("1.0843"),
            observed_at_ms: 1,
        };
        let base = NormalizedOrder {
            client_order_id: "price-semantics".into(), account_id: "account".into(),
            instrument_id: "EURUSD".into(), side: OrderSide::Buy, order_type: OrderType::Market,
            quantity: d("1"), limit_price: None, stop_price: None, time_in_force: None, reduce_only: true,
        };
        assert_eq!(base.margin_reference_price(&quote).unwrap(), d("1.0843"));
        let mut limit = base.clone();
        limit.order_type = OrderType::Limit;
        limit.limit_price = Some(d("1.0800"));
        assert_eq!(limit.margin_reference_price(&quote).unwrap(), d("1.0800"));
        let mut stop = base;
        stop.order_type = OrderType::Stop;
        stop.stop_price = Some(d("1.0900"));
        assert_eq!(stop.margin_reference_price(&quote).unwrap(), d("1.0900"));
    }

    #[test]
    fn broker_quote_rejects_crossed_or_incomplete_market_data() {
        let mut quote = BrokerQuote {
            broker_id: "adapter".into(), account_id: "account".into(),
            instrument_id: "BTC-EUR".into(), bid: d("60000"), ask: d("59999"),
            observed_at_ms: 1,
        };
        assert!(quote.execution_price(OrderSide::Buy).is_err());
        quote.ask = d("60001");
        quote.observed_at_ms = 0;
        assert!(quote.execution_price(OrderSide::Sell).is_err());
    }

    #[test]
    fn supports_non_usdt_forex_equity_and_crypto_instruments() {
        let cases = [
            (instrument(AssetClass::Forex, "EURUSD", "100000", "0.92"), d("0.01"), d("1.1")),
            (instrument(AssetClass::Equity, "AAPL", "1", "0.92"), d("2"), d("190")),
            (instrument(AssetClass::Crypto, "BTC-EUR", "1", "1"), d("0.01"), d("60000")),
        ];
        for (spec, quantity, price) in cases {
            let notional = spec.notional_in_account_currency(quantity, price).unwrap();
            assert!(notional > Decimal::ZERO);
        }
    }

    #[test]
    fn quantity_step_and_minimum_are_enforced_per_instrument() {
        let mut spec = instrument(AssetClass::Equity, "ABC", "1", "1");
        spec.quantity_step = d("0.05");
        assert!(spec.validate_quantity(d("1.25")).is_ok());
        assert!(spec.validate_quantity(d("1.23")).is_err());
        assert!(spec.validate_quantity(d("0.001")).is_err());
    }

    #[test]
    fn account_loss_uses_realized_and_unrealized_pnl() {
        let account = BrokerAccountSnapshot {
            broker_id: "broker".into(), account_id: "account".into(),
            account_currency: "KES".into(), account_healthy: true, trading_enabled: true, equity: d("100000"),
            available_margin: d("50000"), gross_exposure: d("20000"),
            daily_realized_pnl: d("-1000"), daily_unrealized_pnl: d("-500"),
            open_positions: 2, observed_at_ms: 1,
        };
        assert_eq!(account.current_daily_loss().unwrap(), d("1500"));
    }

    #[test]
    fn portfolio_risk_is_account_currency_and_asset_class_agnostic() {
        let mut spec = instrument_at(AssetClass::Forex, "EURUSD", "100000", "0.92", 10_000);
        spec.quote_currency = "USD".into();
        let account = BrokerAccountSnapshot {
            broker_id:"selected-broker".into(), account_id:"acct".into(),
            account_currency:"EUR".into(), account_healthy:true, trading_enabled:true, equity:d("10000"), available_margin:d("5000"),
            gross_exposure:Decimal::ZERO, daily_realized_pnl:Decimal::ZERO,
            daily_unrealized_pnl:Decimal::ZERO, open_positions:0, observed_at_ms:10_000
        };
        let order = NormalizedOrder {
            client_order_id:"fx-order-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Buy, order_type:OrderType::Market,
            quantity:d("0.01"), limit_price:None, stop_price:Some(d("1.09")),
            time_in_force:None, reduce_only:false
        };
        let approved = validate_portfolio_order(&PortfolioRiskPolicy::default(), &account, &spec, &order,
            &RiskCheckInput {
                reference_price:d("1.1"), protective_stop_price:Some(d("1.09")),
                now_ms:10_100, reference_price_observed_at_ms:10_000,
                protective_order_supported:true, required_margin_account_currency:d("100")
            }).unwrap();
        assert_eq!(approved.notional_account_currency, d("1012"));
        assert!(approved.maximum_loss_account_currency > Decimal::ZERO);
    }

    #[test]
    fn portfolio_risk_rejects_stale_quotes_and_missing_protection() {
        let spec = instrument_at(AssetClass::Equity, "ABC", "1", "1", 10_000);
        let account = BrokerAccountSnapshot {
            broker_id:"selected-broker".into(), account_id:"acct".into(),
            account_currency:"USD".into(), account_healthy:true, trading_enabled:true, equity:d("10000"), available_margin:d("5000"),
            gross_exposure:Decimal::ZERO, daily_realized_pnl:Decimal::ZERO,
            daily_unrealized_pnl:Decimal::ZERO, open_positions:0, observed_at_ms:10_000
        };
        let order = NormalizedOrder {
            client_order_id:"equity-order-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Buy, order_type:OrderType::Market,
            quantity:d("1"), limit_price:None, stop_price:Some(d("95")),
            time_in_force:None, reduce_only:false
        };
        let policy = PortfolioRiskPolicy::default();
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:d("100"), protective_stop_price:Some(d("95")), now_ms:16_100,
            reference_price_observed_at_ms:10_000, protective_order_supported:true,
            required_margin_account_currency:d("100") }).is_err());
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:d("100"), protective_stop_price:Some(d("95")), now_ms:10_100,
            reference_price_observed_at_ms:10_000, protective_order_supported:false,
            required_margin_account_currency:d("100") }).is_err());
        assert!(validate_portfolio_order(&policy,&account,&spec,&order,&RiskCheckInput {
            reference_price:d("100"), protective_stop_price:Some(d("95")), now_ms:10_100,
            reference_price_observed_at_ms:10_000, protective_order_supported:true,
            required_margin_account_currency:d("6000") }).is_err());
    }

    #[test]
    fn same_currency_conversion_must_be_exactly_one() {
        let mut spec = instrument_at(AssetClass::Equity, "ABC", "1", "1.00000001", 10_000);
        spec.quote_currency = "USD".into();
        let account = BrokerAccountSnapshot {
            broker_id:"selected-broker".into(), account_id:"acct".into(), account_currency:"USD".into(),
            account_healthy:true, trading_enabled:true, equity:d("10000"), available_margin:d("5000"), gross_exposure:Decimal::ZERO,
            daily_realized_pnl:Decimal::ZERO, daily_unrealized_pnl:Decimal::ZERO,
            open_positions:0, observed_at_ms:10_000
        };
        let order = NormalizedOrder {
            client_order_id:"same-currency-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Buy, order_type:OrderType::Market,
            quantity:d("1"), limit_price:None, stop_price:Some(d("95")), time_in_force:None, reduce_only:false
        };
        assert!(validate_portfolio_order(&PortfolioRiskPolicy::default(),&account,&spec,&order,&RiskCheckInput {
            reference_price:d("100"), protective_stop_price:Some(d("95")), now_ms:10_100,
            reference_price_observed_at_ms:10_000, protective_order_supported:true,
            required_margin_account_currency:d("100") }).is_err());
    }

    #[test]
    fn reduce_only_does_not_add_gross_exposure_or_consume_opening_notional_budget() {
        let spec = instrument_at(AssetClass::Equity, "ABC", "1", "1", 10_000);
        let account = BrokerAccountSnapshot {
            broker_id:"selected-broker".into(), account_id:"acct".into(), account_currency:"USD".into(),
            account_healthy:true, trading_enabled:true, equity:d("1000"), available_margin:d("1000"), gross_exposure:d("2000"),
            daily_realized_pnl:Decimal::ZERO, daily_unrealized_pnl:Decimal::ZERO,
            open_positions:5, observed_at_ms:10_000
        };
        let order = NormalizedOrder {
            client_order_id:"close-only-1".into(), account_id:"acct".into(),
            instrument_id:spec.instrument_id.clone(), side:OrderSide::Sell, order_type:OrderType::Market,
            quantity:d("10"), limit_price:None, stop_price:None, time_in_force:None, reduce_only:true
        };
        let approval = validate_portfolio_order(&PortfolioRiskPolicy::default(),&account,&spec,&order,&RiskCheckInput {
            reference_price:d("100"), protective_stop_price:None, now_ms:10_100,
            reference_price_observed_at_ms:10_000, protective_order_supported:false,
            required_margin_account_currency:d("0") }).unwrap();
        assert_eq!(approval.projected_gross_exposure, d("2000"));
        assert_eq!(approval.maximum_loss_account_currency, Decimal::ZERO);
    }

    #[test]
    fn stop_order_notional_uses_trigger_price_for_pretrade_budget() {
        let spec = instrument_at(AssetClass::Equity, "ABC", "1", "1", 10_000);
        let account = BrokerAccountSnapshot {
            broker_id: "selected-broker".into(), account_id: "acct".into(),
            account_currency: "USD".into(), account_healthy: true, trading_enabled: true,
            equity: d("10000"), available_margin: d("5000"), gross_exposure: Decimal::ZERO,
            daily_realized_pnl: Decimal::ZERO, daily_unrealized_pnl: Decimal::ZERO,
            open_positions: 0, observed_at_ms: 10_000,
        };
        let order = NormalizedOrder {
            client_order_id: "stop-budget-order".into(), account_id: "acct".into(),
            instrument_id: spec.instrument_id.clone(), side: OrderSide::Buy,
            order_type: OrderType::Stop, quantity: d("10"), limit_price: None,
            stop_price: Some(d("300")), time_in_force: None, reduce_only: false,
        };
        let result = validate_portfolio_order(&PortfolioRiskPolicy::default(), &account, &spec, &order,
            &RiskCheckInput {
                reference_price: d("100"), protective_stop_price: Some(d("95")),
                now_ms: 10_100, reference_price_observed_at_ms: 10_000,
                protective_order_supported: true, required_margin_account_currency: d("100"),
            });
        assert!(matches!(result, Err(ContractError::InvalidOrder(_))));
    }

    struct MockAdapter {
        submit_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl BrokerAdapter for MockAdapter {
        fn broker_id(&self) -> &'static str { "mock" }

        async fn latest_quote(&self, _owner_uid: &str, account_id: &str, instrument_id: &str) -> Result<BrokerQuote, ContractError> {
            Ok(BrokerQuote {
                broker_id: "mock".into(), account_id: account_id.into(), instrument_id: instrument_id.into(),
                bid: d("99.99"), ask: d("100.01"), observed_at_ms: current_time_ms()?,
            })
        }

        async fn capabilities(&self, _owner_uid: &str, _account_id: &str, _instrument_id: &str) -> Result<BrokerCapabilities, ContractError> {
            Ok(BrokerCapabilities {
                market_orders: true, limit_orders: true, stop_orders: true, stop_limit_orders: true,
                native_protective_orders: true, amend_orders: true, cancel_orders: true,
                fractional_quantity: true, streaming_execution_reports: true,
            })
        }

        async fn account_snapshot(&self, _owner_uid: &str, account_id: &str) -> Result<BrokerAccountSnapshot, ContractError> {
            Ok(BrokerAccountSnapshot {
                broker_id: "mock".into(), account_id: account_id.into(), account_currency: "USD".into(),
                account_healthy: account_id != "unhealthy", trading_enabled: account_id != "disabled", equity: d("10000"),
                available_margin: d("5000"), gross_exposure: Decimal::ZERO,
                daily_realized_pnl: Decimal::ZERO, daily_unrealized_pnl: Decimal::ZERO,
                open_positions: 0, observed_at_ms: current_time_ms().expect("test clock"),
            })
        }

        async fn instrument_spec(&self, _owner_uid: &str, _account_id: &str, instrument_id: &str) -> Result<InstrumentSpec, ContractError> {
            let mut spec = instrument(AssetClass::Equity, "MOCK", "1", "1");
            spec.instrument_id = instrument_id.into();
            spec.broker_id = "mock".into();
            spec.quote_currency = "USD".into();
            spec.settlement_currency = "USD".into();
            Ok(spec)
        }

        async fn required_margin_account_currency(&self, _owner_uid: &str, order: &NormalizedOrder, _reference_price: Decimal) -> Result<Decimal, ContractError> {
            Ok(if order.account_id == "margin-short" { d("6000") } else { d("100") })
        }

        async fn submit_order(&self, _owner_uid: &str, order: NormalizedOrder, protective_stop_price: Option<Decimal>) -> Result<BrokerOrderReceipt, ContractError> {
            if !order.reduce_only && protective_stop_price.is_none() {
                return Err(ContractError::Unsupported("mock requires attached native protection".into()));
            }
            self.submit_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(BrokerOrderReceipt {
                broker_id: "mock".into(), account_id: order.account_id,
                client_order_id: order.client_order_id, broker_order_id: "mock-order".into(),
                status: "accepted".into(), accepted_at_ms: current_time_ms().expect("test clock"), raw_status: None,
            })
        }

        async fn order_status(&self, _owner_uid: &str, _account_id: &str, _broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError> {
            Err(ContractError::Unsupported("mock status is not used in preflight tests".into()))
        }

        async fn cancel_order(&self, _owner_uid: &str, _account_id: &str, _broker_order_id: &str) -> Result<BrokerOrderReceipt, ContractError> {
            Err(ContractError::Unsupported("mock cancellation is not used in preflight tests".into()))
        }
    }

    #[tokio::test]
    async fn adapter_without_authoritative_client_id_lookup_fails_closed() {
        let adapter = MockAdapter {
            submit_calls: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };
        let result = lookup_order_by_client_order_id(&adapter, "verified-user", "account-a", "client-order-0001").await;
        assert!(matches!(result, Err(ContractError::Unsupported(_))));
    }

    fn selected_order(account_id: &str) -> SelectedBrokerOrder {
        SelectedBrokerOrder {
            broker_id: "mock".into(),
            order: NormalizedOrder {
                client_order_id: format!("mock-order-{account_id}"),
                account_id: account_id.into(),
                instrument_id: "instrument-1".into(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: d("1"),
                limit_price: None,
                stop_price: Some(d("95")),
                time_in_force: None,
                reduce_only: false,
            },
            preflight: ExecutionPreflight {
                protective_stop_price: Some(d("95")),
            },
        }
    }

    #[tokio::test]
    async fn submit_path_never_dispatches_without_authoritative_client_id_lookup() {
        let submit_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let adapter = MockAdapter { submit_calls: submit_calls.clone() };
        let selected = selected_order("account-a");
        let result = submit_risk_checked(
            &adapter,
            "verified-user",
            &PortfolioRiskPolicy::default(),
            selected.order,
            selected.preflight,
        ).await;
        assert!(matches!(result, Err(ContractError::Unsupported(_))));
        assert_eq!(submit_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn selected_account_preflight_checks_every_account_without_submitting() {
        let submit_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let registry = BrokerRegistry::default();
        registry.register(std::sync::Arc::new(MockAdapter { submit_calls: submit_calls.clone() })).unwrap();
        let prepared = preflight_selected_orders(
            &registry,
            "verified-user",
            &PortfolioRiskPolicy::default(),
            vec![selected_order("account-a"), selected_order("account-b")],
        ).await.unwrap();
        assert_eq!(prepared.len(), 2);
        assert_eq!(submit_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn broker_calculated_margin_shortfall_blocks_the_whole_selected_set() {
        let submit_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let registry = BrokerRegistry::default();
        registry.register(std::sync::Arc::new(MockAdapter { submit_calls: submit_calls.clone() })).unwrap();
        let result = preflight_selected_orders(
            &registry,
            "verified-user",
            &PortfolioRiskPolicy::default(),
            vec![selected_order("account-a"), selected_order("margin-short")],
        ).await;
        assert!(matches!(result, Err(ContractError::InvalidOrder(_))));
        assert_eq!(submit_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn one_failed_selected_account_invalidates_the_complete_preflight_set() {
        let submit_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let registry = BrokerRegistry::default();
        registry.register(std::sync::Arc::new(MockAdapter { submit_calls: submit_calls.clone() })).unwrap();
        let result = preflight_selected_orders(
            &registry,
            "verified-user",
            &PortfolioRiskPolicy::default(),
            vec![selected_order("account-a"), selected_order("unhealthy"), selected_order("disabled")],
        ).await;
        assert!(result.is_err());
        assert_eq!(submit_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
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
            quantity: d("1"), limit_price: None, stop_price: Some(d("90")), time_in_force: None,
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
    fn limit_and_stop_prices_must_follow_the_instrument_tick() {
        let mut spec = instrument(AssetClass::Equity, "ABC", "1", "1");
        spec.tick_size = d("0.05");
        let mut order = NormalizedOrder {
            client_order_id: "tick-order-1".into(), account_id: "acct".into(),
            instrument_id: spec.instrument_id.clone(), side: OrderSide::Buy,
            order_type: OrderType::Limit, quantity: d("1"), limit_price: Some(d("100.05")),
            stop_price: None, time_in_force: None, reduce_only: false,
        };
        assert!(order.validate(&spec).is_ok());
        order.limit_price = Some(d("100.03"));
        assert!(order.validate(&spec).is_err());
    }

    #[test]
    fn normalized_order_checks_instrument_identity_and_type_requirements() {
        let spec = instrument(AssetClass::Equity, "AAPL", "1", "1");
        let order = NormalizedOrder {
            client_order_id: "qoin-unique-1".into(), account_id: "acct".into(),
            instrument_id: spec.instrument_id.clone(), side: OrderSide::Buy,
            order_type: OrderType::Limit, quantity: d("1"), limit_price: Some(d("190")),
            stop_price: None, time_in_force: Some("day".into()), reduce_only: false,
        };
        assert!(order.validate(&spec).is_ok());
    }
}
