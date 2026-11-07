//! Deterministic chronological paper backtest using the same EdgeStrategy::decide path
//! as live research ingestion. This module never submits orders.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::{apply, MarketEvent, SymbolState};
use crate::edge_strategy::EdgeStrategy;

fn default_max_events() -> usize { 100_000 }
fn default_warmup_hours() -> u64 { 24 }

#[derive(Debug, Clone, Deserialize)]
pub struct BacktestRequest {
    pub symbol: String,
    pub venue: String,
    pub start_ts_ms: i64,
    pub end_ts_ms: i64,
    #[serde(default = "default_warmup_hours")]
    pub warmup_hours: u64,
    pub initial_cash: f64,
    pub notional_per_trade: f64,
    #[serde(default = "default_fee_bps")]
    pub fee_bps: f64,
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: f64,
    #[serde(default = "default_max_events")]
    pub max_events: usize,
}
fn default_fee_bps() -> f64 { 10.0 }
fn default_slippage_bps() -> f64 { 5.0 }

impl BacktestRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.symbol.trim().is_empty() { return Err("symbol is required".into()); }
        if self.venue.trim().is_empty() { return Err("venue is required; replay one data venue at a time".into()); }
        if self.start_ts_ms <= 0 || self.end_ts_ms <= self.start_ts_ms {
            return Err("end_ts_ms must be greater than a positive start_ts_ms".into());
        }
        if self.warmup_hours == 0 || self.warmup_hours > 168 {
            return Err("warmup_hours must be between 1 and 168".into());
        }
        if !self.initial_cash.is_finite() || self.initial_cash <= 0.0 {
            return Err("initial_cash must be finite and positive".into());
        }
        if !self.notional_per_trade.is_finite() || self.notional_per_trade <= 0.0
            || self.notional_per_trade > self.initial_cash {
            return Err("notional_per_trade must be positive and no greater than initial_cash".into());
        }
        if !self.fee_bps.is_finite() || !(0.0..=100.0).contains(&self.fee_bps) {
            return Err("fee_bps must be between 0 and 100".into());
        }
        if !self.slippage_bps.is_finite() || !(0.0..=100.0).contains(&self.slippage_bps) {
            return Err("slippage_bps must be between 0 and 100".into());
        }
        if self.max_events == 0 || self.max_events > 100_000 {
            return Err("max_events must be between 1 and 100000".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulatedTrade {
    pub entry_ts_ms: i64,
    pub exit_ts_ms: Option<i64>,
    pub quantity: f64,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub gross_pnl: Option<f64>,
    pub fees: f64,
    pub net_pnl: Option<f64>,
    pub return_pct: Option<f64>,
    pub exit_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MirrorDecision {
    pub sequence: usize,
    pub event_id: String,
    pub venue: String,
    pub ts_ms: i64,
    pub strategy_version: String,
    pub action: String,
    pub reason: String,
    pub features: BTreeMap<String, f64>,
    pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>,
    pub decision_vector: [f64; 2],
    pub magnitude: f64,
    pub angle_radians: f64,
    pub spread_bps: Option<f64>,
    pub execution: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BacktestReport {
    pub run_id: String,
    pub symbol: String,
    pub venue: String,
    pub strategy_version: String,
    pub start_ts_ms: i64,
    pub end_ts_ms: i64,
    pub initial_cash: f64,
    pub final_equity: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub net_pnl: f64,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub event_count: usize,
    pub warmup_event_count: usize,
    pub signal_count: usize,
    pub buy_signals: usize,
    pub sell_signals: usize,
    pub wait_signals: usize,
    pub executed_entries: usize,
    pub executed_exits: usize,
    pub skipped_entries: usize,
    pub win_rate_pct: Option<f64>,
    pub profit_factor: Option<f64>,
    pub fees_paid: f64,
    pub assumptions: String,
    pub open_position: Option<SimulatedTrade>,
    pub trades: Vec<SimulatedTrade>,
    pub decisions: Vec<MirrorDecision>,
}

#[derive(Debug, Clone)]
struct OpenPosition {
    entry_ts_ms: i64,
    quantity: f64,
    entry_price: f64,
    entry_fee: f64,
}

pub fn run_backtest(
    request: &BacktestRequest,
    events: &[MarketEvent],
    strategy: &EdgeStrategy,
    run_id: String,
) -> Result<BacktestReport, String> {
    request.validate()?;
    strategy.validate()?;
    let mut ordered = events.to_vec();
    ordered.sort_by(|a, b| a.ts_ms.cmp(&b.ts_ms).then_with(|| a.event_id.cmp(&b.event_id)));
    let mut state = SymbolState::default();
    let mut cash = request.initial_cash;
    let mut realized_pnl = 0.0;
    let mut fees_paid = 0.0;
    let mut position: Option<OpenPosition> = None;
    let mut trades: Vec<SimulatedTrade> = Vec::new();
    let mut decisions: Vec<MirrorDecision> = Vec::new();
    let mut signal_count = 0usize;
    let mut buy_signals = 0usize;
    let mut sell_signals = 0usize;
    let mut wait_signals = 0usize;
    let mut executed_entries = 0usize;
    let mut executed_exits = 0usize;
    let mut skipped_entries = 0usize;
    let mut peak_equity = request.initial_cash;
    let mut max_drawdown = 0.0_f64;
    let mut last_bid: Option<f64> = None;

    let mut warmup_event_count = 0usize;
    for (index, event) in ordered.iter().enumerate() {
        let warmup_start = request.start_ts_ms.saturating_sub((request.warmup_hours as i64).saturating_mul(3_600_000));
        if event.symbol != request.symbol || event.venue != request.venue || event.ts_ms < warmup_start || event.ts_ms > request.end_ts_ms {
            continue;
        }
        apply(&mut state, event);
        if event.ts_ms < request.start_ts_ms {
            warmup_event_count += 1;
            continue;
        }
        if let (Some(bid), Some(ask)) = (event.bid, event.ask) {
            if bid.is_finite() && ask.is_finite() && bid > 0.0 && ask >= bid {
                last_bid = Some(bid);
            }
        }
        let decision = strategy.decide(
            &request.symbol,
            state.order_flow_imbalance,
            state.funding_rate,
            state.spread_bps,
            state.ewma_abs_return,
            state.event_intensity,
            state.liquidation_events,
            state.last_liquidation_side.as_deref(),
        )?;
        signal_count += 1;
        match decision.action { "buy" => buy_signals += 1, "sell" => sell_signals += 1, _ => wait_signals += 1 }
        let mut execution = "no_action".to_owned();
        let fresh_quote = match (event.bid, event.ask) {
            (Some(bid), Some(ask)) if bid.is_finite() && ask.is_finite() && bid > 0.0 && ask >= bid => Some((bid, ask)),
            _ => None,
        };
        if decision.action == "buy" && position.is_none() {
            if let Some((_bid, ask)) = fresh_quote {
                let entry_price = ask * (1.0 + request.slippage_bps / 10_000.0);
                let quantity = request.notional_per_trade / entry_price;
                let fee = quantity * entry_price * request.fee_bps / 10_000.0;
                let debit = quantity * entry_price + fee;
                if debit <= cash {
                    cash -= debit;
                    fees_paid += fee;
                    position = Some(OpenPosition { entry_ts_ms: event.ts_ms, quantity, entry_price, entry_fee: fee });
                    executed_entries += 1;
                    execution = "entered_long".into();
                } else {
                    skipped_entries += 1;
                    execution = "skipped_insufficient_cash".into();
                }
            } else {
                skipped_entries += 1;
                execution = "skipped_no_fresh_bid_ask".into();
            }
        } else if decision.action == "sell" && position.is_some() {
            if let Some((bid, _ask)) = fresh_quote {
                let open = position.take().expect("position checked above");
                let exit_price = bid * (1.0 - request.slippage_bps / 10_000.0);
                let gross = open.quantity * (exit_price - open.entry_price);
                let exit_fee = open.quantity * exit_price * request.fee_bps / 10_000.0;
                let total_fees = open.entry_fee + exit_fee;
                let net = gross - total_fees;
                cash += open.quantity * exit_price - exit_fee;
                fees_paid += exit_fee;
                realized_pnl += net;
                trades.push(SimulatedTrade {
                    entry_ts_ms: open.entry_ts_ms, exit_ts_ms: Some(event.ts_ms),
                    quantity: open.quantity, entry_price: open.entry_price,
                    exit_price: Some(exit_price), gross_pnl: Some(gross), fees: total_fees,
                    net_pnl: Some(net), return_pct: Some(net / (open.quantity * open.entry_price) * 100.0),
                    exit_reason: Some("strategy_sell".into()),
                });
                executed_exits += 1;
                execution = "exited_long".into();
            } else {
                execution = "sell_waiting_for_fresh_bid_ask".into();
            }
        }
        decisions.push(MirrorDecision {
            sequence: index, event_id: event.event_id.clone(), venue: event.venue.clone(), ts_ms: event.ts_ms,
            strategy_version: decision.strategy_version.clone(),
            action: decision.action.into(), reason: decision.reason,
            features: decision.features.clone(), attention: decision.attention.clone(),
            attended_features: decision.attended_features.clone(), decision_vector: decision.decision_vector,
            magnitude: decision.magnitude, angle_radians: decision.angle_radians,
            spread_bps: state.spread_bps, execution,
        });
        let marked_position = position.as_ref().map(|open| {
            let bid = last_bid.unwrap_or(open.entry_price);
            let liquidation_price = bid * (1.0 - request.slippage_bps / 10_000.0);
            let estimated_exit_fee = open.quantity * liquidation_price * request.fee_bps / 10_000.0;
            open.quantity * liquidation_price - estimated_exit_fee
        }).unwrap_or(0.0);
        let equity = cash + marked_position;
        peak_equity = peak_equity.max(equity);
        if peak_equity > 0.0 {
            max_drawdown = max_drawdown.max((peak_equity - equity) / peak_equity);
        }
    }

    let unrealized_pnl = position.as_ref().map(|open| {
        let bid = last_bid.unwrap_or(open.entry_price);
        let liquidation_price = bid * (1.0 - request.slippage_bps / 10_000.0);
        let estimated_exit_fee = open.quantity * liquidation_price * request.fee_bps / 10_000.0;
        open.quantity * (liquidation_price - open.entry_price) - open.entry_fee - estimated_exit_fee
    }).unwrap_or(0.0);
    let final_equity = cash + position.as_ref().map(|open| {
        let bid = last_bid.unwrap_or(open.entry_price);
        let liquidation_price = bid * (1.0 - request.slippage_bps / 10_000.0);
        let estimated_exit_fee = open.quantity * liquidation_price * request.fee_bps / 10_000.0;
        open.quantity * liquidation_price - estimated_exit_fee
    }).unwrap_or(0.0);
    let net_pnl = final_equity - request.initial_cash;
    let closed = trades.iter().filter_map(|trade| trade.net_pnl).collect::<Vec<_>>();
    let winners = closed.iter().filter(|pnl| **pnl > 0.0).count();
    let gross_profit: f64 = closed.iter().filter(|pnl| **pnl > 0.0).sum();
    let gross_loss: f64 = closed.iter().filter(|pnl| **pnl < 0.0).map(|pnl| pnl.abs()).sum();
    let open_position = position.as_ref().map(|open| SimulatedTrade {
        entry_ts_ms: open.entry_ts_ms, exit_ts_ms: None, quantity: open.quantity,
        entry_price: open.entry_price, exit_price: None, gross_pnl: None,
        fees: open.entry_fee, net_pnl: None, return_pct: None, exit_reason: None,
    });
    Ok(BacktestReport {
        run_id, symbol: request.symbol.clone(), venue: request.venue.clone(), strategy_version: strategy.version.clone(),
        start_ts_ms: request.start_ts_ms, end_ts_ms: request.end_ts_ms,
        initial_cash: request.initial_cash, final_equity, realized_pnl, unrealized_pnl,
        net_pnl, return_pct: net_pnl / request.initial_cash * 100.0,
        max_drawdown_pct: max_drawdown * 100.0, event_count: ordered.iter().filter(|e|
            e.symbol == request.symbol && e.venue == request.venue && e.ts_ms >= request.start_ts_ms && e.ts_ms <= request.end_ts_ms).count(),
        warmup_event_count, signal_count, buy_signals, sell_signals, wait_signals, executed_entries, executed_exits,
        skipped_entries, win_rate_pct: if closed.is_empty() { None } else { Some(winners as f64 / closed.len() as f64 * 100.0) },
        profit_factor: if gross_loss > 0.0 { Some(gross_profit / gross_loss) } else { None },
        fees_paid, assumptions: "Long-only; one position at a time; fixed cash notional per entry; marketable fills at observed ask/bid plus configured slippage; fees charged on both sides; open positions are marked to estimated liquidation value including exit slippage and fee; no leverage, no shorting, no stop-loss, no financing/funding debit, no partial fills. Only events carrying a fresh valid bid and ask can fill. Results depend on data completeness and supplied cost assumptions.".into(),
        open_position, trades, decisions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_future_or_invalid_cost_inputs() {
        let request = BacktestRequest { symbol: "BTCUSDT".into(), venue: "test".into(), start_ts_ms: 10, end_ts_ms: 20,
            warmup_hours: 24, initial_cash: 1000.0, notional_per_trade: 100.0, fee_bps: 101.0, slippage_bps: 5.0, max_events: 100 };
        assert!(request.validate().is_err());
    }

    #[test]
    fn empty_history_returns_flat_equity_without_inventing_trades() {
        let request = BacktestRequest { symbol: "BTCUSDT".into(), venue: "test".into(), start_ts_ms: 10, end_ts_ms: 20,
            warmup_hours: 24, initial_cash: 20_000.0, notional_per_trade: 2_000.0, fee_bps: 10.0, slippage_bps: 5.0, max_events: 100 };
        let report = run_backtest(&request, &[], &EdgeStrategy::baseline(), "empty-run".into()).unwrap();
        assert_eq!(report.event_count, 0);
        assert_eq!(report.final_equity, 20_000.0);
        assert_eq!(report.net_pnl, 0.0);
        assert!(report.trades.is_empty());
        assert!(report.decisions.is_empty());
    }
}
