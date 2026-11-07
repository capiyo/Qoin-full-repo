//! Forward, event-driven paper execution for the operating-cycle session.
//! This module never submits broker orders. Each configured venue/symbol pair
//! has an isolated quote-currency ledger; capital is never silently shared
//! across instruments or currencies.
use crate::MarketEvent;
use crate::edge_strategy::EdgeDecision;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperAccountConfig {
    pub symbol: String,
    pub venue: String,
    pub quote_currency: String,
    pub initial_cash: f64,
    pub notional_per_trade: f64,
    #[serde(default = "default_multiplier")]
    pub contract_multiplier: f64,
    #[serde(default = "default_quantity_step")]
    pub quantity_step: f64,
    #[serde(default = "default_true")]
    pub reinvest_profits: bool,
    #[serde(default = "default_fee")]
    pub fee_bps: f64,
    #[serde(default = "default_slippage")]
    pub slippage_bps: f64,
}
fn default_multiplier() -> f64 { 1.0 }
fn default_quantity_step() -> f64 { 0.000001 }
fn default_true() -> bool { true }
fn default_fee() -> f64 { 10.0 }
fn default_slippage() -> f64 { 5.0 }

impl PaperAccountConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.symbol.trim().is_empty() || self.venue.trim().is_empty() || self.quote_currency.trim().is_empty() {
            return Err("paper account requires symbol, exact venue, and quote currency".into());
        }
        if !self.initial_cash.is_finite() || self.initial_cash <= 0.0 {
            return Err(format!("{} initial_cash must be positive", self.symbol));
        }
        if !self.notional_per_trade.is_finite() || self.notional_per_trade <= 0.0 || self.notional_per_trade > self.initial_cash {
            return Err(format!("{} notional_per_trade must be positive and no greater than initial_cash", self.symbol));
        }
        if !self.contract_multiplier.is_finite() || self.contract_multiplier <= 0.0
            || !self.quantity_step.is_finite() || self.quantity_step <= 0.0 {
            return Err(format!("{} contract_multiplier and quantity_step must be finite and positive", self.symbol));
        }
        if !self.fee_bps.is_finite() || !(0.0..=100.0).contains(&self.fee_bps)
            || !self.slippage_bps.is_finite() || !(0.0..=100.0).contains(&self.slippage_bps) {
            return Err(format!("{} fee and slippage must be between 0 and 100 bps", self.symbol));
        }
        Ok(())
    }
    fn key(&self) -> String { format!("{}::{}", self.venue, self.symbol) }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct OpenPosition {
    entry_ts_ms: i64,
    entry_event_id: String,
    strategy_version: String,
    quantity: f64,
    entry_price: f64,
    entry_fee: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingAction {
    action: String,
    signal_ts_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaperAccount {
    config: PaperAccountConfig,
    cash: f64,
    realized_pnl: f64,
    fees_paid: f64,
    position: Option<OpenPosition>,
    pending: Option<PendingAction>,
    events: u64,
    decisions: u64,
    buy_signals: u64,
    sell_signals: u64,
    wait_signals: u64,
    entries: u64,
    exits: u64,
    skipped_entries: u64,
    peak_equity: f64,
    max_drawdown_pct: f64,
    last_bid: Option<f64>,
}

impl PaperAccount {
    fn new(config: PaperAccountConfig) -> Self {
        Self {
            cash: config.initial_cash, peak_equity: config.initial_cash, config,
            realized_pnl: 0.0, fees_paid: 0.0, position: None, pending: None,
            events: 0, decisions: 0, buy_signals: 0, sell_signals: 0, wait_signals: 0,
            entries: 0, exits: 0, skipped_entries: 0, max_drawdown_pct: 0.0, last_bid: None,
        }
    }
    fn unrealized_pnl(&self) -> f64 {
        self.position.as_ref().map(|position| {
            let bid = self.last_bid.unwrap_or(position.entry_price);
            let exit_price = bid * (1.0 - self.config.slippage_bps / 10_000.0);
            let exit_fee = position.quantity * exit_price * self.config.contract_multiplier * self.config.fee_bps / 10_000.0;
            position.quantity * (exit_price - position.entry_price) * self.config.contract_multiplier - position.entry_fee - exit_fee
        }).unwrap_or(0.0)
    }
    fn equity(&self) -> f64 {
        let marked = self.position.as_ref().map(|p| {
            let bid = self.last_bid.unwrap_or(p.entry_price);
            let exit = bid * (1.0 - self.config.slippage_bps / 10_000.0);
            let fee = p.quantity * exit * self.config.contract_multiplier * self.config.fee_bps / 10_000.0;
            p.quantity * exit * self.config.contract_multiplier - fee
        }).unwrap_or(0.0);
        self.cash + marked
    }
    fn process(&mut self, event: &MarketEvent, decision: &EdgeDecision, cycle_id: &str) -> (PaperMirrorRecord, Option<PaperTradeRecord>) {
        self.events = self.events.saturating_add(1);
        self.decisions = self.decisions.saturating_add(1);
        match decision.action { "buy" => self.buy_signals += 1, "sell" => self.sell_signals += 1, _ => self.wait_signals += 1 }
        let quote = match (event.bid, event.ask) {
            (Some(bid), Some(ask)) if bid.is_finite() && ask.is_finite() && bid > 0.0 && ask >= bid => {
                self.last_bid = Some(bid); Some((bid, ask))
            }
            _ => None,
        };
        let mut execution = "no_action".to_owned();
        let mut trade = None;
        let mut action_to_execute: Option<PendingAction> = None;
        if let Some(pending) = self.pending.take() {
            let opposite = (pending.action == "buy" && decision.action == "sell")
                || (pending.action == "sell" && decision.action == "buy");
            if opposite {
                execution = format!("cancelled_pending_{}", pending.action);
            } else if quote.is_some() {
                action_to_execute = Some(pending);
            } else {
                self.pending = Some(pending);
            }
        }
        if action_to_execute.is_none() && self.pending.is_none() {
            let eligible = (decision.action == "buy" && self.position.is_none())
                || (decision.action == "sell" && self.position.is_some());
            if eligible {
                if quote.is_some() {
                    action_to_execute = Some(PendingAction { action: decision.action.to_owned(), signal_ts_ms: event.ts_ms });
                } else {
                    self.pending = Some(PendingAction { action: decision.action.to_owned(), signal_ts_ms: event.ts_ms });
                    execution = format!("queued_{}_waiting_for_quote", decision.action);
                }
            }
        }
        if let (Some(action), Some((bid, ask))) = (action_to_execute, quote) {
            if action.action == "buy" && self.position.is_none() {
                let entry_price = ask * (1.0 + self.config.slippage_bps / 10_000.0);
                let raw_notional = if self.config.reinvest_profits {
                    self.config.notional_per_trade * (self.cash / self.config.initial_cash)
                } else { self.config.notional_per_trade };
                let notional = raw_notional.min(self.cash / (1.0 + self.config.fee_bps / 10_000.0));
                let raw_quantity = notional / (entry_price * self.config.contract_multiplier);
                let quantity = (raw_quantity / self.config.quantity_step).floor() * self.config.quantity_step;
                let filled_notional = quantity * entry_price * self.config.contract_multiplier;
                let fee = filled_notional * self.config.fee_bps / 10_000.0;
                let debit = filled_notional + fee;
                if debit <= self.cash && quantity.is_finite() && quantity > 0.0 {
                    self.cash -= debit; self.fees_paid += fee; self.entries += 1;
                    self.position = Some(OpenPosition { entry_ts_ms: event.ts_ms, entry_event_id: event.event_id.clone(), strategy_version: decision.strategy_version.clone(), quantity, entry_price, entry_fee: fee });
                    execution = if action.signal_ts_ms == event.ts_ms { "entered_long" } else { "filled_pending_buy" }.into();
                } else {
                    self.skipped_entries += 1; execution = "skipped_insufficient_cash".into();
                }
            } else if action.action == "sell" {
                if let Some(open) = self.position.take() {
                    let exit_price = bid * (1.0 - self.config.slippage_bps / 10_000.0);
                    let gross = open.quantity * (exit_price - open.entry_price) * self.config.contract_multiplier;
                    let exit_fee = open.quantity * exit_price * self.config.contract_multiplier * self.config.fee_bps / 10_000.0;
                    let total_fees = open.entry_fee + exit_fee;
                    let net = gross - total_fees;
                    self.cash += open.quantity * exit_price * self.config.contract_multiplier - exit_fee;
                    self.fees_paid += exit_fee; self.realized_pnl += net; self.exits += 1;
                    trade = Some(PaperTradeRecord {
                        cycle_id: cycle_id.to_owned(), symbol: self.config.symbol.clone(), venue: self.config.venue.clone(),
                        quote_currency: self.config.quote_currency.to_uppercase(), strategy_version: open.strategy_version.clone(),
                        entry_event_id: open.entry_event_id.clone(), exit_event_id: event.event_id.clone(), entry_ts_ms: open.entry_ts_ms,
                        exit_ts_ms: event.ts_ms, quantity: open.quantity, contract_multiplier: self.config.contract_multiplier,
                        quantity_step: self.config.quantity_step, fee_bps: self.config.fee_bps, slippage_bps: self.config.slippage_bps,
                        reinvest_profits: self.config.reinvest_profits, entry_price: open.entry_price,
                        exit_price, gross_pnl: gross, fees: total_fees, net_pnl: net,
                        return_pct: net / (open.quantity * open.entry_price * self.config.contract_multiplier) * 100.0,
                        exit_reason: "strategy_sell".into(),
                    });
                    execution = if action.signal_ts_ms == event.ts_ms { "exited_long" } else { "filled_pending_sell" }.into();
                }
            }
        }
        let equity = self.equity();
        self.peak_equity = self.peak_equity.max(equity);
        if self.peak_equity > 0.0 {
            self.max_drawdown_pct = self.max_drawdown_pct.max((self.peak_equity - equity) / self.peak_equity * 100.0);
        }
        let spread_bps = match (event.bid, event.ask) {
            (Some(bid), Some(ask)) if bid + ask > 0.0 => Some((ask - bid) / ((ask + bid) / 2.0) * 10_000.0),
            _ => None,
        };
        let record = PaperMirrorRecord {
            cycle_id: cycle_id.to_owned(), event_id: event.event_id.clone(), venue: event.venue.clone(),
            symbol: event.symbol.clone(), quote_currency: self.config.quote_currency.to_uppercase(),
            contract_multiplier: self.config.contract_multiplier, quantity_step: self.config.quantity_step,
            notional_per_trade: self.config.notional_per_trade, reinvest_profits: self.config.reinvest_profits,
            fee_bps: self.config.fee_bps, slippage_bps: self.config.slippage_bps,
            ts_ms: event.ts_ms, strategy_version: decision.strategy_version.clone(),
            bullish_weights: BTreeMap::new(), bearish_weights: BTreeMap::new(), action: decision.action.into(),
            reason: decision.reason.clone(), features: decision.features.clone(), attention: decision.attention.clone(),
            attended_features: decision.attended_features.clone(), decision_vector: decision.decision_vector,
            magnitude: decision.magnitude, angle_radians: decision.angle_radians, spread_bps, execution,
            equity, cash: self.cash, realized_pnl: self.realized_pnl, fees_paid: self.fees_paid,
            open_position: self.position.is_some(),
        };
        (record, trade)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMirrorRecord {
    pub cycle_id: String, pub event_id: String, pub venue: String, pub symbol: String,
    pub quote_currency: String, pub contract_multiplier: f64, pub quantity_step: f64,
    pub notional_per_trade: f64, pub reinvest_profits: bool, pub fee_bps: f64, pub slippage_bps: f64,
    pub ts_ms: i64, pub strategy_version: String, pub bullish_weights: BTreeMap<String, f64>, pub bearish_weights: BTreeMap<String, f64>, pub action: String,
    pub reason: String, pub features: BTreeMap<String, f64>, pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>, pub decision_vector: [f64; 2], pub magnitude: f64,
    pub angle_radians: f64, pub spread_bps: Option<f64>, pub execution: String,
    pub equity: f64, pub cash: f64, pub realized_pnl: f64, pub fees_paid: f64, pub open_position: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradeRecord {
    pub cycle_id: String, pub symbol: String, pub venue: String, pub quote_currency: String,
    pub strategy_version: String, pub entry_event_id: String, pub exit_event_id: String,
    pub entry_ts_ms: i64, pub exit_ts_ms: i64, pub quantity: f64, pub contract_multiplier: f64,
    pub quantity_step: f64, pub fee_bps: f64, pub slippage_bps: f64, pub reinvest_profits: bool, pub entry_price: f64,
    pub exit_price: f64, pub gross_pnl: f64, pub fees: f64, pub net_pnl: f64,
    pub return_pct: f64, pub exit_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaperOpenPositionSnapshot {
    pub entry_ts_ms: i64, pub entry_event_id: String, pub strategy_version: String, pub quantity: f64, pub contract_multiplier: f64, pub entry_price: f64, pub entry_fee: f64,
    pub unrealized_pnl: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaperAccountSnapshot {
    pub symbol: String, pub venue: String, pub quote_currency: String,
    pub contract_multiplier: f64, pub quantity_step: f64,
    pub initial_cash: f64, pub cash: f64, pub equity: f64, pub net_pnl: f64,
    pub realized_pnl: f64, pub unrealized_pnl: f64, pub fees_paid: f64, pub max_drawdown_pct: f64,
    pub events: u64, pub decisions: u64, pub buy_signals: u64, pub sell_signals: u64,
    pub wait_signals: u64, pub entries: u64, pub exits: u64, pub skipped_entries: u64,
    pub has_open_position: bool, pub open_position: Option<PaperOpenPositionSnapshot>, pub pending_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaperSessionSnapshot {
    pub cycle_id: Option<String>, pub active: bool, pub accounts: Vec<PaperAccountSnapshot>,
    pub live_execution_enabled: bool, pub note: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaperSession {
    cycle_id: Option<String>,
    active: bool,
    accounts: BTreeMap<String, PaperAccount>,
}

impl PaperSession {
    pub fn start(cycle_id: String, configs: &[PaperAccountConfig]) -> Result<Self, String> {
        if configs.is_empty() { return Err("add at least one paper account before starting a paper cycle".into()); }
        if configs.len() > 50 { return Err("a paper cycle supports at most 50 configured instruments".into()); }
        let mut accounts = BTreeMap::new();
        for config in configs {
            config.validate()?;
            let key = config.key();
            if accounts.contains_key(&key) { return Err(format!("duplicate paper account for {}", key)); }
            accounts.insert(key, PaperAccount::new(config.clone()));
        }
        Ok(Self { cycle_id: Some(cycle_id), active: true, accounts })
    }
    pub fn stop(&mut self) { self.active = false; }
    pub fn is_active(&self) -> bool { self.active }
    pub fn process_event(&mut self, event: &MarketEvent, decision: &EdgeDecision) -> Option<(PaperMirrorRecord, Option<PaperTradeRecord>)> {
        if !self.active { return None; }
        let key = format!("{}::{}", event.venue, event.symbol);
        let account = self.accounts.get_mut(&key)?;
        let cycle_id = self.cycle_id.as_deref().unwrap_or("unknown");
        Some(account.process(event, decision, cycle_id))
    }
    pub fn snapshot(&self) -> PaperSessionSnapshot {
        let accounts = self.accounts.values().map(|account| {
            let equity = account.equity();
            PaperAccountSnapshot {
                symbol: account.config.symbol.clone(), venue: account.config.venue.clone(),
                quote_currency: account.config.quote_currency.to_uppercase(),
                contract_multiplier: account.config.contract_multiplier, quantity_step: account.config.quantity_step,
                initial_cash: account.config.initial_cash, cash: account.cash, equity,
                net_pnl: equity - account.config.initial_cash, realized_pnl: account.realized_pnl,
                unrealized_pnl: account.unrealized_pnl(), fees_paid: account.fees_paid, max_drawdown_pct: account.max_drawdown_pct,
                events: account.events, decisions: account.decisions, buy_signals: account.buy_signals,
                sell_signals: account.sell_signals, wait_signals: account.wait_signals,
                entries: account.entries, exits: account.exits, skipped_entries: account.skipped_entries,
                has_open_position: account.position.is_some(),
                open_position: account.position.as_ref().map(|position| PaperOpenPositionSnapshot {
                    entry_ts_ms: position.entry_ts_ms, entry_event_id: position.entry_event_id.clone(), strategy_version: position.strategy_version.clone(),
                    quantity: position.quantity, contract_multiplier: account.config.contract_multiplier, entry_price: position.entry_price,
                    entry_fee: position.entry_fee, unrealized_pnl: account.unrealized_pnl(),
                }),
                pending_action: account.pending.as_ref().map(|pending| pending.action.clone()),
            }
        }).collect();
        PaperSessionSnapshot {
            cycle_id: self.cycle_id.clone(), active: self.active, accounts,
            live_execution_enabled: false,
            note: "Isolated quote-currency paper ledgers; marketable fills use observed bid/ask plus configured slippage and fees. No broker orders are sent.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> PaperAccountConfig {
        PaperAccountConfig { symbol: "BTCUSDT".into(), venue: "venue-a".into(), quote_currency: "USDT".into(),
            initial_cash: 20_000.0, notional_per_trade: 2_000.0, contract_multiplier: 1.0, quantity_step: 0.000001, reinvest_profits: true, fee_bps: 10.0, slippage_bps: 5.0 }
    }
    fn event(ts_ms: i64, bid: Option<f64>, ask: Option<f64>) -> MarketEvent {
        MarketEvent { event_id: format!("e{ts_ms}"), venue: "venue-a".into(), symbol: "BTCUSDT".into(),
            event_type: "book".into(), ts_ms, price: None, size: None, side: None, bid, ask,
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None, liquidation_side: None }
    }
    fn decision(action: &'static str) -> EdgeDecision {
        EdgeDecision { symbol: "BTCUSDT".into(), strategy_key: "active".into(), strategy_version: "v1".into(),
            features: BTreeMap::new(), attention: vec![], attended_features: vec![], decision_vector: [0.8, 0.1],
            magnitude: 0.8, angle_radians: 0.1, angular_threshold_radians: 0.4, action,
            reason: "test".into(), research_only: true }
    }
    #[test]
    fn paper_account_fills_only_on_observed_two_sided_quote_and_closes_on_sell() {
        let mut session = PaperSession::start("cycle-1".into(), &[config()]).unwrap();
        let (entry, trade) = session.process_event(&event(1000, Some(99.9), Some(100.1)), &decision("buy")).unwrap();
        assert_eq!(entry.execution, "entered_long");
        assert!(trade.is_none());
        let (exit, trade) = session.process_event(&event(2000, Some(101.0), Some(101.2)), &decision("sell")).unwrap();
        assert_eq!(exit.execution, "exited_long");
        assert!(trade.unwrap().net_pnl.is_finite());
        let snapshot = session.snapshot();
        assert_eq!(snapshot.accounts[0].entries, 1);
        assert_eq!(snapshot.accounts[0].exits, 1);
        assert!(!snapshot.accounts[0].has_open_position);
    }
    #[test]
    fn no_quote_queues_signal_and_next_valid_quote_fills() {
        let mut session = PaperSession::start("cycle-1".into(), &[config()]).unwrap();
        let (queued, _) = session.process_event(&event(1000, None, None), &decision("buy")).unwrap();
        assert_eq!(queued.execution, "queued_buy_waiting_for_quote");
        let (filled, _) = session.process_event(&event(2000, Some(99.9), Some(100.1)), &decision("wait")).unwrap();
        assert_eq!(filled.execution, "filled_pending_buy");
    }
    #[test]
    fn applies_contract_multiplier_and_rounds_to_venue_quantity_step() {
        let mut instrument = config();
        instrument.contract_multiplier = 10.0;
        instrument.quantity_step = 0.5;
        let mut session = PaperSession::start("cycle-multiplier".into(), &[instrument]).unwrap();
        session.process_event(&event(1_000, Some(99.9), Some(100.1)), &decision("buy")).unwrap();
        let (_, trade) = session.process_event(&event(2_000, Some(101.0), Some(101.2)), &decision("sell")).unwrap();
        let trade = trade.unwrap();
        assert_eq!(trade.quantity, 1.5);
        assert_eq!(trade.contract_multiplier, 10.0);
        assert!((trade.gross_pnl - 11.99175).abs() < 1e-8);
        let snapshot = session.snapshot();
        assert!((snapshot.accounts[0].cash - (20_000.0 + trade.net_pnl)).abs() < 1e-8);
    }

    #[test]
    fn rejects_duplicate_and_overallocated_accounts() {
        assert!(PaperSession::start("cycle".into(), &[config(), config()]).is_err());
        let mut invalid = config(); invalid.notional_per_trade = 20_001.0;
        assert!(PaperSession::start("cycle".into(), &[invalid]).is_err());
    }
}
