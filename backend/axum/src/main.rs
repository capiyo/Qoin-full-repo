use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, HashMap}, env, sync::{Arc, RwLock}};
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;

mod risk;
#[cfg(feature = "live-execution")]
mod trading;
use qoin_vector_engine::{DecisionContext, EdgeDecision, EdgeStrategy, STRATEGY_KEY};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct MarketEvent {
    event_id: String,
    venue: String,
    symbol: String,
    event_type: String,
    ts_ms: i64,
    #[serde(default)] received_at_ms: Option<i64>,
    #[serde(default)] sequence: Option<u64>,
    #[serde(default)] price: Option<f64>,
    #[serde(default)] size: Option<f64>,
    #[serde(default)] side: Option<String>,
    #[serde(default)] bid: Option<f64>,
    #[serde(default)] ask: Option<f64>,
    #[serde(default)] bid_size: Option<f64>,
    #[serde(default)] ask_size: Option<f64>,
    #[serde(default)] funding_rate: Option<f64>,
    #[serde(default)] open_interest: Option<f64>,
    #[serde(default)] basis: Option<f64>,
    #[serde(default)] liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct SymbolState {
    symbol: String,
    venue: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    last_state_update_ts_ms: Option<i64>,
    last_event_id: Option<String>,
    candidate_action: Option<String>,
    candidate_persistence_ticks: usize,
    last_sequences: BTreeMap<String, u64>,
    unsequenced_event_types: BTreeMap<String, bool>,
    sequence_valid: bool,
    last_price: Option<f64>,
    last_price_ts_ms: Option<i64>,
    spread_bps: Option<f64>,
    spread_ts_ms: Option<i64>,
    funding_ts_ms: Option<i64>,
    open_interest_ts_ms: Option<i64>,
    basis_ts_ms: Option<i64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    book_imbalance: f64,
    event_intensity: f64,
    ewma_abs_return: f64,
    funding_rate: Option<f64>,
    open_interest: Option<f64>,
    open_interest_change: f64,
    basis: Option<f64>,
    liquidation_events: f64,
    liquidation_pressure: f64,
    liquidation_size: f64,
    state_quality: f64,
}
#[derive(Clone)]
struct AppState {
    events: Collection<Document>,
    states: Collection<Document>,
    decisions: Collection<Document>,
    strategies: Collection<Document>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
    ingest_lock: Arc<Mutex<()>>,
    decay: DecayConfig,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
#[derive(Debug, Clone, Copy)]
struct DecayConfig {
    flow_half_life_ms: f64,
    event_intensity_half_life_ms: f64,
    liquidation_half_life_ms: f64,
    volatility_half_life_ms: f64,
}
impl Default for DecayConfig {
    fn default() -> Self {
        Self { flow_half_life_ms: 5_000.0, event_intensity_half_life_ms: 2_000.0,
            liquidation_half_life_ms: 30_000.0, volatility_half_life_ms: 60_000.0 }
    }
}
impl DecayConfig {
    fn from_env() -> Result<Self, String> {
        let mut config = Self::default();
        for (name, target) in [
            ("QOIN_FLOW_HALF_LIFE_MS", &mut config.flow_half_life_ms),
            ("QOIN_EVENT_INTENSITY_HALF_LIFE_MS", &mut config.event_intensity_half_life_ms),
            ("QOIN_LIQUIDATION_HALF_LIFE_MS", &mut config.liquidation_half_life_ms),
            ("QOIN_VOLATILITY_HALF_LIFE_MS", &mut config.volatility_half_life_ms),
        ] {
            if let Ok(raw) = env::var(name) {
                let value = raw.parse::<f64>().map_err(|_| format!("{name} must be numeric"))?;
                if !value.is_finite() || value <= 0.0 { return Err(format!("{name} must be finite and positive")); }
                *target = value;
            }
        }
        Ok(config)
    }
}
fn decay_factor(elapsed_ms: f64, half_life_ms: f64) -> f64 {
    0.5_f64.powf(elapsed_ms.max(0.0) / half_life_ms)
}
fn token_authorized(headers: &HeaderMap, env_name: &str) -> bool {
    let expected = match env::var(env_name) {
        Ok(value) if !value.trim().is_empty() => format!("Bearer {}", value),
        _ => return false,
    };
    headers.get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value == expected)
        .unwrap_or(false)
}
fn admin_authorized(headers: &HeaderMap) -> bool { token_authorized(headers, "QOIN_ADMIN_TOKEN") }
fn ingest_authorized(headers: &HeaderMap) -> bool { token_authorized(headers, "QOIN_INGEST_TOKEN") }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    if e.received_at_ms.map(|arrival| arrival < e.ts_ms || arrival <= 0).unwrap_or(false) { return Err("received_at_ms must be at or after event time".into()); }
    if env::var("QOIN_REPLAY_MODE").map(|v| v == "true").unwrap_or(false) && e.received_at_ms.is_none() { return Err("replay events require received_at_ms for causal latency modeling".into()); }
    if !["trade", "book", "liquidation", "funding", "open_interest"].contains(&e.event_type.as_str()) {
        return Err("unsupported event_type".into());
    }
    if e.event_type == "trade" && (e.size.unwrap_or(0.0) <= 0.0 || !matches!(e.side.as_deref(), Some("buy") | Some("sell"))) {
        return Err("trade events require positive size and side buy/sell".into());
    }
    if e.event_type == "book" && (e.bid.is_none() || e.ask.is_none() || e.bid_size.is_none() || e.ask_size.is_none()) {
        return Err("book events require bid, ask, bid_size, and ask_size".into());
    }
    if e.event_type == "liquidation" && (e.size.unwrap_or(0.0) <= 0.0 || !matches!(e.liquidation_side.as_deref(), Some("long") | Some("short"))) {
        return Err("liquidation events require positive size and liquidation_side long/short".into());
    }
    for (name, value) in [("price",e.price),("size",e.size),("bid",e.bid),("ask",e.ask),
        ("bid_size",e.bid_size),("ask_size",e.ask_size),("funding_rate",e.funding_rate),("open_interest",e.open_interest),("basis",e.basis)] {
        if !valid_number(value) { return Err(format!("{name} must be finite")); }
    }
    if let (Some(bid),Some(ask))=(e.bid,e.ask) {
        if bid < 0.0 || ask < bid { return Err("book prices must be non-negative and ask >= bid".into()); }
    }
    if [e.price,e.size,e.bid,e.ask,e.bid_size,e.ask_size,e.open_interest].into_iter().flatten().any(|v| v < 0.0) {
        return Err("prices, sizes, and open interest must be non-negative".into());
    }
    Ok(())
}
fn apply(state: &mut SymbolState, event: &MarketEvent, decay: &DecayConfig) -> Result<(), String> {
    if state.event_count > 0 && state.venue.is_empty() {
        return Err("legacy state has no venue provenance; recovery or migration is required before ingest".into());
    }
    if state.event_count > 0 && state.venue != event.venue {
        return Err("venue mismatch: symbol state cannot mix events from different venues".into());
    }
    if let Some(previous_ts) = state.last_event_ts_ms {
        if event.ts_ms < previous_ts { return Err("out-of-order event rejected".into()); }
    }
    if let Some(sequence) = event.sequence {
        let stream_key = format!("{}:{}", event.venue, event.event_type);
        if let Some(previous) = state.last_sequences.get(&stream_key) {
            if sequence <= *previous { return Err("out-of-order source sequence rejected".into()); }
            if sequence != previous.saturating_add(1) { return Err("source sequence gap rejected".into()); }
        }
        state.last_sequences.insert(stream_key.clone(), sequence);
        // Once an unsequenced event has been observed, a later sequence number
        // cannot reconstruct the missing interval. Keep the stream untrusted until
        // an explicit operator-controlled resync/checkpoint is implemented.
        state.sequence_valid = state.unsequenced_event_types.is_empty();
    } else {
        // A feed without sequence metadata cannot prove continuity; signals must abstain.
        state.unsequenced_event_types.insert(format!("{}:{}", event.venue, event.event_type), true);
        state.sequence_valid = false;
    }
    let previous_update = state.last_state_update_ts_ms.or(state.last_event_ts_ms);
    let elapsed_ms = previous_update.map(|previous_ts| event.ts_ms.saturating_sub(previous_ts).max(0) as f64).unwrap_or(0.0);
    let flow_decay = decay_factor(elapsed_ms, decay.flow_half_life_ms);
    let event_decay = decay_factor(elapsed_ms, decay.event_intensity_half_life_ms);
    let liquidation_decay = decay_factor(elapsed_ms, decay.liquidation_half_life_ms);
    let volatility_decay = decay_factor(elapsed_ms, decay.volatility_half_life_ms);
    state.signed_flow *= flow_decay;
    state.total_trade_size *= flow_decay;
    state.event_intensity *= event_decay;
    state.liquidation_events *= liquidation_decay;
    state.liquidation_pressure *= liquidation_decay;
    state.liquidation_size *= liquidation_decay;
    state.ewma_abs_return *= volatility_decay;
    state.symbol = event.symbol.clone();
    state.venue = event.venue.clone();
    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            let abs_return = (price.ln() - previous.ln()).abs();
            let price_elapsed_ms = state.last_price_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts).max(0) as f64).unwrap_or(elapsed_ms);
            let alpha = 1.0 - decay_factor(price_elapsed_ms, decay.volatility_half_life_ms);
            if abs_return.is_finite() { state.ewma_abs_return += alpha * abs_return; }
        }
        state.last_price = Some(price);
        state.last_price_ts_ms = Some(event.ts_ms);
    }
    if let Some(basis) = event.basis { state.basis = Some(basis); state.basis_ts_ms = Some(event.ts_ms); }
    if let (Some(bid), Some(ask)) = (event.bid, event.ask) {
        let mid = (bid + ask) / 2.0;
        state.spread_bps = if mid > 0.0 { Some((ask - bid) / mid * 10_000.0) } else { None };
        state.spread_ts_ms = Some(event.ts_ms);
        if let (Some(bs), Some(as_)) = (event.bid_size, event.ask_size) {
            let depth = bs + as_;
            if depth > 0.0 {
                // Book imbalance is a separate normalized observation, not trade volume.
                state.book_imbalance = ((bs - as_) / depth).clamp(-1.0, 1.0);
            }
        }
    }
    match event.event_type.as_str() {
        "trade" => {
            let size = event.size.unwrap_or(0.0);
            match event.side.as_deref() {
                Some("buy") => state.signed_flow += size,
                Some("sell") => state.signed_flow -= size,
                _ => {}
            }
            state.total_trade_size += size;
            state.event_intensity += 1.0;
        }
        "liquidation" => {
            let size = event.size.unwrap_or(0.0);
            state.liquidation_events += 1.0;
            state.event_intensity += 2.0;
            state.liquidation_size += size;
            // Liquidations are their own normalized force, not ordinary trade OFI.
            match event.liquidation_side.as_deref() {
                Some("long") => state.liquidation_pressure -= size,
                Some("short") => state.liquidation_pressure += size,
                _ => {}
            }
        }
        "funding" => if event.funding_rate.is_some() { state.funding_rate = event.funding_rate; state.funding_ts_ms = Some(event.ts_ms); },
        "open_interest" => if let Some(value) = event.open_interest { if let Some(previous) = state.open_interest.filter(|v| *v > 0.0) { state.open_interest_change = (value - previous) / previous; } state.open_interest = Some(value); state.open_interest_ts_ms = Some(event.ts_ms); },

        "book" => {}
        _ => return Err("unsupported event_type".into()),
    }
    state.event_count = state.event_count.saturating_add(1);
    state.last_event_ts_ms = Some(event.ts_ms);
    state.last_state_update_ts_ms = Some(event.ts_ms);
    state.last_event_id = Some(event.event_id.clone());
    state.order_flow_imbalance = if state.total_trade_size > f64::EPSILON {
        (state.signed_flow / state.total_trade_size).clamp(-1.0, 1.0)
    } else { 0.0 };
    // Quality reflects the bounded availability of recent evidence, not lifetime event count.
    state.state_quality = (state.event_intensity / 5.0).clamp(0.0, 1.0);
    Ok(())
}

async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok=s.events.count_documents(doc!{},None).await.is_ok();
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if ok {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok{"ok"}else{"degraded"},
       "mongodb":ok,"strategy_version":strategy_version,"live_order_submission":"not wired"})))
}
async fn ingest(State(s):State<AppState>, headers: HeaderMap, Json(event):Json<MarketEvent>)->impl IntoResponse{
    if !ingest_authorized(&headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"authorized service token required"}))); }
    if let Err(message)=validate_event(&event){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let _ingest_guard = s.ingest_lock.lock().await;
    // Preflight a new event against a private state clone before making it durable.
    // A stale/out-of-order/gapped event must never enter market_events.
    let mut preflight_snapshot: Option<SymbolState> = None;
    let mut preflight_error: Option<String> = None;
    let current_snapshot = match s.memory.read() {
        Ok(guard) => guard.get(&event.symbol).cloned().unwrap_or_default(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned"}))),
    };
    let mut candidate_snapshot = current_snapshot;
    match apply(&mut candidate_snapshot, &event, &s.decay) {
        Ok(()) => preflight_snapshot = Some(candidate_snapshot),
        Err(message) => {
            // A repeated event may correctly fail sequence/time preflight because it
            // was already applied. Let the idempotency path below recover it by ID.
            match s.events.find_one(doc!{"venue": &event.venue, "symbol": &event.symbol, "event_id": &event.event_id}, None).await {
                Ok(Some(_)) => preflight_error = Some(message),
                Ok(None) => return (StatusCode::CONFLICT, Json(serde_json::json!({"error":message}))),
                Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"event idempotency lookup unavailable"}))),
            }
        }
    }
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    let mut recovered_snapshot: Option<SymbolState> = None;
    let event_was_inserted = match s.events.insert_one(raw,None).await{
        Ok(_) => true,Err(e) if e.to_string().contains("E11000") => {
            // A duplicate may be an event persisted before a previous state write failed.
            // Re-process it unless the durable in-memory checkpoint already includes it.
            let in_memory = s.memory.read().ok().and_then(|guard| guard.get(&event.symbol).cloned());
            if let Some(state) = in_memory.filter(|state| state.last_event_id.as_deref() == Some(event.event_id.as_str())) {
                recovered_snapshot = Some(state);
            } else {
                match s.states.find_one(doc!{"venue": &event.venue, "symbol": &event.symbol, "last_event_id": &event.event_id},
                    FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await {
                    Ok(Some(document)) => match mongodb::bson::from_document::<SymbolState>(document) {
                        Ok(state) => recovered_snapshot = Some(state),
                        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"persisted event state could not be recovered"}))),
                    },
                    Ok(None) => {},
                    Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state storage unavailable during duplicate recovery"}))),
                }
            }
            if recovered_snapshot.is_some() {
                match s.decisions.find_one(doc!{"event_id": &event.event_id, "symbol": &event.symbol, "venue": &event.venue}, None).await {
                    Ok(Some(_)) => return (StatusCode::OK, Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id}))),
                    Ok(None) => {},
                    Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision storage unavailable during duplicate recovery"}))),
                }
            }
        },
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    };
    let already_applied = recovered_snapshot.is_some();
    let mut snapshot = if let Some(recovered) = recovered_snapshot {
        recovered
    } else if event_was_inserted {
        match preflight_snapshot {
            Some(candidate) => candidate,
            None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"preflight state unavailable"}))),
        }
    } else {
        let guard = match s.memory.read() {
            Ok(v) => v,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned"}))),
        };
        guard.get(&event.symbol).cloned().unwrap_or_default()
    };
    // If preflight failed, the event already existed and is handled through the
    // recovery path. Do not mutate or persist a failed candidate state.
    if let Some(message) = preflight_error {
        if recovered_snapshot.is_none() {
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"duplicate event state requires recovery","detail":message})));
        }
    }
    let decay = s.decay;
    if snapshot.last_event_id.as_deref() != Some(event.event_id.as_str()) {
        if let Err(message) = apply(&mut snapshot, &event, &decay) {
            return (StatusCode::CONFLICT, Json(serde_json::json!({"error":message})));
        }
    }
    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let max_age_ms = strategy.max_state_age_ms;
    let fresh_spread = snapshot.spread_ts_ms
        .filter(|ts| event.ts_ms.saturating_sub(*ts) >= 0 && event.ts_ms.saturating_sub(*ts) <= max_age_ms)
        .and(snapshot.spread_bps);
    let fresh_funding = snapshot.funding_ts_ms
        .filter(|ts| event.ts_ms.saturating_sub(*ts) >= 0 && event.ts_ms.saturating_sub(*ts) <= max_age_ms)
        .and(snapshot.funding_rate);
    let fresh_book_imbalance = snapshot.spread_ts_ms
        .filter(|ts| event.ts_ms.saturating_sub(*ts) >= 0 && event.ts_ms.saturating_sub(*ts) <= max_age_ms)
        .map(|_| snapshot.book_imbalance).unwrap_or(0.0);
    let fresh_open_interest_change = snapshot.open_interest_ts_ms
        .filter(|ts| event.ts_ms.saturating_sub(*ts) >= 0 && event.ts_ms.saturating_sub(*ts) <= max_age_ms)
        .map(|_| snapshot.open_interest_change).unwrap_or(0.0);
    let fresh_basis = snapshot.basis_ts_ms
        .filter(|ts| event.ts_ms.saturating_sub(*ts) >= 0 && event.ts_ms.saturating_sub(*ts) <= max_age_ms)
        .and(snapshot.basis);
    let arrival_ts_ms = if env::var("QOIN_REPLAY_MODE").map(|v| v == "true").unwrap_or(false) {
        event.received_at_ms
    } else {
        Some(chrono::Utc::now().timestamp_millis())
    };
    let state_age_ms = arrival_ts_ms.map(|arrival| arrival.saturating_sub(event.ts_ms));
    let is_entry_tick = matches!(event.event_type.as_str(), "trade" | "book");
    let mut context = DecisionContext {
        expected_edge_bps: env::var("QOIN_EXPECTED_EDGE_BPS").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0),
        fee_bps: env::var("QOIN_FEE_BPS").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v >= 0.0),
        slippage_bps: env::var("QOIN_SLIPPAGE_BPS").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v >= 0.0),
        state_age_ms,
        state_quality: Some(snapshot.state_quality),
        persistence_ticks: snapshot.candidate_persistence_ticks,
        feed_sequence_valid: snapshot.sequence_valid,
        is_entry_tick,
        position_side: None,
        regime_allowed: env::var("QOIN_REGIME_ALLOWED").map(|v| v == "true").unwrap_or(false),
    };
    let candidate = match strategy.decide_with_forces(&snapshot.symbol, snapshot.order_flow_imbalance, fresh_book_imbalance,
        fresh_funding, fresh_spread, snapshot.ewma_abs_return, snapshot.event_intensity,
        if snapshot.liquidation_size > f64::EPSILON { (snapshot.liquidation_pressure / snapshot.liquidation_size).clamp(-1.0, 1.0) } else { 0.0 },
        fresh_open_interest_change, fresh_basis, &context) {
        Ok(v) => v, Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":e})))
    };
    if is_entry_tick && !already_applied {
        if candidate.candidate_action == "buy" || candidate.candidate_action == "sell" {
            if snapshot.candidate_action.as_deref() == Some(candidate.candidate_action) {
                snapshot.candidate_persistence_ticks = snapshot.candidate_persistence_ticks.saturating_add(1);
            } else {
                snapshot.candidate_action = Some(candidate.candidate_action.to_owned());
                snapshot.candidate_persistence_ticks = 1;
            }
        } else {
            snapshot.candidate_action = None;
            snapshot.candidate_persistence_ticks = 0;
        }
    }
    context.persistence_ticks = snapshot.candidate_persistence_ticks;
    let decision: EdgeDecision = match strategy.decide_with_forces(&snapshot.symbol, snapshot.order_flow_imbalance, fresh_book_imbalance,
        fresh_funding, fresh_spread, snapshot.ewma_abs_return, snapshot.event_intensity,
        if snapshot.liquidation_size > f64::EPSILON { (snapshot.liquidation_pressure / snapshot.liquidation_size).clamp(-1.0, 1.0) } else { 0.0 },
        fresh_open_interest_change, fresh_basis, &context) {
        Ok(v) => v, Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":e})))
    };
    let mut state_doc = match mongodb::bson::to_document(&snapshot) {
        Ok(value) => value,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state serialization failed"}))),
    };
    state_doc.insert("stored_at", mongodb::bson::DateTime::now());
    state_doc.insert("event_id", event.event_id.clone());
    if let Err(e) = s.states.insert_one(state_doc, None).await {
        tracing::error!(error=%e,"state snapshot persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
    }
    let mut risk_context = risk::RiskContext::default();
    risk_context.sequence_gap = !snapshot.sequence_valid;
    risk_context.duplicate_or_out_of_order = false;
    risk_context.kill_switch = env::var("QOIN_GLOBAL_KILL_SWITCH").map(|v| v != "false" && v != "0").unwrap_or(true);
    let risk_limits = risk::RiskLimits::from_env().unwrap_or_else(|error| { tracing::warn!(%error, "risk policy incomplete; all entries remain vetoed"); risk::RiskLimits::target_policy() });
    let risk_assessment = risk::evaluate_entry(&risk_limits, &risk_context);
    let mut decision_doc=match mongodb::bson::to_document(&decision) { Ok(value) => value, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"decision serialization failed"}))) };
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    decision_doc.insert("event_id", event.event_id.clone());
    decision_doc.insert("venue", event.venue.clone());
    decision_doc.insert("risk_gate", mongodb::bson::to_bson(&risk_assessment).unwrap_or(mongodb::bson::Bson::Null));
    decision_doc.insert("execution_allowed", false);
    decision_doc.insert("effective_action", if risk_assessment.allowed { decision.action } else { "wait" });
    decision_doc.insert("gate_context", doc!{
        "expected_edge_bps": context.expected_edge_bps.map(mongodb::bson::Bson::Double).unwrap_or(mongodb::bson::Bson::Null),
        "fee_bps": context.fee_bps.map(mongodb::bson::Bson::Double).unwrap_or(mongodb::bson::Bson::Null),
        "slippage_bps": context.slippage_bps.map(mongodb::bson::Bson::Double).unwrap_or(mongodb::bson::Bson::Null),
        "state_age_ms": context.state_age_ms.map(mongodb::bson::Bson::Int64).unwrap_or(mongodb::bson::Bson::Null),
        "state_quality": context.state_quality.map(mongodb::bson::Bson::Double).unwrap_or(mongodb::bson::Bson::Null),
        "persistence_ticks": context.persistence_ticks as i64,
        "feed_sequence_valid": context.feed_sequence_valid,
        "is_entry_tick": context.is_entry_tick,
        "regime_allowed": context.regime_allowed,
        "position_side": match context.position_side { Some(qoin_vector_engine::PositionSide::Long) => "long", Some(qoin_vector_engine::PositionSide::Short) => "short", None => "none" }
    });
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    match s.memory.write() {
        Ok(mut guard) => { guard.insert(event.symbol.clone(), snapshot.clone()); }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned after persistence"}))),
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision,"gate_context":{"expected_edge_bps":context.expected_edge_bps,"fee_bps":context.fee_bps,"slippage_bps":context.slippage_bps,"state_age_ms":context.state_age_ms,"state_quality":context.state_quality,"persistence_ticks":context.persistence_ticks,"feed_sequence_valid":context.feed_sequence_valid,"is_entry_tick":context.is_entry_tick,"regime_allowed":context.regime_allowed},"risk_gate":risk_assessment.clone(),"effective_action":if risk_assessment.allowed {decision.action} else {"wait"},"execution_allowed":false})))
}
async fn get_state(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    if let Ok(g)=s.memory.read(){if let Some(v)=g.get(&symbol){return(StatusCode::OK,Json(serde_json::to_value(v).unwrap_or_default()));}}
    match s.states.find_one(doc!{"symbol":&symbol},FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"event_count":0,"order_flow_imbalance":0.0,"state_quality":0.0}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state storage unavailable"})))
    }
}
async fn get_vector(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    match s.decisions.find_one(doc!{"symbol":&symbol},FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"action":"wait","reason":"no decision available"}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision storage unavailable"})))
    }
}
async fn get_strategy(State(s):State<AppState>)->impl IntoResponse{
    match s.active_strategy.read(){Ok(v)=>(StatusCode::OK,Json(serde_json::to_value(&*v).unwrap_or_default())),
        Err(_)=>(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))}
}
async fn put_strategy(State(s):State<AppState>, headers: HeaderMap, Json(strategy):Json<EdgeStrategy>)->impl IntoResponse{
    if !admin_authorized(&headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"authorized service token required"}))); }
    if let Err(message)=strategy.validate(){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let mut stored=strategy.clone();
    stored.updated_at_ms=chrono::Utc::now().timestamp_millis();
    let doc=match mongodb::bson::to_document(&stored){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"strategy serialization failed"})))};
    let options=UpdateOptions::builder().upsert(true).build();
    if let Err(e)=s.strategies.update_one(doc!{"strategy_key":STRATEGY_KEY},doc!{"$set":doc},Some(options)).await{
        tracing::error!(error=%e,"strategy map persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"strategy map persistence failed"})));
    }
    match s.active_strategy.write(){Ok(mut current)=>*current=stored.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned after persistence"})))}
    (StatusCode::OK,Json(serde_json::json!({"status":"strategy_updated","strategy":stored,"live_trading":false})))
}
#[tokio::main]
async fn main(){
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init();
    let uri=env::var("QOIN_MONGODB_URI").unwrap_or_else(|_|"mongodb://127.0.0.1:27017".into());
    let client=Client::with_uri_str(&uri).await.expect("connect to MongoDB");
    let db=client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_|"qoin".into()));
    db.run_command(doc!{"ping":1},None).await.expect("MongoDB ping");
    let events:Collection<Document>=db.collection("market_events");
    let states:Collection<Document>=db.collection("market_states");
    let decisions:Collection<Document>=db.collection("trade_decisions");
    let strategies:Collection<Document>=db.collection("edge_strategy");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).partial_filter_expression(doc!{"event_id":{"$exists":true}}).build()).build(),None).await.expect("decision idempotency index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    let baseline=EdgeStrategy::baseline();
    let active=match strategies.find_one(doc!{"strategy_key":STRATEGY_KEY},None).await.expect("load edge strategy map"){
        Some(doc)=>match mongodb::bson::from_document::<EdgeStrategy>(doc){
            Ok(mut value)=>{
                if value.migrate_legacy_features() {
                    value.validate().expect("migrated edge strategy map must validate");
                    let migrated=mongodb::bson::to_document(&value).expect("serialize migrated strategy map");
                    strategies.update_one(doc!{"strategy_key":STRATEGY_KEY},doc!{"$set":migrated},None).await.expect("persist strategy map migration");
                }
                value.validate().expect("stored edge strategy must validate");
                value
            },
            Err(e)=>panic!("stored edge strategy map is invalid: {e}")
        },
        None=>{
            let d=mongodb::bson::to_document(&baseline).expect("serialize baseline strategy map");
            strategies.insert_one(d,None).await.expect("seed baseline strategy map");
            baseline
        }
    };
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded active edge strategy map from MongoDB");
    let mut recovered = HashMap::new();
    let mut symbols = states.distinct("symbol", None, None).await.expect("load state symbols");
    for value in symbols.drain(..) {
        if let Some(symbol) = value.as_str() {
            if let Some(mut document) = states.find_one(doc!{"symbol":symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await.expect("load latest state") {
                let legacy_venue_missing = document.get_str("venue").is_err();
                let last_event_id = document.get_str("event_id").ok().map(str::to_owned)
                    .or_else(|| document.get_str("last_event_id").ok().map(str::to_owned));
                let recovered_venue = if legacy_venue_missing {
                    if let Some(event_id) = last_event_id {
                        events.find_one(doc!{"symbol":symbol,"event_id":event_id},None).await
                            .expect("recover legacy state venue from event log")
                            .and_then(|event| event.get_str("venue").ok().map(str::to_owned))
                    } else { None }
                } else { document.get_str("venue").ok().map(str::to_owned) };
                document.remove("_id"); document.remove("stored_at"); document.remove("event_id");
                if let Ok(mut snapshot) = mongodb::bson::from_document::<SymbolState>(document) {
                    if snapshot.venue.is_empty() {
                        if let Some(venue) = recovered_venue { snapshot.venue = venue; }
                    }
                    recovered.insert(symbol.to_owned(), snapshot);
                }
            }
        }
    }
    let decay = DecayConfig::from_env().expect("all state half-life settings must be finite and positive");
    let state=AppState{events,states,decisions,strategies,memory:Arc::new(RwLock::new(recovered)),active_strategy:Arc::new(RwLock::new(active)),ingest_lock:Arc::new(Mutex::new(())),decay};
    let app=Router::new().route("/health",get(health)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}


#[cfg(test)]
mod tests {
    use super::*;

    fn test_decay(half_life_ms: f64) -> DecayConfig {
        DecayConfig { flow_half_life_ms: half_life_ms, event_intensity_half_life_ms: half_life_ms,
            liquidation_half_life_ms: half_life_ms, volatility_half_life_ms: half_life_ms }
    }

    fn event(id: &str, ts_ms: i64, event_type: &str) -> MarketEvent {
        MarketEvent {
            event_id: id.into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms, received_at_ms: None, sequence: None, price: None, size: Some(2.0),
            side: Some("buy".into()), bid: None, ask: None, bid_size: None, ask_size: None,
            funding_rate: None, open_interest: None, basis: None, liquidation_side: None,
        }
    }

    #[test]
    fn flow_and_intensity_decay_by_half_life() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("one", 1_000, "trade"), &test_decay(1_000.0)).unwrap();
        apply(&mut state, &event("two", 2_000, "trade"), &test_decay(1_000.0)).unwrap();
        assert!((state.signed_flow - 3.0).abs() < 1e-9);
        assert!((state.event_intensity - 1.5).abs() < 1e-9);
        assert!((state.total_trade_size - 3.0).abs() < 1e-9);
    }

    #[test]
    fn book_imbalance_does_not_inflate_trade_denominator() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("trade", 1_000, "trade"), &test_decay(30_000.0)).unwrap();
        let mut book = event("book", 2_000, "book");
        book.bid = Some(99.0); book.ask = Some(101.0);
        book.bid_size = Some(9.0); book.ask_size = Some(1.0);
        apply(&mut state, &book, &test_decay(30_000.0)).unwrap();
        assert!((state.total_trade_size - 2.0 * 0.5_f64.powf(1_000.0 / 30_000.0)).abs() < 1e-9);
        assert!((state.order_flow_imbalance - 1.0).abs() < 1e-9);
        assert!((state.book_imbalance - 0.8).abs() < 1e-9);
    }

    #[test]
    fn out_of_order_event_is_rejected_without_mutation() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("later", 2_000, "trade"), &test_decay(30_000.0)).unwrap();
        let before = state.clone();
        assert!(apply(&mut state, &event("earlier", 1_000, "trade"), &test_decay(30_000.0)).is_err());
        assert_eq!(state.last_event_id, before.last_event_id);
        assert_eq!(state.signed_flow, before.signed_flow);
    }

    #[test]
    fn trade_schema_rejects_missing_side_and_zero_size() {
        let mut invalid = event("bad", 1_000, "trade");
        invalid.side = None;
        assert!(validate_event(&invalid).is_err());
        invalid.side = Some("buy".into());
        invalid.size = Some(0.0);
        assert!(validate_event(&invalid).is_err());
    }
    #[test]
    fn sequence_gaps_are_rejected_without_advancing_state() {
        let mut state = SymbolState::default();
        let mut first = event("one", 1_000, "trade"); first.sequence = Some(10);
        apply(&mut state, &first, &test_decay(30_000.0)).unwrap();
        let before = state.clone();
        let mut gap = event("gap", 2_000, "trade"); gap.sequence = Some(12);
        assert!(apply(&mut state, &gap, &test_decay(30_000.0)).unwrap_err().contains("sequence gap"));
        assert_eq!(state.last_sequences.get("test:trade"), before.last_sequences.get("test:trade"));
        let mut next = event("next", 2_000, "trade"); next.sequence = Some(11);
        apply(&mut state, &next, &test_decay(30_000.0)).unwrap();
        assert!(state.sequence_valid);
    }

    #[test]
    fn venue_mismatch_is_rejected_without_mixing_state() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("one", 1_000, "trade"), &DecayConfig::default()).unwrap();
        let before = state.clone();
        let mut other_venue = event("two", 2_000, "trade");
        other_venue.venue = "other".into();
        assert!(apply(&mut state, &other_venue, &DecayConfig::default()).unwrap_err().contains("venue mismatch"));
        assert_eq!(state.venue, before.venue);
        assert_eq!(state.last_event_id, before.last_event_id);
        assert_eq!(state.signed_flow, before.signed_flow);
    }

    #[test]
    fn missing_sequence_marks_feed_untrusted() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("one", 1_000, "trade"), &test_decay(30_000.0)).unwrap();
        assert!(!state.sequence_valid);
    }

    #[test]
    fn sequence_metadata_after_unsequenced_event_does_not_restore_trust() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("unsequenced", 1_000, "trade"), &test_decay(30_000.0)).unwrap();
        let mut sequenced = event("sequenced", 2_000, "trade");
        sequenced.sequence = Some(1);
        apply(&mut state, &sequenced, &test_decay(30_000.0)).unwrap();
        assert!(!state.sequence_valid, "sequence continuity requires an explicit resync after unknown gaps");
    }

    #[test]
    fn liquidation_pressure_is_separate_and_normalized() {
        let mut state = SymbolState::default();
        let mut long_liquidation = event("liq-long", 1_000, "liquidation");
        long_liquidation.size = Some(2.0);
        long_liquidation.liquidation_side = Some("long".into());
        apply(&mut state, &long_liquidation, &test_decay(30_000.0)).unwrap();
        assert_eq!(state.signed_flow, 0.0);
        assert_eq!(state.total_trade_size, 0.0);
        assert_eq!(state.liquidation_pressure, -2.0);
        let mut short_liquidation = event("liq-short", 2_000, "liquidation");
        short_liquidation.size = Some(2.0);
        short_liquidation.liquidation_side = Some("short".into());
        apply(&mut state, &short_liquidation, &test_decay(30_000.0)).unwrap();
        assert!(state.liquidation_pressure > 0.0);
        assert!(state.liquidation_pressure < 0.1);
    }

}
