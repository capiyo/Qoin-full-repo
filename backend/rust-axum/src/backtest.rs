use serde::{Deserialize, Serialize};
#[cfg(test)]
use crate::edge_strategy::EdgeStrategy;
use qoin_vector_engine::{decide as decide_vector, EngineConfig, ForceObservation};

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayEvent {
    pub event_id: String, pub venue: String, pub symbol: String, pub event_type: String, pub ts_ms: i64,
    pub price: Option<f64>, pub size: Option<f64>, pub side: Option<String>,
    pub bid: Option<f64>, pub ask: Option<f64>, pub bid_size: Option<f64>, pub ask_size: Option<f64>,
    pub funding_rate: Option<f64>, pub open_interest: Option<f64>, pub liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Default)]
struct ReplayState {
    last_price: Option<f64>, last_mark_price: Option<f64>, basis_bps: Option<f64>, spread_bps: Option<f64>, last_book_ts_ms: Option<i64>,
    last_event_ts_ms: Option<i64>, last_trade_ts_ms: Option<i64>, liquidity_imbalance: f64, signed_flow: f64, total_trade_size: f64,
    ofi: f64, event_intensity: f64, ewma_abs_return: f64, funding_rate: Option<f64>,
    open_interest: Option<f64>, open_interest_change: f64, liquidation_pressure: f64,
    liquidation_events: usize, last_liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayOrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayPendingOrder {
    pub order_id: String,
    pub status: ReplayOrderStatus,
    pub action: String,
    pub decision_ts_ms: i64,
    pub decision_event_id: String,
    pub submitted_ts_ms: i64,
    pub submitted_event_id: String,
    pub target_quantity: Option<f64>,
    pub filled_quantity: f64,
    pub remaining_quantity: Option<f64>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayFill {
    pub order_id: String,
    pub status: ReplayOrderStatus,
    pub ts_ms: i64, pub event_id: String, pub action: String, pub price: f64,
    pub quantity: f64, pub fee: f64, pub slippage_cost: f64, pub realized_pnl: f64,
    pub equity_after: f64, pub reason: String,
    pub decision_ts_ms: i64, pub decision_event_id: String,
    pub submitted_ts_ms: i64, pub submitted_event_id: String,
    pub target_quantity: f64, pub filled_quantity: f64, pub remaining_quantity: f64,
    pub market_impact_bps: f64, pub market_impact_cost: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionStressScenario {
    pub name: String,
    pub cost_multiplier: f64,
    pub fees_paid: f64,
    pub slippage_paid: f64,
    pub market_impact_paid: f64,
    pub estimated_net_pnl: f64,
    pub estimated_return_pct: f64,
    pub fills_recomputed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayDecision {
    pub action: String,
    pub reason: String,
    pub score: Option<f64>,
    pub alignment: Option<f64>,
    pub angle_radians: Option<f64>,
    pub feature_trace: Option<Vec<qoin_vector_engine::FeatureTrace>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayResult {
    pub symbol: String, pub venue: String, pub strategy_key: String, pub strategy_version: String,
    pub from_ts_ms: i64, pub to_ts_ms: i64, pub events_replayed: usize,
    pub max_event_gap_ms: i64, pub event_gaps_over_5m: usize,
    pub decisions: usize, pub buy_decisions: usize, pub sell_decisions: usize,
    pub wait_decisions: usize, pub initial_capital: f64, pub final_equity: f64,
    pub net_pnl: f64, pub net_return_pct: f64, pub max_drawdown_pct: f64,
    pub annualized_return_pct: Option<f64>, pub volatility_pct: Option<f64>,
    pub sharpe_ratio: Option<f64>, pub sortino_ratio: Option<f64>, pub calmar_ratio: Option<f64>,
    pub win_rate_pct: Option<f64>, pub profit_factor: Option<f64>,
    pub winning_trade_pnl: f64, pub losing_trade_pnl: f64, pub completed_trades: usize,
    pub max_gross_exposure_pct: f64, pub transaction_costs: f64,
    pub buy_and_hold_return_pct: Option<f64>,
    pub fees_paid: f64, pub slippage_paid: f64, pub net_funding_cashflow: f64, pub funding_settlements: usize,
    pub fee_bps: f64, pub slippage_bps: f64, pub latency_ms: i64,
    pub max_participation_rate: f64, pub market_impact_bps: f64, pub market_impact_paid: f64,
    pub fills: Vec<ReplayFill>, pub last_decision: Option<ReplayDecision>,
    pub unfilled_pending_order: Option<ReplayPendingOrder>,
    pub execution_stress_scenarios: Vec<ExecutionStressScenario>,
    pub assumptions: Vec<String>,
}
fn funding_cashflow(position: f64, mark_price: f64, funding_rate: f64) -> f64 {
    if position <= 0.0 || mark_price <= 0.0 { return 0.0; }
    -position * mark_price * funding_rate
}

fn event_mark_price(event: &ReplayEvent) -> Option<f64> {
    match event.event_type.as_str() {
        "trade" | "mark_price" => event.price.filter(|price| price.is_finite() && *price > 0.0),
        "book" => match (event.bid, event.ask) {
            (Some(bid), Some(ask)) if bid.is_finite() && ask.is_finite() && bid > 0.0 && ask >= bid => {
                let mid = (bid + ask) / 2.0;
                (mid.is_finite() && mid > 0.0).then_some(mid)
            }
            _ => None,
        },
        _ => None,
    }
}
fn event_execution_price(event: &ReplayEvent, action: &str) -> Option<f64> {
    let price = match (event.event_type.as_str(), action) {
        ("trade", "buy" | "sell") => event.price,
        ("book", "buy") => event.ask,
        ("book", "sell") => event.bid,
        _ => None,
    };
    price.filter(|value| value.is_finite() && *value > 0.0)
}

fn event_available_liquidity(event: &ReplayEvent, action: &str) -> Option<f64> {
    let quantity = match (event.event_type.as_str(), action) {
        ("trade", "buy" | "sell") => event.size,
        ("book", "buy") => event.ask_size,
        ("book", "sell") => event.bid_size,
        _ => None,
    };
    quantity.filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(test)]
pub fn replay(events: Vec<ReplayEvent>, strategy: &EdgeStrategy, initial_capital: f64,
              fee_bps: f64, slippage_bps: f64) -> Result<ReplayResult, String> {
    replay_with_execution(events, strategy, initial_capital, fee_bps, slippage_bps, 0, 0.10, 5.0)
}

#[cfg(test)]
pub fn replay_with_latency(events: Vec<ReplayEvent>, strategy: &EdgeStrategy, initial_capital: f64,
              fee_bps: f64, slippage_bps: f64, latency_ms: i64) -> Result<ReplayResult, String> {
    replay_with_execution(events, strategy, initial_capital, fee_bps, slippage_bps, latency_ms, 0.10, 5.0)
}

#[cfg(test)]
pub fn replay_with_execution(events: Vec<ReplayEvent>, strategy: &EdgeStrategy, initial_capital: f64,
              fee_bps: f64, slippage_bps: f64, latency_ms: i64,
              max_participation_rate: f64, market_impact_bps: f64) -> Result<ReplayResult, String> {
    strategy.validate()?;
    replay_with_decider(events, strategy.strategy_key.clone(), strategy.version.clone(),
        |symbol, state, _event| {
            let decision = strategy.decide(symbol, state.ofi, state.funding_rate, state.spread_bps,
                state.ewma_abs_return, state.event_intensity, state.liquidation_events,
                state.last_liquidation_side.as_deref())?;
            Ok(ReplayDecision { action: decision.action.to_owned(), reason: decision.reason,
                score: None, alignment: None, angle_radians: None, feature_trace: None })
        }, initial_capital, fee_bps, slippage_bps, latency_ms, max_participation_rate, market_impact_bps)
}

/// Run the identical execution simulator with the authoritative seven-force
/// engine used by live event ingestion. This keeps research replay and live
/// signal calculations on one versioned decision implementation.
pub fn replay_with_vector_engine(events: Vec<ReplayEvent>, config: &EngineConfig, config_version: &str,
              initial_capital: f64, fee_bps: f64, slippage_bps: f64, latency_ms: i64,
              max_participation_rate: f64, market_impact_bps: f64) -> Result<ReplayResult, String> {
    replay_with_decider(events, "qoin-vector-baseline".to_owned(), config_version.to_owned(),
        |_symbol, state, _event| {
            let observation = ForceObservation {
                ofi: state.ofi,
                liquidity: state.liquidity_imbalance,
                volatility: state.ewma_abs_return / 0.01,
                funding: state.funding_rate.unwrap_or(0.0) * 100.0,
                open_interest_change: state.open_interest_change * 100.0,
                liquidation_pressure: state.liquidation_pressure / 10.0,
                basis: state.basis_bps.map(|basis| basis / 25.0).unwrap_or(0.0),
                spread_bps: state.spread_bps,
                spread_age_ms: state.last_book_ts_ms.map(|book_ts| _event.ts_ms.saturating_sub(book_ts)),
            };
            let signal = decide_vector(observation, config, config_version, false)
                .map_err(|error| error.to_string())?;
            Ok(ReplayDecision { action: signal.action, reason: signal.rationale,
                score: Some(signal.score), alignment: Some(signal.alignment),
                angle_radians: Some(signal.angle_radians), feature_trace: Some(signal.feature_trace) })
        }, initial_capital, fee_bps, slippage_bps, latency_ms, max_participation_rate, market_impact_bps)
}

fn replay_with_decider<F>(mut events: Vec<ReplayEvent>, strategy_key: String, strategy_version: String,
              mut decide: F, initial_capital: f64, fee_bps: f64, slippage_bps: f64, latency_ms: i64,
              max_participation_rate: f64, market_impact_bps: f64) -> Result<ReplayResult, String>
where F: FnMut(&str, &ReplayState, &ReplayEvent) -> Result<ReplayDecision, String> {
    if !initial_capital.is_finite() || initial_capital <= 0.0 || !fee_bps.is_finite()
        || fee_bps < 0.0 || !slippage_bps.is_finite() || !(0.0..10_000.0).contains(&slippage_bps)
        || !(0..=60_000).contains(&latency_ms) || !max_participation_rate.is_finite()
        || max_participation_rate <= 0.0 || max_participation_rate > 1.0
        || !market_impact_bps.is_finite() || market_impact_bps < 0.0 || market_impact_bps > 1_000.0
        || slippage_bps + market_impact_bps >= 10_000.0 {
        return Err("capital must be positive; fee/slippage must be finite and non-negative; latency must be between 0 and 60000 ms; max participation must be in (0, 1]; market impact must be in [0, 1000] bps and combined slippage/impact must stay below 10000 bps".into());
    }
    if events.is_empty() { return Err("no persisted market events in the requested replay window".into()); }
    events.sort_by(|a,b| a.ts_ms.cmp(&b.ts_ms).then_with(||a.event_id.cmp(&b.event_id)));
    let symbol=events[0].symbol.clone();
    if events.iter().any(|e| e.symbol != symbol) { return Err("replay events must contain exactly one canonical symbol".into()); }
    let venue = events[0].venue.clone();
    if venue.trim().is_empty() || events.iter().any(|e| e.venue != venue) {
        return Err("replay events must contain exactly one non-empty venue; select a venue before replaying mixed-provider symbols".into());
    }
    if events.iter().any(|e| {
        e.ts_ms <= 0
            || [e.price, e.size, e.bid, e.ask, e.bid_size, e.ask_size, e.funding_rate, e.open_interest]
                .into_iter().flatten().any(|v| !v.is_finite())
            || e.price.map(|v| v <= 0.0).unwrap_or(false)
            || [e.size, e.bid_size, e.ask_size, e.open_interest].into_iter().flatten().any(|v| v < 0.0)
            || matches!((e.bid, e.ask), (Some(bid), Some(ask)) if bid <= 0.0 || ask < bid)
    }) {
        return Err("replay contains invalid timestamps, prices, sizes, book values or provider metrics".into());
    }
    let mut state=ReplayState::default();
    let mut cash=initial_capital; let mut position=0.0; let mut position_entry_cost=0.0; let mut fees=0.0; let mut slippage=0.0;
    let mut peak=initial_capital; let mut max_dd: f64=0.0; let mut fills=Vec::new();
    let mut net_funding_cashflow=0.0; let mut funding_settlements=0usize; let mut market_impact_paid=0.0;
    let mut buys=0; let mut sells=0; let mut waits=0; let mut last_decision=None;
    let mut daily_equity: Vec<(i64, f64)> = Vec::new();
    let mut last_equity_day: Option<i64> = None;
    let mut max_gross_exposure_pct: f64 = 0.0;
    let mut first_mark_price: Option<f64> = None;
    let mut pending_order: Option<ReplayPendingOrder> = None;
    for event in &events {
        if let Some(previous_ts) = state.last_event_ts_ms {
            let elapsed_ms = event.ts_ms.saturating_sub(previous_ts) as f64;
            let decay = (-std::f64::consts::LN_2 * elapsed_ms / 60_000.0).exp();
            state.signed_flow *= decay;
            state.total_trade_size *= decay;
            state.liquidation_pressure *= decay;
            let oi_decay = (-std::f64::consts::LN_2 * elapsed_ms / 300_000.0).exp();
            state.open_interest_change *= oi_decay;
        }
        let current_mark_price = event_mark_price(event);
        if let Some(price) = current_mark_price {
            state.last_mark_price = Some(price);
            if first_mark_price.is_none() { first_mark_price = Some(price); }
        }

        // A decision made on an earlier event may only fill on this later,
        // executable trade/book observation. Funding and mark events are not fills.
        if let Some(mut pending) = pending_order.take() {
            let eligible = event.ts_ms > pending.decision_ts_ms
                && event.ts_ms >= pending.submitted_ts_ms.saturating_add(latency_ms);
            let mut keep_pending = true;
            if eligible {
                if let (Some(price), Some(available_liquidity)) = (
                    event_execution_price(event, &pending.action),
                    event_available_liquidity(event, &pending.action),
                ) {
                    let mark = current_mark_price.unwrap_or(price);
                    let target_quantity = pending.target_quantity.unwrap_or_else(|| {
                        if pending.action == "buy" {
                            (cash / (price * (1.0 + (slippage_bps + market_impact_bps) / 10_000.0) * (1.0 + fee_bps / 10_000.0))).max(0.0)
                        } else {
                            position
                        }
                    });
                    pending.target_quantity = Some(target_quantity);
                    let remaining_before = (target_quantity - pending.filled_quantity).max(0.0);
                    let liquidity_cap = available_liquidity * max_participation_rate;
                    let quantity = remaining_before.min(liquidity_cap);
                    if quantity > 0.0 {
                        if pending.action == "buy" && cash > 0.0 {
                            let base_execution = price * (1.0 + slippage_bps / 10_000.0);
                            let max_execution = price * (1.0 + (slippage_bps + market_impact_bps) / 10_000.0);
                            let affordable_quantity = (cash / (max_execution * (1.0 + fee_bps / 10_000.0))).max(0.0);
                            let quantity = quantity.min(affordable_quantity);
                            if quantity > 0.0 {
                                let participation_ratio = (quantity / available_liquidity).clamp(0.0, 1.0);
                                let applied_impact_bps = market_impact_bps * participation_ratio.powi(2);
                                let execution = price * (1.0 + (slippage_bps + applied_impact_bps) / 10_000.0);
                                let fee = quantity * execution * fee_bps / 10_000.0;
                                let slip = quantity * (base_execution - price);
                                let impact_cost = quantity * (execution - base_execution);
                                cash -= quantity * execution + fee;
                                position += quantity;
                                position_entry_cost += quantity * execution + fee;
                                fees += fee;
                                slippage += slip;
                                market_impact_paid += impact_cost;
                                let filled_after = pending.filled_quantity + quantity;
                                let remaining_after = (target_quantity - filled_after).max(0.0);
                                let status = if remaining_after <= 1e-12 { ReplayOrderStatus::Filled } else { ReplayOrderStatus::PartiallyFilled };
                                fills.push(ReplayFill {
                                    order_id: pending.order_id.clone(), status,
                                    ts_ms: event.ts_ms, event_id: event.event_id.clone(), action: "BUY".into(),
                                    price: execution, quantity, fee, slippage_cost: slip, realized_pnl: 0.0,
                                    equity_after: cash + position * mark, reason: pending.reason.clone(),
                                    decision_ts_ms: pending.decision_ts_ms,
                                    decision_event_id: pending.decision_event_id.clone(),
                                    submitted_ts_ms: pending.submitted_ts_ms,
                                    submitted_event_id: pending.submitted_event_id.clone(),
                                    target_quantity, filled_quantity: filled_after, remaining_quantity: remaining_after,
                                    market_impact_bps: applied_impact_bps, market_impact_cost: impact_cost,
                                });
                                pending.filled_quantity = filled_after;
                                pending.remaining_quantity = Some(remaining_after);
                                pending.status = status;
                                keep_pending = status != ReplayOrderStatus::Filled;
                            }
                        } else if pending.action == "sell" && position > 0.0 {
                            let quantity = quantity.min(position);
                            let base_execution = price * (1.0 - slippage_bps / 10_000.0);
                            let participation_ratio = (quantity / available_liquidity).clamp(0.0, 1.0);
                            let applied_impact_bps = market_impact_bps * participation_ratio.powi(2);
                            let execution = price * (1.0 - (slippage_bps + applied_impact_bps) / 10_000.0);
                            let fee = quantity * execution * fee_bps / 10_000.0;
                            let slip = quantity * (price - base_execution);
                            let impact_cost = quantity * (base_execution - execution);
                            let proceeds = quantity * execution - fee;
                            let entry_cost_portion = position_entry_cost * (quantity / position);
                            let realized_pnl = proceeds - entry_cost_portion;
                            cash += proceeds;
                            position -= quantity;
                            position_entry_cost = (position_entry_cost - entry_cost_portion).max(0.0);
                            if position <= 1e-12 {
                                position = 0.0;
                                position_entry_cost = 0.0;
                            }
                            fees += fee;
                            slippage += slip;
                            market_impact_paid += impact_cost;
                            let filled_after = pending.filled_quantity + quantity;
                            let remaining_after = (target_quantity - filled_after).max(0.0);
                            let status = if remaining_after <= 1e-12 { ReplayOrderStatus::Filled } else { ReplayOrderStatus::PartiallyFilled };
                            fills.push(ReplayFill {
                                order_id: pending.order_id.clone(), status,
                                ts_ms: event.ts_ms, event_id: event.event_id.clone(), action: "SELL".into(),
                                price: execution, quantity, fee, slippage_cost: slip, realized_pnl,
                                equity_after: cash + position * mark, reason: pending.reason.clone(),
                                decision_ts_ms: pending.decision_ts_ms,
                                decision_event_id: pending.decision_event_id.clone(),
                                submitted_ts_ms: pending.submitted_ts_ms,
                                submitted_event_id: pending.submitted_event_id.clone(),
                                target_quantity, filled_quantity: filled_after, remaining_quantity: remaining_after,
                                market_impact_bps: applied_impact_bps, market_impact_cost: impact_cost,
                            });
                            pending.filled_quantity = filled_after;
                            pending.remaining_quantity = Some(remaining_after);
                            pending.status = status;
                            keep_pending = status != ReplayOrderStatus::Filled;
                        }
                    } else {
                        pending.remaining_quantity = Some(remaining_before);
                    }
                }
            }
            if keep_pending {
                pending_order = Some(pending);
            }
        }
        if event.event_type == "trade" {
            state.last_trade_ts_ms=Some(event.ts_ms);
        if let Some(price)=event.price.filter(|v|*v>0.0) {
            if let Some(previous)=state.last_price.filter(|v|*v>0.0) {
                state.ewma_abs_return=0.94*state.ewma_abs_return+0.06*(price/previous).ln().abs();
            }
            state.last_price=Some(price);
            state.basis_bps=state.last_mark_price.filter(|mark| *mark > 0.0)
                .map(|mark| (mark-price)/price*10_000.0);
        }
        }
        if event.event_type == "mark_price" {
            if let Some(mark)=event.price.filter(|value| *value > 0.0) {
                state.basis_bps=state.last_price.filter(|last| *last > 0.0)
                    .map(|last| (mark-last)/last*10_000.0);
            }
        }
        if let (Some(bid),Some(ask))=(event.bid,event.ask) {
            let mid=(bid+ask)/2.0;
            state.spread_bps=if mid>0.0 {Some((ask-bid)/mid*10_000.0)} else {None};
            state.last_book_ts_ms=Some(event.ts_ms);
            state.liquidity_imbalance=match (event.bid_size,event.ask_size) {
                (Some(bs),Some(as_)) if bs+as_>0.0 => (bs-as_)/(bs+as_), _ => 0.0,
            };
        }
        if event.event_type == "funding" && position > 0.0 {
            if let (Some(rate), Some(mark_price)) = (event.funding_rate, event.price.filter(|price| price.is_finite() && *price > 0.0)) {
                let cashflow = funding_cashflow(position, mark_price, rate);
                cash += cashflow;
                net_funding_cashflow += cashflow;
                funding_settlements += 1;
            }
        }
        match event.event_type.as_str() {
            "trade" => { let size=event.size.unwrap_or(0.0); match event.side.as_deref() {
                Some("buy")=>state.signed_flow+=size, Some("sell")=>state.signed_flow-=size, _=>{} }
                state.total_trade_size+=size; state.event_intensity+=1.0; }
            "liquidation" => {state.liquidation_events+=1;state.event_intensity+=2.0;
                state.last_liquidation_side=event.liquidation_side.clone();
                match event.liquidation_side.as_deref() {Some("long")=>state.liquidation_pressure-=1.0,
                    Some("short")=>state.liquidation_pressure+=1.0,_=>{}}}
            "funding" | "mark_price"=>if event.funding_rate.is_some(){state.funding_rate=event.funding_rate;},
            "open_interest"=>if let Some(current)=event.open_interest {
                if let Some(previous)=state.open_interest.filter(|value| *value>0.0) {
                    state.open_interest_change=((current-previous)/previous).clamp(-3.0,3.0);
                }
                state.open_interest=Some(current);
            },
            _=>{}
        }
        if state.total_trade_size>0.0 && state.last_trade_ts_ms.is_some_and(|last_trade| event.ts_ms.saturating_sub(last_trade)<=60_000) {
            state.ofi=(state.signed_flow/state.total_trade_size).clamp(-1.0,1.0);
        } else { state.ofi=0.0; }
        state.last_event_ts_ms=Some(state.last_event_ts_ms.unwrap_or(0).max(event.ts_ms));
        let decision=decide(&symbol,&state,event)?;
        match decision.action.as_str() { "buy"=>buys+=1,"sell"=>sells+=1,_=>waits+=1 }
        let mark = state.last_mark_price.or(state.last_price);
        if let Some(price)=mark {
            let equity=cash+position*price;
            peak=peak.max(equity);
            if peak>0.0 {max_dd=max_dd.max((peak-equity)/peak);}
            if equity.is_finite() && equity > 0.0 {
                let day = event.ts_ms.div_euclid(86_400_000);
                match last_equity_day {
                    None => {
                        daily_equity.push((day, equity));
                        last_equity_day = Some(day);
                    }
                    Some(previous_day) if day == previous_day => {
                        if let Some(last) = daily_equity.last_mut() { last.1 = equity; }
                    }
                    Some(previous_day) if day > previous_day => {
                        daily_equity.push((day, equity));
                        last_equity_day = Some(day);
                    }
                    _ => {}
                }
                max_gross_exposure_pct = max_gross_exposure_pct.max((position * price / equity).max(0.0) * 100.0);
            }
        }
        if pending_order.is_none() {
            let should_queue = (decision.action == "buy" && position == 0.0)
                || (decision.action == "sell" && position > 0.0);
            if should_queue {
                pending_order = Some(ReplayPendingOrder {
                    order_id: format!("{}:{}:{}", event.ts_ms, event.event_id, decision.action),
                    status: ReplayOrderStatus::Pending,
                    action: decision.action.to_string(),
                    decision_ts_ms: event.ts_ms,
                    decision_event_id: event.event_id.clone(),
                    // The current simulator assumes immediate submission after the signal.
                    submitted_ts_ms: event.ts_ms,
                    submitted_event_id: event.event_id.clone(),
                    target_quantity: if decision.action == "buy" {
                        mark.filter(|price| *price > 0.0).map(|price| (cash / (price * (1.0 + (slippage_bps + market_impact_bps) / 10_000.0) * (1.0 + fee_bps / 10_000.0))).max(0.0))
                    } else {
                        Some(position)
                    },
                    filled_quantity: 0.0,
                    remaining_quantity: if decision.action == "buy" {
                        mark.filter(|price| *price > 0.0).map(|price| (cash / (price * (1.0 + (slippage_bps + market_impact_bps) / 10_000.0) * (1.0 + fee_bps / 10_000.0))).max(0.0))
                    } else {
                        Some(position)
                    },
                    reason: decision.reason.clone(),
                });
            }
        }
        last_decision=Some(decision);
    }
    let final_mark=state.last_mark_price.or(state.last_price).unwrap_or(0.0); let final_equity=cash+position*final_mark;
    if !final_equity.is_finite() || !final_mark.is_finite() {
        return Err("backtest final valuation overflowed; reduce position size or replay input magnitude".into());
    }
    let net_pnl = final_equity - initial_capital;
    if !net_pnl.is_finite() {
        return Err("backtest net P&L overflowed".into());
    }
    let net_return_pct = (final_equity / initial_capital - 1.0) * 100.0;
    let elapsed_ms = events.last().unwrap().ts_ms.saturating_sub(events.first().unwrap().ts_ms);
    let elapsed_years = elapsed_ms as f64 / (365.25 * 86_400_000.0);
    let annualized_return_pct = if elapsed_years > 0.0 && final_equity > 0.0 {
        Some(((final_equity / initial_capital).powf(1.0 / elapsed_years) - 1.0) * 100.0).filter(|value| value.is_finite())
    } else { None };
    let daily_returns: Vec<(f64, f64)> = daily_equity.windows(2)
        .filter_map(|pair| {
            if pair[0].1 <= 0.0 || pair[1].1 <= 0.0 { return None; }
            let days = (pair[1].0 - pair[0].0).max(1) as f64;
            let dailyized = (pair[1].1 / pair[0].1).powf(1.0 / days) - 1.0;
            dailyized.is_finite().then_some((dailyized, days))
        })
        .collect();
    let observed_days = daily_returns.iter().map(|(_, days)| *days).sum::<f64>();
    let mean_daily_return = if observed_days > 0.0 {
        Some(daily_returns.iter().map(|(value, days)| value * days).sum::<f64>() / observed_days)
    } else { None };
    let daily_variance = if daily_returns.len() >= 2 && observed_days > 0.0 {
        let mean = mean_daily_return.unwrap_or(0.0);
        Some(daily_returns.iter().map(|(value, days)| (value - mean).powi(2) * days).sum::<f64>() / observed_days)
    } else { None };
    let daily_volatility = daily_variance.map(f64::sqrt).filter(|value| value.is_finite() && *value > 0.0);
    let volatility_pct = daily_volatility.map(|value| value * 365.25_f64.sqrt() * 100.0).filter(|value| value.is_finite());
    let sharpe_ratio = match (mean_daily_return, daily_volatility) {
        (Some(mean), Some(volatility)) => Some(mean / volatility * 365.25_f64.sqrt()).filter(|value| value.is_finite()),
        _ => None,
    };
    let downside_deviation = (if observed_days > 0.0 {
        Some((daily_returns.iter().map(|(value, days)| value.min(0.0).powi(2) * days).sum::<f64>() / observed_days).sqrt())
    } else { None }).filter(|value| value.is_finite() && *value > 0.0);
    let sortino_ratio = match (mean_daily_return, downside_deviation) {
        (Some(mean), Some(downside)) => Some(mean / downside * 365.25_f64.sqrt()).filter(|value| value.is_finite()),
        _ => None,
    };
    let calmar_ratio = match (annualized_return_pct, max_dd > 0.0) {
        (Some(annualized), true) => Some((annualized / 100.0) / max_dd),
        _ => None,
    };
    let mut closed_trade_pnl: std::collections::BTreeMap<String, (f64, bool)> = std::collections::BTreeMap::new();
    for fill in fills.iter().filter(|fill| fill.action == "SELL") {
        let entry = closed_trade_pnl.entry(fill.order_id.clone()).or_insert((0.0, false));
        entry.0 += fill.realized_pnl;
        if fill.status == ReplayOrderStatus::Filled { entry.1 = true; }
    }
    let completed: Vec<f64> = closed_trade_pnl.values().filter(|(pnl, complete)| *complete && pnl.is_finite()).map(|(pnl, _)| *pnl).collect();
    let winning_trade_pnl = completed.iter().filter(|pnl| **pnl > 0.0).sum::<f64>();
    let losing_trade_pnl = -completed.iter().filter(|pnl| **pnl < 0.0).sum::<f64>();
    let completed_trades = completed.len();
    let win_rate_pct = (completed_trades > 0).then_some(
        completed.iter().filter(|pnl| **pnl > 0.0).count() as f64 / completed_trades as f64 * 100.0
    );
    let profit_factor = (losing_trade_pnl > 0.0).then_some(winning_trade_pnl / losing_trade_pnl).filter(|value| value.is_finite());
    let buy_and_hold_return_pct = first_mark_price.filter(|price| *price > 0.0)
        .map(|first| (final_mark / first - 1.0) * 100.0)
        .filter(|value| value.is_finite());
    let transaction_costs = fees + slippage + market_impact_paid;
    if !winning_trade_pnl.is_finite() || !losing_trade_pnl.is_finite() || !transaction_costs.is_finite() || !max_gross_exposure_pct.is_finite() {
        return Err("backtest performance metric overflowed".into());
    }
    let realized_execution_costs = transaction_costs;
    let execution_stress_scenarios = [
        ("baseline", 1.0),
        ("moderate_cost_stress", 1.5),
        ("severe_cost_stress", 2.0),
    ].into_iter().map(|(name, cost_multiplier)| {
        let estimated_net_pnl = net_pnl - realized_execution_costs * (cost_multiplier - 1.0);
        ExecutionStressScenario {
            name: name.into(),
            cost_multiplier,
            fees_paid: fees * cost_multiplier,
            slippage_paid: slippage * cost_multiplier,
            market_impact_paid: market_impact_paid * cost_multiplier,
            estimated_net_pnl,
            estimated_return_pct: estimated_net_pnl / initial_capital * 100.0,
            fills_recomputed: false,
        }
    }).collect();
    Ok(ReplayResult{symbol,venue,strategy_key,strategy_version,
        from_ts_ms:events.first().unwrap().ts_ms,to_ts_ms:events.last().unwrap().ts_ms,events_replayed:events.len(),
        max_event_gap_ms:events.windows(2).map(|pair|pair[1].ts_ms-pair[0].ts_ms).max().unwrap_or(0),
        event_gaps_over_5m:events.windows(2).filter(|pair|pair[1].ts_ms-pair[0].ts_ms>300_000).count(),
        decisions:events.len(),buy_decisions:buys,sell_decisions:sells,wait_decisions:waits,initial_capital,
        final_equity,net_pnl,net_return_pct,
        max_drawdown_pct:max_dd*100.0,
        annualized_return_pct,volatility_pct,sharpe_ratio,sortino_ratio,calmar_ratio,
        win_rate_pct,profit_factor,winning_trade_pnl,losing_trade_pnl,completed_trades,
        max_gross_exposure_pct,transaction_costs,buy_and_hold_return_pct,
        fees_paid:fees,slippage_paid:slippage,net_funding_cashflow,funding_settlements,fee_bps,slippage_bps,latency_ms,max_participation_rate,market_impact_bps,market_impact_paid,fills,last_decision,
        unfilled_pending_order: pending_order,
        execution_stress_scenarios,
        assumptions:vec!["Historical replay uses only persisted raw events; it does not download missing history.".into(),
        "Events are sorted by event timestamp then event ID; each decision sees only the current and prior events.".into(),
        "A decision is submitted immediately at its signal event in this model, then can only fill on a strictly later executable trade or book event; the observation that generated the decision is never reused as its fill.".into(),
        "Submission-to-fill latency is a minimum event-timestamp delay in milliseconds; routing jitter, acknowledgements and queue priority are not modeled. A final pending order without a later eligible executable observation remains unfilled and is returned as unfilled_pending_order.".into(),
        "Fills are capped at the configured fraction of observed trade size or side-specific top-of-book size; partial orders persist across later events. Hidden liquidity and queue position are not modeled.".into(),
        "Market impact is modeled as the configured coefficient multiplied by the squared per-fill participation ratio; impact is bounded by the configured coefficient and reported separately from base slippage. Cross-level depth, hidden liquidity and queue priority are not modeled.".into(),
        "Buy order sizing reserves the configured maximum slippage and impact before fees so a fill cannot spend more than available cash under the configured cost bound.".into(),
        "Execution stress scenarios reprice realized fees, base slippage and market-impact costs at 1.0x, 1.5x and 2.0x; they are cost-only estimates and do not recompute fills, quantities or liquidity.".into(),
        "Long-only, all-available-cash entries and full exits on SELL; no leverage or shorting.".into(),
        "Fees and slippage are constant user-supplied basis-point assumptions; slippage must be below 10,000 bps. Positive perpetual funding rates are paid by long positions and negative rates are received only when that funding event carries its own valid mark price.".into(),
        "Trade fills use the observed trade price; book fills use ask for buys and bid for sells. Equity is marked to the latest trade or book midpoint. No forced liquidation or borrow cost model.".into(),
        "Results are a research simulation, not live or expected performance.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge_strategy::EdgeStrategy;
    #[test]
    fn replay_is_time_ordered_and_charges_execution_costs() {
        let events=vec![
            ReplayEvent{event_id:"b".into(),venue:"test".into(),symbol:"BTCUSDT".into(),event_type:"trade".into(),ts_ms:2,
                price:Some(101.0),size:Some(1.0),side:Some("buy".into()),bid:None,ask:None,bid_size:None,ask_size:None,
                funding_rate:None,open_interest:None,liquidation_side:None},
            ReplayEvent{event_id:"a".into(),venue:"test".into(),symbol:"BTCUSDT".into(),event_type:"book".into(),ts_ms:1,
                price:None,size:None,side:None,bid:Some(99.0),ask:Some(100.0),bid_size:Some(1.0),ask_size:Some(1.0),
                funding_rate:None,open_interest:None,liquidation_side:None},
        ];
        let result=replay(events,&EdgeStrategy::baseline(),1000.0,10.0,5.0).unwrap();
        assert_eq!(result.from_ts_ms,1); assert_eq!(result.to_ts_ms,2);
        assert_eq!(result.events_replayed,2); assert_eq!(result.max_event_gap_ms,1); assert_eq!(result.event_gaps_over_5m,0); assert!(result.fees_paid>=0.0);
        assert_eq!(result.strategy_version,EdgeStrategy::baseline().version);
    }
    #[test]
    fn execution_requires_a_current_price_or_book_midpoint() {
        let mut event = ReplayEvent {
            event_id:"book".into(), venue:"binance_usdm".into(), symbol:"BTCUSDT".into(),
            event_type:"book".into(), ts_ms:1, price:None, size:None, side:None,
            bid:Some(100.0), ask:Some(102.0), bid_size:None, ask_size:None,
            funding_rate:None, open_interest:None, liquidation_side:None,
        };
        assert_eq!(event_execution_price(&event, "buy"), Some(102.0));
        assert_eq!(event_execution_price(&event, "sell"), Some(100.0));
        event.bid_size = Some(3.0);
        event.ask_size = Some(4.0);
        assert_eq!(event_available_liquidity(&event, "buy"), Some(4.0));
        assert_eq!(event_available_liquidity(&event, "sell"), Some(3.0));
        event.bid_size = None;
        event.ask_size = None;
        event.bid = None;
        event.ask = None;
        assert_eq!(event_execution_price(&event, "buy"), None);
        event.event_type = "funding".into();
        event.price = Some(101.0);
        assert_eq!(event_execution_price(&event, "buy"), None);
    }
    #[test]
    fn replay_rejects_mixed_venues_and_unsafe_slippage() {
        let event = |id: &str, venue: &str| ReplayEvent {
            event_id:id.into(), venue:venue.into(), symbol:"BTCUSDT".into(), event_type:"trade".into(),
            ts_ms:1, price:Some(100.0), size:Some(1.0), side:Some("buy".into()),
            bid:None, ask:None, bid_size:None, ask_size:None, funding_rate:None,
            open_interest:None, liquidation_side:None,
        };
        assert!(replay(vec![event("a","binance_usdm"),event("b","finnhub")],
            &EdgeStrategy::baseline(),1000.0,10.0,5.0).is_err());
        assert!(replay(vec![event("a","binance_usdm")],
            &EdgeStrategy::baseline(),1000.0,10.0,10_000.0).is_err());
    }
    #[test]
    fn replay_settles_funding_only_after_a_long_position_is_open() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 5.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let events = vec![
            ReplayEvent {
                event_id: "book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 1, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(5.0), ask_size: Some(4.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "trade".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "trade".into(), ts_ms: 2, price: Some(100.0), size: Some(1.0), side: Some("buy".into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "next-trade".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "trade".into(), ts_ms: 3, price: Some(100.0), size: Some(1.0), side: Some("buy".into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "funding".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "funding".into(), ts_ms: 4, price: Some(100.0), size: None, side: None,
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: Some(0.001), open_interest: None, liquidation_side: None,
            },
        ];
        let result = replay(events, &strategy, 1_000.0, 10.0, 5.0).unwrap();
        assert_eq!(result.funding_settlements, 1);
        assert!(result.net_funding_cashflow < 0.0);
        let entry = result.fills.iter().find(|fill| fill.action == "BUY").unwrap();
        assert_eq!(entry.ts_ms, 3);
        assert_eq!(entry.decision_ts_ms, 2);
        assert_eq!(entry.decision_event_id, "trade");
        assert_eq!(entry.submitted_ts_ms, 2);
        assert_eq!(entry.submitted_event_id, "trade");
        assert!(matches!(entry.status, ReplayOrderStatus::PartiallyFilled));
        let pending = result.unfilled_pending_order.unwrap();
        assert!(matches!(pending.status, ReplayOrderStatus::PartiallyFilled));
        assert!(pending.remaining_quantity.unwrap() > 0.0);
    }

    #[test]
    fn replay_respects_submission_latency_before_filling() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 5.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let event = |event_id: &str, event_type: &str, ts_ms: i64| ReplayEvent {
            event_id: event_id.into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms,
            price: if event_type == "trade" { Some(100.0) } else { None },
            size: if event_type == "trade" { Some(1.0) } else { None },
            side: if event_type == "trade" { Some("buy".into()) } else { None },
            bid: if event_type == "book" { Some(99.99) } else { None },
            ask: if event_type == "book" { Some(100.01) } else { None },
            bid_size: if event_type == "book" { Some(5.0) } else { None },
            ask_size: if event_type == "book" { Some(4.0) } else { None },
            funding_rate: None, open_interest: None, liquidation_side: None,
        };
        let events = vec![
            event("book", "book", 1),
            event("signal", "trade", 2),
            event("too-early", "trade", 20),
            event("eligible", "trade", 52),
        ];
        let result = replay_with_latency(events, &strategy, 1_000.0, 10.0, 5.0, 50).unwrap();
        assert_eq!(result.latency_ms, 50);
        let fill = result.fills.first().unwrap();
        assert_eq!(fill.event_id, "eligible");
        assert_eq!(fill.ts_ms, 52);
        assert_eq!(fill.submitted_ts_ms, 2);
    }

    #[test]
    fn replay_applies_quadratic_market_impact_from_fill_participation() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 5.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let event = |event_id: &str, event_type: &str, ts_ms: i64, size: f64| ReplayEvent {
            event_id: event_id.into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms,
            price: if event_type == "trade" { Some(100.0) } else { None },
            size: if event_type == "trade" { Some(size) } else { None },
            side: if event_type == "trade" { Some("buy".into()) } else { None },
            bid: if event_type == "book" { Some(99.99) } else { None },
            ask: if event_type == "book" { Some(100.01) } else { None },
            bid_size: if event_type == "book" { Some(10.0) } else { None },
            ask_size: if event_type == "book" { Some(10.0) } else { None },
            funding_rate: None, open_interest: None, liquidation_side: None,
        };
        let events = vec![
            event("book", "book", 1, 0.0),
            event("signal", "trade", 2, 1.0),
            event("fill", "trade", 3, 2.0),
        ];
        let result = replay_with_execution(events, &strategy, 1_000.0, 0.0, 0.0, 0, 0.5, 10.0).unwrap();
        let fill = result.fills.first().unwrap();
        assert!((fill.market_impact_bps - 2.5).abs() < 1e-12);
        assert!((fill.market_impact_cost - 0.025).abs() < 1e-9);
        assert_eq!(result.slippage_paid, 0.0);
        assert!((result.market_impact_paid - fill.market_impact_cost).abs() < 1e-12);
        assert_eq!(result.market_impact_bps, 10.0);
        assert_eq!(result.execution_stress_scenarios.len(), 3);
        assert_eq!(result.execution_stress_scenarios[0].cost_multiplier, 1.0);
        assert_eq!(result.execution_stress_scenarios[1].cost_multiplier, 1.5);
        assert_eq!(result.execution_stress_scenarios[2].cost_multiplier, 2.0);
        assert!((result.execution_stress_scenarios[0].estimated_net_pnl - result.net_pnl).abs() < 1e-12);
        assert!(result.execution_stress_scenarios[1].estimated_net_pnl < result.net_pnl);
        assert!(result.execution_stress_scenarios[2].estimated_net_pnl < result.execution_stress_scenarios[1].estimated_net_pnl);
        assert!(result.execution_stress_scenarios.iter().all(|scenario| !scenario.fills_recomputed));
    }

    #[test]
    fn replay_partially_fills_orders_within_observed_liquidity() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 5.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let event = |event_id: &str, event_type: &str, ts_ms: i64, size: f64| ReplayEvent {
            event_id: event_id.into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms,
            price: if event_type == "trade" { Some(100.0 + ts_ms as f64 / 100.0) } else { None },
            size: if event_type == "trade" { Some(size) } else { None },
            side: if event_type == "trade" { Some("buy".into()) } else { None },
            bid: if event_type == "book" { Some(99.99) } else { None },
            ask: if event_type == "book" { Some(100.01) } else { None },
            bid_size: if event_type == "book" { Some(10.0) } else { None },
            ask_size: if event_type == "book" { Some(10.0) } else { None },
            funding_rate: None, open_interest: None, liquidation_side: None,
        };
        let events = vec![
            event("book", "book", 1, 0.0),
            event("signal", "trade", 2, 1.0),
            event("partial-1", "trade", 3, 2.0),
            event("partial-2", "trade", 4, 4.0),
        ];
        let result = replay_with_execution(events, &strategy, 1_000.0, 10.0, 0.0, 0, 0.5, 5.0).unwrap();
        assert_eq!(result.max_participation_rate, 0.5);
        assert_eq!(result.fills.len(), 2);
        assert!((result.fills[0].quantity - 1.0).abs() < 1e-12);
        assert!((result.fills[1].quantity - 2.0).abs() < 1e-12);
        assert!(matches!(result.fills[0].status, ReplayOrderStatus::PartiallyFilled));
        assert!(matches!(result.fills[1].status, ReplayOrderStatus::PartiallyFilled));
        assert_eq!(result.fills[0].order_id, result.fills[1].order_id);
        assert!((result.fills[1].filled_quantity - 3.0).abs() < 1e-12);
        assert!(result.fills[1].remaining_quantity > 6.0);
        let pending = result.unfilled_pending_order.unwrap();
        assert!(matches!(pending.status, ReplayOrderStatus::PartiallyFilled));
        assert!((pending.filled_quantity - 3.0).abs() < 1e-12);
        assert!(pending.remaining_quantity.unwrap() > 6.0);
    }

    #[test]
    fn replay_partially_fills_sell_orders_and_realizes_cost_basis_proportionally() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.bullish_weights.insert("order_flow_imbalance".into(), 5.0);
        strategy.bearish_weights.insert("funding_pressure".into(), 5.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let trade = |event_id: &str, ts_ms: i64, price: f64, size: f64, side: &str| ReplayEvent {
            event_id: event_id.into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms, price: Some(price), size: Some(size), side: Some(side.into()),
            bid: None, ask: None, bid_size: None, ask_size: None,
            funding_rate: None, open_interest: None, liquidation_side: None,
        };
        let events = vec![
            ReplayEvent {
                event_id: "book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 1, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(10.0), ask_size: Some(10.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            trade("buy-signal", 2, 100.0, 1.0, "buy"),
            trade("buy-fill", 3, 100.0, 20.0, "buy"),
            ReplayEvent {
                event_id: "funding".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "funding".into(), ts_ms: 4, price: Some(100.0), size: None, side: None,
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: Some(0.01), open_interest: None, liquidation_side: None,
            },
            trade("sell-signal", 5, 102.0, 4.0, "sell"),
            trade("sell-partial-1", 6, 105.0, 4.0, "sell"),
            trade("sell-partial-2", 7, 110.0, 4.0, "sell"),
        ];
        let result = replay_with_execution(events, &strategy, 1_000.0, 10.0, 0.0, 0, 0.5, 5.0).unwrap();
        let sell_fills: Vec<_> = result.fills.iter().filter(|fill| fill.action == "SELL").collect();
        assert_eq!(sell_fills.len(), 2);
        assert!(matches!(result.fills[0].status, ReplayOrderStatus::Filled));
        assert!(matches!(sell_fills[0].status, ReplayOrderStatus::PartiallyFilled));
        assert!(matches!(sell_fills[1].status, ReplayOrderStatus::PartiallyFilled));
        assert_eq!(sell_fills[0].order_id, sell_fills[1].order_id);
        assert!((sell_fills[0].quantity - 2.0).abs() < 1e-12);
        assert!((sell_fills[1].quantity - 2.0).abs() < 1e-12);
        assert!(sell_fills.iter().all(|fill| fill.realized_pnl > 0.0));
        assert!((sell_fills[1].filled_quantity - 4.0).abs() < 1e-12);
        let pending = result.unfilled_pending_order.unwrap();
        assert!(matches!(pending.status, ReplayOrderStatus::PartiallyFilled));
        assert_eq!(pending.action, "sell");
        assert!((pending.filled_quantity - 4.0).abs() < 1e-12);
        assert!(pending.remaining_quantity.unwrap() > 5.0);
    }

    #[test]
    fn final_signal_ignores_stale_and_same_timestamp_book_events() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.values_mut().for_each(|weight| *weight = 5.0);
        strategy.bearish_weights.values_mut().for_each(|weight| *weight = 0.0);
        strategy.angular_threshold_degrees = 90.0;
        strategy.magnitude_threshold = 0.001;
        let events = vec![
            ReplayEvent {
                event_id: "book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 1, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(5.0), ask_size: Some(4.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "signal-trade".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "trade".into(), ts_ms: 2, price: Some(100.0), size: Some(1.0), side: Some("buy".into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "z-stale-book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 1, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(5.0), ask_size: Some(4.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "z-same-time-book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 2, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(5.0), ask_size: Some(4.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
        ];
        let result = replay(events, &strategy, 1_000.0, 10.0, 5.0).unwrap();
        assert!(result.fills.is_empty(), "same-timestamp events cannot fill a decision");
        let pending = result.unfilled_pending_order.unwrap();
        assert_eq!(pending.action, "buy");
        assert_eq!(pending.decision_ts_ms, 2);
        assert_eq!(pending.decision_event_id, "signal-trade");
        assert_eq!(pending.submitted_ts_ms, 2);
        assert_eq!(pending.submitted_event_id, "signal-trade");
        assert!(matches!(pending.status, ReplayOrderStatus::Pending));
    }

    #[test]
    fn funding_cashflow_charges_longs_on_positive_rates_and_credits_negative_rates() {
        assert!((funding_cashflow(2.0, 100.0, 0.001) + 0.2).abs() < 1e-12);
        assert!((funding_cashflow(2.0, 100.0, -0.001) - 0.2).abs() < 1e-12);
        assert_eq!(funding_cashflow(0.0, 100.0, 0.001), 0.0);
    }
    #[test]
    fn replay_rejects_empty_or_invalid_cost_assumptions() {
        assert!(replay(vec![],&EdgeStrategy::baseline(),1000.0,0.0,0.0).is_err());
    }

    #[test]
    fn replay_rejects_latency_outside_the_bounded_range() {
        let event = ReplayEvent {
            event_id: "trade".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms: 1, price: Some(100.0), size: Some(1.0),
            side: Some("buy".into()), bid: None, ask: None, bid_size: None, ask_size: None,
            funding_rate: None, open_interest: None, liquidation_side: None,
        };
        assert!(replay_with_latency(vec![event.clone()], &EdgeStrategy::baseline(), 1_000.0, 10.0, 5.0, -1).is_err());
        assert!(replay_with_latency(vec![event.clone()], &EdgeStrategy::baseline(), 1_000.0, 10.0, 5.0, 60_001).is_err());
        assert!(replay_with_execution(vec![event.clone()], &EdgeStrategy::baseline(), 1_000.0, 10.0, 5.0, 0, 0.0, 5.0).is_err());
        assert!(replay_with_execution(vec![event.clone()], &EdgeStrategy::baseline(), 1_000.0, 10.0, 5.0, 0, 1.01, 5.0).is_err());
        assert!(replay_with_execution(vec![event.clone()], &EdgeStrategy::baseline(), 1_000.0, 10.0, 5.0, 0, 0.5, 1_001.0).is_err());
        assert!(replay_with_execution(vec![event], &EdgeStrategy::baseline(), 1_000.0, 10.0, 9_999.0, 0, 0.5, 5.0).is_err());
    }
}


#[cfg(test)]
mod vector_engine_replay_tests {
    use super::*;
    use qoin_vector_engine::EngineConfig;

    #[test]
    fn replay_reports_annualized_risk_metrics_and_buy_and_hold_benchmark() {
        let day = 86_400_000_i64;
        let start = 1_700_000_000_000_i64;
        let events = vec![
            ReplayEvent {
                event_id: "day-0".into(), venue: "venue-a".into(), symbol: "ASSET-USD".into(),
                event_type: "book".into(), ts_ms: start, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(10.0), ask_size: Some(10.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "day-1".into(), venue: "venue-a".into(), symbol: "ASSET-USD".into(),
                event_type: "book".into(), ts_ms: start + day, price: None, size: None, side: None,
                bid: Some(100.99), ask: Some(101.01), bid_size: Some(10.0), ask_size: Some(10.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "day-2".into(), venue: "venue-a".into(), symbol: "ASSET-USD".into(),
                event_type: "book".into(), ts_ms: start + 2 * day, price: None, size: None, side: None,
                bid: Some(101.99), ask: Some(102.01), bid_size: Some(10.0), ask_size: Some(10.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
        ];
        let result = replay_with_vector_engine(
            events, &EngineConfig::default(), "metrics-test-v1",
            1_000.0, 10.0, 5.0, 0, 0.1, 5.0,
        ).unwrap();
        assert!(result.annualized_return_pct.is_some());
        assert!(result.annualized_return_pct.unwrap().abs() < 1e-9);
        assert!(result.volatility_pct.is_none());
        assert!(result.sharpe_ratio.is_none());
        assert!(result.sortino_ratio.is_none());
        assert!(result.calmar_ratio.is_none());
        assert_eq!(result.completed_trades, 0);
        assert!(result.win_rate_pct.is_none());
        assert!(result.profit_factor.is_none());
        assert_eq!(result.max_gross_exposure_pct, 0.0);
        assert_eq!(result.transaction_costs, 0.0);
        assert!((result.buy_and_hold_return_pct.unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn historical_replay_uses_the_same_vector_engine_and_rejects_stale_quotes() {
        let events = vec![
            ReplayEvent {
                event_id: "book".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "book".into(), ts_ms: 1, price: None, size: None, side: None,
                bid: Some(99.99), ask: Some(100.01), bid_size: Some(5.0), ask_size: Some(5.0),
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "trade-fresh".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "trade".into(), ts_ms: 2, price: Some(100.0), size: Some(1.0), side: Some("buy".into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
            ReplayEvent {
                event_id: "trade-stale".into(), venue: "binance_usdm".into(), symbol: "BTCUSDT".into(),
                event_type: "trade".into(), ts_ms: 30_002, price: Some(100.1), size: Some(1.0), side: Some("buy".into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            },
        ];
        let result = replay_with_vector_engine(
            events, &EngineConfig::default(), "seven-force-test-v1",
            1_000.0, 0.0, 0.0, 0, 0.5, 0.0,
        ).unwrap();

        assert_eq!(result.strategy_key, "qoin-vector-baseline");
        assert_eq!(result.strategy_version, "seven-force-test-v1");
        let last = result.last_decision.unwrap();
        assert_eq!(last.action, "wait");
        assert!(last.reason.contains("stale"));
    }
}
