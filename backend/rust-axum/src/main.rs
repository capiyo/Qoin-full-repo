use axum::{
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, FindOptions, IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use futures_util::TryStreamExt;
use std::{collections::HashMap, env, sync::{Arc, RwLock}};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod backtest;
mod operating_cycle;
mod paper_session;
mod edge_strategy;
mod jepa;
mod vector_engine;
use edge_strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};
use backtest::{BacktestRequest, run_backtest};
use operating_cycle::{CycleConfig, OperatingCycle};
use paper_session::PaperSession;
use jepa::{FeatureHistory, JepaClient, JepaPredictionRecord};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MarketEvent {
    pub(crate) event_id: String,
    pub(crate) venue: String,
    pub(crate) symbol: String,
    pub(crate) event_type: String,
    pub(crate) ts_ms: i64,
    #[serde(default)] pub(crate) price: Option<f64>,
    #[serde(default)] pub(crate) size: Option<f64>,
    #[serde(default)] pub(crate) side: Option<String>,
    #[serde(default)] pub(crate) bid: Option<f64>,
    #[serde(default)] pub(crate) ask: Option<f64>,
    #[serde(default)] pub(crate) bid_size: Option<f64>,
    #[serde(default)] pub(crate) ask_size: Option<f64>,
    #[serde(default)] pub(crate) funding_rate: Option<f64>,
    #[serde(default)] pub(crate) open_interest: Option<f64>,
    #[serde(default)] pub(crate) liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct SymbolState {
    pub(crate) venue: String,
    pub(crate) symbol: String,
    pub(crate) event_count: usize,
    pub(crate) last_event_ts_ms: Option<i64>,
    pub(crate) last_price: Option<f64>,
    pub(crate) spread_bps: Option<f64>,
    pub(crate) signed_flow: f64,
    pub(crate) total_trade_size: f64,
    pub(crate) order_flow_imbalance: f64,
    pub(crate) event_intensity: f64,
    pub(crate) ewma_abs_return: f64,
    pub(crate) funding_rate: Option<f64>,
    pub(crate) open_interest: Option<f64>,
    pub(crate) liquidation_events: usize,
    pub(crate) last_liquidation_side: Option<String>,
    pub(crate) state_quality: f64,
}
#[derive(Clone)]
struct AppState {
    events: Collection<Document>,
    states: Collection<Document>,
    decisions: Collection<Document>,
    strategies: Collection<Document>,
    jepa_predictions: Collection<Document>,
    backtest_runs: Collection<Document>,
    backtest_decisions: Collection<Document>,
    operating_cycles: Collection<Document>,
    paper_sessions: Collection<Document>,
    paper_session_decisions: Collection<Document>,
    paper_session_trades: Collection<Document>,
    active_cycle: Arc<Mutex<OperatingCycle>>,
    active_paper_session: Arc<Mutex<PaperSession>>,
    jepa_client: Option<JepaClient>,
    jepa_history: Arc<RwLock<HashMap<String, FeatureHistory>>>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String>, venue: Option<String> }
#[derive(Deserialize)]
struct PaperHistoryQuery { symbol: Option<String>, limit: Option<i64> }

async fn persist_operating_cycle(s: &AppState, cycle: &OperatingCycle) -> Result<(), String> {
    let mut document = mongodb::bson::to_document(cycle).map_err(|error| error.to_string())?;
    document.insert("cycle_id", "active");
    let options = UpdateOptions::builder().upsert(true).build();
    s.operating_cycles.update_one(doc! {"cycle_id": "active"}, doc! {"$set": document}, Some(options))
        .await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn persist_paper_session(s: &AppState, session: &PaperSession) -> Result<(), String> {
    let mut document = mongodb::bson::to_document(session).map_err(|error| error.to_string())?;
    document.insert("session_key", "active");
    let options = UpdateOptions::builder().upsert(true).build();
    s.paper_sessions.update_one(doc! {"session_key": "active"}, doc! {"$set": document}, Some(options))
        .await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn get_paper_session_history(Query(query): Query<PaperHistoryQuery>, State(s): State<AppState>) -> impl IntoResponse {
    let cycle_id = s.active_paper_session.lock().await.snapshot().cycle_id;
    let mut filter = Document::new();
    if let Some(cycle_id) = cycle_id { filter.insert("cycle_id", cycle_id); }
    if let Some(symbol) = query.symbol { filter.insert("symbol", symbol); }
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut decisions_cursor = match s.paper_session_decisions.find(filter.clone(), FindOptions::builder().sort(doc! {"ts_ms": -1}).limit(limit).build()).await {
        Ok(cursor) => cursor,
        Err(error) => { tracing::error!(error=%error, "paper Mirror history query failed"); return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper Mirror history unavailable"}))); }
    };
    let mut decisions = Vec::new();
    loop {
        match decisions_cursor.try_next().await {
            Ok(Some(mut document)) => { document.remove("_id"); document.remove("stored_at"); decisions.push(document); }
            Ok(None) => break,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper Mirror history read interrupted"}))),
        }
    }
    let mut trades_cursor = match s.paper_session_trades.find(filter, FindOptions::builder().sort(doc! {"exit_ts_ms": -1}).limit(limit).build()).await {
        Ok(cursor) => cursor,
        Err(error) => { tracing::error!(error=%error, "paper trade history query failed"); return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade history unavailable"}))); }
    };
    let mut trades = Vec::new();
    loop {
        match trades_cursor.try_next().await {
            Ok(Some(mut document)) => { document.remove("_id"); document.remove("stored_at"); trades.push(document); }
            Ok(None) => break,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade history read interrupted"}))),
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"decisions":decisions,"trades":trades})))
}

async fn get_paper_session(State(s): State<AppState>) -> impl IntoResponse {
    let cycle = s.active_cycle.lock().await;
    let cycle_snapshot = cycle.snapshot();
    let cycle_active = cycle_snapshot.active && cycle_snapshot.config.mode == "paper";
    drop(cycle);
    let mut session = s.active_paper_session.lock().await;
    if !cycle_active && session.is_active() {
        session.stop();
        if let Err(error) = persist_paper_session(&s, &session).await {
            tracing::error!(error=%error, "paper session completion persistence failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session state unavailable"})));
        }
    }
    (StatusCode::OK, Json(serde_json::to_value(session.snapshot()).unwrap_or_default()))
}

async fn get_operating_cycle(State(s): State<AppState>) -> impl IntoResponse {
    let cycle = s.active_cycle.lock().await;
    (StatusCode::OK, Json(serde_json::to_value(cycle.snapshot()).unwrap_or_default()))
}

async fn configure_operating_cycle(State(s): State<AppState>, Json(config): Json<CycleConfig>) -> impl IntoResponse {
    let mut cycle = s.active_cycle.lock().await;
    let snapshot = match cycle.configure(config) {
        Ok(value) => value,
        Err(message) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":message}))),
    };
    if let Err(error) = persist_operating_cycle(&s, &cycle).await {
        tracing::error!(error=%error, "operating cycle configuration persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"cycle persistence failed"})));
    }
    (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()))
}

async fn start_operating_cycle(State(s): State<AppState>) -> impl IntoResponse {
    let mut cycle = s.active_cycle.lock().await;
    if cycle.snapshot().active {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"an operating cycle is already active"})));
    }
    let config = cycle.snapshot().config;
    let cycle_id = format!("cycle-{}", chrono::Utc::now().timestamp_millis());
    let paper = if config.mode == "paper" {
        match PaperSession::start(cycle_id, &config.paper_accounts) {
            Ok(session) => session,
            Err(message) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":message}))),
        }
    } else { PaperSession::default() };
    let snapshot = cycle.start();
    if let Err(error) = persist_operating_cycle(&s, &cycle).await {
        tracing::error!(error=%error, "operating cycle start persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"cycle persistence failed"})));
    }
    drop(cycle);
    let mut active_paper = s.active_paper_session.lock().await;
    *active_paper = paper;
    if let Err(error) = persist_paper_session(&s, &active_paper).await {
        tracing::error!(error=%error, "paper session start persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session persistence failed"})));
    }
    (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()))
}

async fn stop_operating_cycle(State(s): State<AppState>) -> impl IntoResponse {
    let mut cycle = s.active_cycle.lock().await;
    let snapshot = cycle.stop();
    if let Err(error) = persist_operating_cycle(&s, &cycle).await {
        tracing::error!(error=%error, "operating cycle stop persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"cycle persistence failed"})));
    }
    drop(cycle);
    let mut paper = s.active_paper_session.lock().await;
    paper.stop();
    if let Err(error) = persist_paper_session(&s, &paper).await {
        tracing::error!(error=%error, "paper session stop persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session persistence failed"})));
    }
    (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()))
}

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn is_out_of_order(state: &SymbolState, event: &MarketEvent) -> bool {
    state.last_event_ts_ms.map(|last| event.ts_ms < last).unwrap_or(false)
}
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    for (name, value) in [("price",e.price),("size",e.size),("bid",e.bid),("ask",e.ask),
        ("bid_size",e.bid_size),("ask_size",e.ask_size),("funding_rate",e.funding_rate),("open_interest",e.open_interest)] {
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
pub(crate) fn apply(state: &mut SymbolState, event: &MarketEvent) {
    state.venue = event.venue.clone();
    state.symbol = event.symbol.clone();
    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            state.ewma_abs_return = 0.94 * state.ewma_abs_return + 0.06 * (price / previous).ln().abs();
        }
        state.last_price = Some(price);
    }
    if let (Some(bid),Some(ask))=(event.bid,event.ask) {
        let mid=(bid+ask)/2.0;
        state.spread_bps=if mid>0.0 {Some((ask-bid)/mid*10_000.0)} else {None};
        if let (Some(bs),Some(as_))=(event.bid_size,event.ask_size) {
            if bs+as_>0.0 {state.signed_flow+=(bs-as_)/(bs+as_)*0.1;}
        }
    }
    match event.event_type.as_str() {
        "trade" => { let size=event.size.unwrap_or(0.0); match event.side.as_deref() {
            Some("buy")=>state.signed_flow+=size, Some("sell")=>state.signed_flow-=size, _=>{} }
            state.total_trade_size+=size; state.event_intensity+=1.0; }
        "liquidation" => { state.liquidation_events+=1; state.event_intensity+=2.0;
            state.last_liquidation_side=event.liquidation_side.clone();
            if event.liquidation_side.as_deref()==Some("long"){state.signed_flow-=event.size.unwrap_or(0.0);}
            if event.liquidation_side.as_deref()==Some("short"){state.signed_flow+=event.size.unwrap_or(0.0);} }
        "funding" => if event.funding_rate.is_some(){state.funding_rate=event.funding_rate;},
        "open_interest" => if event.open_interest.is_some(){state.open_interest=event.open_interest;},
        "book" => {}, _ => {}
    }
    state.event_count+=1;
    state.last_event_ts_ms=Some(state.last_event_ts_ms.unwrap_or(0).max(event.ts_ms));
    if state.total_trade_size>0.0 {state.order_flow_imbalance=(state.signed_flow/state.total_trade_size).clamp(-1.0,1.0);}
    state.state_quality=((state.event_count as f64/100.0).sqrt()).min(1.0);
}
#[cfg(test)]
mod venue_state_tests {
    use super::*;
    fn event(venue: &str, symbol: &str, ts_ms: i64, size: f64) -> MarketEvent {
        MarketEvent { event_id: format!("{venue}-{ts_ms}"), venue: venue.into(), symbol: symbol.into(), event_type: "trade".into(),
            ts_ms, price: Some(100.0), size: Some(size), side: Some("buy".into()), bid: Some(99.9), ask: Some(100.1),
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None, liquidation_side: None }
    }
    #[test]
    fn symbol_state_keeps_venues_separate() {
        let mut first = SymbolState::default();
        let mut second = SymbolState::default();
        apply(&mut first, &event("broker-a", "EURUSD", 1_000, 2.0));
        apply(&mut second, &event("broker-b", "EURUSD", 1_000, 5.0));
        assert_eq!(first.venue, "broker-a");
        assert_eq!(second.venue, "broker-b");
        assert_ne!(first.total_trade_size, second.total_trade_size);
    }
    #[test]
    fn older_event_is_not_a_forward_update() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("broker-a", "EURUSD", 2_000, 2.0));
        assert!(is_out_of_order(&state, &event("broker-a", "EURUSD", 1_000, 1.0)));
    }
}

async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok=s.events.count_documents(doc!{},None).await.is_ok();
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if ok {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok{"ok"}else{"degraded"},
       "mongodb":ok,"strategy_version":strategy_version,
       "jepa_research_inference":if s.jepa_client.is_some(){"enabled"}else{"disabled"},
       "live_order_submission":"not wired"})))
}
async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    if let Err(message)=validate_event(&event){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    match s.events.insert_one(raw,None).await{
        Ok(_)=>{},Err(e) if e.to_string().contains("E11000")=>return(StatusCode::OK,Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id}))),
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    }
    let state_key = format!("{}::{}", event.venue, event.symbol);
    let (snapshot, out_of_order) = {
        let mut guard=match s.memory.write(){Ok(v)=>v,Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})))};
        let state=guard.entry(state_key.clone()).or_default();
        let late = is_out_of_order(state, &event);
        if !late { apply(state,&event); }
        (state.clone(), late)
    };
    if out_of_order {
        return (StatusCode::ACCEPTED, Json(serde_json::json!({
            "status":"stored_out_of_order_event_not_applied", "event_id":event.event_id,
            "venue":event.venue, "symbol":event.symbol,
            "note":"Event is retained for chronological historical replay but excluded from the forward state and paper session."
        })));
    }
    let mut state_doc=mongodb::bson::to_document(&snapshot).unwrap_or_default();
    state_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.states.insert_one(state_doc,None).await{
        tracing::error!(error=%e,"state snapshot persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
    }
    // Optional JEPA sidecar is observational only. Failure or lag never blocks or
    // changes the existing Version 1 vector decision.
    let jepa_window = if let Some(client) = s.jepa_client.as_ref() {
        match s.jepa_history.write() {
            Ok(mut histories) => histories.entry(state_key.clone()).or_default()
                .push_if_due(event.ts_ms, jepa::features_from_state(&snapshot),
                    client.context_steps(), client.min_interval_ms()),
            Err(_) => {
                tracing::warn!("JEPA history lock poisoned; skipping research prediction");
                None
            }
        }
    } else {
        None
    };
    if let (Some(client), Some(rows)) = (s.jepa_client.clone(), jepa_window) {
        let predictions = s.jepa_predictions.clone();
        let symbol = snapshot.symbol.clone();
        let event_ts_ms = event.ts_ms;
        tokio::spawn(async move {
            match client.predict(&rows).await {
                Ok(prediction) => {
                    let record = JepaPredictionRecord {
                        symbol: symbol.clone(),
                        event_ts_ms,
                        predicted_at_ms: chrono::Utc::now().timestamp_millis(),
                        model_version: prediction.model_version,
                        horizon_steps: prediction.horizon_steps,
                        feature_names: prediction.feature_names,
                        predicted_latent_state: prediction.predicted_latent_state,
                        note: prediction.note,
                    };
                    match mongodb::bson::to_document(&record) {
                        Ok(document) => {
                            if let Err(error) = predictions.insert_one(document, None).await {
                                tracing::warn!(symbol=%symbol, error=%error,
                                    "could not persist JEPA research prediction");
                            }
                        }
                        Err(error) => tracing::warn!(symbol=%symbol, error=%error,
                            "could not serialize JEPA research prediction"),
                    }
                }
                Err(error) => tracing::warn!(symbol=%symbol, error=%error,
                    "JEPA research prediction unavailable; vector decision unchanged"),
            }
        });
    }

    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let decision:EdgeDecision=match strategy.decide(&snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
        snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,snapshot.liquidation_events,snapshot.last_liquidation_side.as_deref()){
        Ok(v)=>v,Err(e)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":e})))
    };
    let mut decision_doc=mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("venue", &event.venue);
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    let cycle_phase = {
        let mut cycle = s.active_cycle.lock().await;
        if cycle.is_active() {
            cycle.record_observation();
            let directional_score = decision.decision_vector[0] - decision.decision_vector[1];
            let signed_alignment = (decision.angle_radians * 2.0).cos();
            cycle.record_signal_evaluation(directional_score, signed_alignment, decision.action, &decision.features, &decision.attention, &decision.attended_features, &strategy.bullish_weights, &strategy.bearish_weights);
            if let Err(error) = persist_operating_cycle(&s, &cycle).await {
                tracing::error!(error=%error, symbol=%event.symbol, event_id=%event.event_id, "operating cycle Mirror persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"cycle Mirror persistence failed", "event_persisted":true,
                    "state_persisted":true, "decision_persisted":true, "event_id":event.event_id
                })));
            }
        }
        cycle.snapshot().phase
    };
    if cycle_phase == "paper_session" {
        let mut paper = s.active_paper_session.lock().await;
        if let Some((mirror, trade)) = paper.process_event(&event, &decision) {
            let mut mirror_doc = match mongodb::bson::to_document(&mirror) {
                Ok(value) => value,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"paper Mirror serialization failed"}))),
            };
            mirror_doc.insert("stored_at", mongodb::bson::DateTime::now());
            if let Err(error) = s.paper_session_decisions.insert_one(mirror_doc, None).await {
                tracing::error!(error=%error, event_id=%event.event_id, "paper Mirror decision persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper Mirror decision persistence failed","event_persisted":true,"event_id":event.event_id})));
            }
            if let Some(trade) = trade {
                let mut trade_doc = match mongodb::bson::to_document(&trade) {
                    Ok(value) => value,
                    Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"paper trade serialization failed"}))),
                };
                trade_doc.insert("stored_at", mongodb::bson::DateTime::now());
                if let Err(error) = s.paper_session_trades.insert_one(trade_doc, None).await {
                    tracing::error!(error=%error, event_id=%event.event_id, "paper trade persistence failed");
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade persistence failed","event_persisted":true,"event_id":event.event_id})));
                }
            }
            if let Err(error) = persist_paper_session(&s, &paper).await {
                tracing::error!(error=%error, event_id=%event.event_id, "paper account state persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper account state persistence failed","event_persisted":true,"event_id":event.event_id})));
            }
        }
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision,"paper_phase":cycle_phase})))
}
async fn run_paper_backtest(State(s): State<AppState>, Json(request): Json<BacktestRequest>) -> impl IntoResponse {
    if let Err(message) = request.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": message})));
    }
    let strategy = match s.active_strategy.read() {
        Ok(value) => value.clone(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"strategy lock poisoned"}))),
    };
    let warmup_start = request.start_ts_ms.saturating_sub((request.warmup_hours as i64).saturating_mul(3_600_000));
    let filter = doc! {"symbol": &request.symbol, "venue": &request.venue, "ts_ms": {"$gte": warmup_start, "$lte": request.end_ts_ms}};
    let options = FindOptions::builder()
        .sort(doc! {"ts_ms": 1, "event_id": 1})
        .limit(request.max_events as i64 + 1)
        .build();
    let mut cursor = match s.events.find(filter, options).await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(error=%error, "backtest event query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"market event history unavailable"})));
        }
    };
    let mut events = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                document.remove("_id");
                match mongodb::bson::from_document::<MarketEvent>(document) {
                    Ok(event) => events.push(event),
                    Err(error) => {
                        tracing::error!(error=%error, "stored market event could not be decoded");
                        return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored market event has an invalid schema"})));
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, "backtest event cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"market event replay interrupted"})));
            }
        }
    }
    if events.len() > request.max_events {
        return (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({
            "error":"event limit reached; narrow the time range or raise max_events up to 100000",
            "max_events":request.max_events
        })));
    }
    let run_id = format!("{}-{}-{}", request.venue, request.symbol, chrono::Utc::now().timestamp_micros());
    let mut report = match run_backtest(&request, &events, &strategy, run_id.clone()) {
        Ok(value) => value,
        Err(message) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":message}))),
    };
    let mut batch = Vec::with_capacity(500);
    for decision in &report.decisions {
        let mut document = match mongodb::bson::to_document(decision) {
            Ok(value) => value,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"Mirror decision serialization failed"}))),
        };
        document.insert("run_id", &run_id);
        batch.push(document);
        if batch.len() == 500 {
            if let Err(error) = s.backtest_decisions.insert_many(std::mem::take(&mut batch), None).await {
                tracing::error!(error=%error, run_id=%run_id, "Mirror decision batch persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"Mirror trace persistence failed","run_id":run_id})));
            }
            batch = Vec::with_capacity(500);
        }
    }
    if !batch.is_empty() {
        if let Err(error) = s.backtest_decisions.insert_many(batch, None).await {
            tracing::error!(error=%error, run_id=%run_id, "Mirror decision batch persistence failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"Mirror trace persistence failed","run_id":run_id})));
        }
    }
    let summary = match mongodb::bson::to_document(&serde_json::json!({
        "run_id": run_id,
        "symbol": report.symbol,
        "venue": report.venue,
        "quote_currency": report.quote_currency,
        "strategy_version": report.strategy_version,
        "start_ts_ms": report.start_ts_ms,
        "end_ts_ms": report.end_ts_ms,
        "initial_cash": report.initial_cash,
        "reinvest_profits": report.reinvest_profits,
        "final_equity": report.final_equity,
        "realized_pnl": report.realized_pnl,
        "unrealized_pnl": report.unrealized_pnl,
        "net_pnl": report.net_pnl,
        "return_pct": report.return_pct,
        "max_drawdown_pct": report.max_drawdown_pct,
        "event_count": report.event_count,
        "warmup_event_count": report.warmup_event_count,
        "signal_count": report.signal_count,
        "buy_signals": report.buy_signals,
        "sell_signals": report.sell_signals,
        "wait_signals": report.wait_signals,
        "executed_entries": report.executed_entries,
        "executed_exits": report.executed_exits,
        "skipped_entries": report.skipped_entries,
        "pending_order": report.pending_order,
        "pending_signal_ts_ms": report.pending_signal_ts_ms,
        "win_rate_pct": report.win_rate_pct,
        "profit_factor": report.profit_factor,
        "fees_paid": report.fees_paid,
        "trades": &report.trades,
        "open_position": &report.open_position,
        "assumptions": report.assumptions,
        "created_at": chrono::Utc::now().timestamp_millis()
    })) {
        Ok(value) => value,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"backtest report serialization failed","run_id":run_id}))),
    };
    if let Err(error) = s.backtest_runs.insert_one(summary, None).await {
        tracing::error!(error=%error, run_id=%run_id, "backtest report persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"backtest summary persistence failed","run_id":run_id})));
    }
    // Keep the complete trace in the immediate response as well as MongoDB.
    (StatusCode::OK, Json(serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({"run_id":run_id,"error":"report serialization failed"}))))
}

async fn get_backtest_run(Path(run_id): Path<String>, State(s): State<AppState>) -> impl IntoResponse {
    let summary = match s.backtest_runs.find_one(doc! {"run_id": &run_id}, None).await {
        Ok(Some(mut value)) => { value.remove("_id"); value }
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"backtest run not found"}))),
        Err(error) => {
            tracing::error!(error=%error, run_id=%run_id, "backtest history lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"backtest history unavailable"})));
        }
    };
    let options = FindOptions::builder().sort(doc! {"sequence": 1}).build();
    let mut cursor = match s.backtest_decisions.find(doc! {"run_id": &run_id}, options).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"Mirror trace unavailable","run_id":run_id}))),
    };
    let mut decisions = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut value)) => {
                value.remove("_id");
                decisions.push(value);
            }
            Ok(None) => break,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"Mirror trace read interrupted","run_id":run_id}))),
        }
    }
    let summary_value = mongodb::bson::from_document::<serde_json::Value>(summary).unwrap_or(serde_json::Value::Null);
    (StatusCode::OK, Json(serde_json::json!({"summary":summary_value,"decisions":decisions})))
}

async fn list_backtest_runs(Query(query): Query<SymbolQuery>, State(s): State<AppState>) -> impl IntoResponse {
    let filter = match query.symbol {
        Some(symbol) => doc! {"symbol": symbol},
        None => doc! {},
    };
    let options = FindOptions::builder().sort(doc! {"created_at": -1}).limit(50).build();
    let mut cursor = match s.backtest_runs.find(filter, options).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"backtest history unavailable"}))),
    };
    let mut runs = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut value)) => {
                value.remove("_id");
                runs.push(value);
            }
            Ok(None) => break,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"backtest history read interrupted"}))),
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"runs":runs})))
}

async fn get_state(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let venue=q.venue;
    if let Ok(g)=s.memory.read(){
        let found = if let Some(ref venue) = venue { g.get(&format!("{}::{}", venue, symbol)) }
            else { g.values().filter(|state| state.symbol == symbol).max_by_key(|state| state.last_event_ts_ms) };
        if let Some(value)=found { return (StatusCode::OK,Json(serde_json::to_value(value).unwrap_or_default())); }
    }
    let mut filter=doc!{"symbol":&symbol};
    if let Some(ref venue)=venue { filter.insert("venue", venue); }
    match s.states.find_one(filter,FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"venue":venue,"event_count":0,"order_flow_imbalance":0.0,"state_quality":0.0}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state storage unavailable"})))
    }
}
async fn get_vector(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let venue=q.venue;
    let mut filter=doc!{"symbol":&symbol};
    if let Some(ref venue)=venue { filter.insert("venue", venue); }
    match s.decisions.find_one(filter,FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"venue":venue,"action":"wait","reason":"no decision available"}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision storage unavailable"})))
    }
}
async fn get_jepa_prediction(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    if s.jepa_client.is_none() {
        return (StatusCode::OK,Json(serde_json::json!({
            "symbol":symbol,"status":"disabled","decision":null,
            "note":"Set QOIN_JEPA_URL to enable optional loopback research inference."
        })));
    }
    match s.jepa_predictions.find_one(doc!{"symbol":&symbol},
        FindOneOptions::builder().sort(doc!{"event_ts_ms":-1}).build()).await {
        Ok(Some(mut document)) => {
            document.remove("_id");
            (StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(document)
                .unwrap_or_default()))
        }
        Ok(None) => (StatusCode::OK,Json(serde_json::json!({
            "symbol":symbol,"status":"warming_up","decision":null,
            "note":"Waiting for a complete causal context window and successful model inference."
        }))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "error":"JEPA research prediction storage unavailable"
        })))
    }
}
async fn get_strategy(State(s):State<AppState>)->impl IntoResponse{
    match s.active_strategy.read(){Ok(v)=>(StatusCode::OK,Json(serde_json::to_value(&*v).unwrap_or_default())),
        Err(_)=>(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))}
}
async fn put_strategy(State(s):State<AppState>,Json(strategy):Json<EdgeStrategy>)->impl IntoResponse{
    let cycle = s.active_cycle.lock().await;
    if cycle.snapshot().active {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"strategy weights are locked while an operating cycle is active"})));
    }
    drop(cycle);
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
    let jepa_predictions:Collection<Document>=db.collection("jepa_predictions");
    let backtest_runs:Collection<Document>=db.collection("backtest_runs");
    let backtest_decisions:Collection<Document>=db.collection("backtest_decisions");
    let operating_cycles:Collection<Document>=db.collection("operating_cycles");
    let paper_sessions:Collection<Document>=db.collection("paper_sessions");
    let paper_session_decisions:Collection<Document>=db.collection("paper_session_decisions");
    let paper_session_trades:Collection<Document>=db.collection("paper_session_trades");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"ts_ms":1})
        .build(),None).await.expect("market event replay index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"stored_at":-1}).build(),None).await.expect("venue-isolated state lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"stored_at":-1}).build(),None).await.expect("venue-isolated decision lookup index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    backtest_runs.create_index(mongodb::IndexModel::builder().keys(doc!{"run_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("backtest run index");
    operating_cycles.create_index(mongodb::IndexModel::builder().keys(doc!{"cycle_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("operating cycle index");
    paper_sessions.create_index(mongodb::IndexModel::builder().keys(doc!{"session_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("paper session state index");
    paper_session_decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"cycle_id":1,"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("paper Mirror event index");
    paper_session_trades.create_index(mongodb::IndexModel::builder().keys(doc!{"cycle_id":1,"venue":1,"symbol":1,"exit_ts_ms":1}).build(),None).await.expect("paper trade history index");
    backtest_decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"run_id":1,"sequence":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("backtest decision index");
    jepa_predictions.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"symbol":1,"event_ts_ms":-1}).build(),None).await
        .expect("JEPA prediction lookup index");
    let baseline=EdgeStrategy::baseline();
    let active=match strategies.find_one(doc!{"strategy_key":STRATEGY_KEY},None).await.expect("load edge strategy map"){
        Some(doc)=>match mongodb::bson::from_document::<EdgeStrategy>(doc){Ok(v)=>{v.validate().expect("stored edge strategy must validate");v},
            Err(e)=>panic!("stored edge strategy map is invalid: {e}")},
        None=>{
            let d=mongodb::bson::to_document(&baseline).expect("serialize baseline strategy map");
            strategies.insert_one(d,None).await.expect("seed baseline edge strategy map");
            baseline
        }
    };
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded active edge strategy map from MongoDB");
    let jepa_client = match env::var("QOIN_JEPA_URL") {
        Ok(url) => {
            let context_steps = env::var("QOIN_JEPA_CONTEXT_STEPS").ok()
                .and_then(|value| value.parse::<usize>().ok()).unwrap_or(32);
            let min_interval_ms = env::var("QOIN_JEPA_MIN_INTERVAL_MS").ok()
                .and_then(|value| value.parse::<i64>().ok()).unwrap_or(1000);
            match JepaClient::new(&url, context_steps, min_interval_ms) {
                Ok(client) => {
                    tracing::info!(context_steps=client.context_steps(),
                        min_interval_ms=client.min_interval_ms(),
                        "optional JEPA research sidecar configured; predictions will not affect vector decisions");
                    Some(client)
                }
                Err(error) => {
                    tracing::warn!(error=%error,"invalid JEPA sidecar configuration; leaving JEPA disabled");
                    None
                }
            }
        }
        Err(_) => None,
    };
    let persisted_cycle = match operating_cycles.find_one(doc!{"cycle_id":"active"}, None).await {
        Ok(Some(mut document)) => {
            document.remove("_id");
            mongodb::bson::from_document::<OperatingCycle>(document).unwrap_or_default()
        }
        _ => OperatingCycle::default(),
    };
    let persisted_paper = match paper_sessions.find_one(doc!{"session_key":"active"}, None).await {
        Ok(Some(mut document)) => { document.remove("_id"); mongodb::bson::from_document::<PaperSession>(document).unwrap_or_default() }
        _ => PaperSession::default(),
    };
    let mut restored_memory = HashMap::new();
    let state_pipeline = vec![
        doc! {"$match": {"venue": {"$exists": true}, "symbol": {"$exists": true}}},
        doc! {"$sort": {"stored_at": -1}},
        doc! {"$group": {"_id": {"venue": "$venue", "symbol": "$symbol"}, "latest": {"$first": "$ROOT"}}},
    ];
    match states.aggregate(state_pipeline, None).await {
        Ok(mut cursor) => loop {
            match cursor.try_next().await {
                Ok(Some(row)) => {
                    if let Ok(mut latest) = row.get_document("latest").cloned() {
                        latest.remove("_id"); latest.remove("stored_at");
                        if let Ok(restored) = mongodb::bson::from_document::<SymbolState>(latest) {
                            restored_memory.insert(format!("{}::{}", restored.venue, restored.symbol), restored);
                        }
                    }
                }
                Ok(None) => break,
                Err(error) => { tracing::warn!(error=%error, "could not fully restore latest venue-specific market states"); break; }
            }
        },
        Err(error) => tracing::warn!(error=%error, "market-state restore unavailable; waiting for fresh events"),
    }
    tracing::info!(restored_symbols=restored_memory.len(), "restored latest venue-isolated market states");
    let state=AppState{
        events,states,decisions,strategies,jepa_predictions,backtest_runs,backtest_decisions,operating_cycles,
        paper_sessions,paper_session_decisions,paper_session_trades,
        active_cycle:Arc::new(Mutex::new(persisted_cycle)),
        active_paper_session:Arc::new(Mutex::new(persisted_paper)),jepa_client,
        jepa_history:Arc::new(RwLock::new(HashMap::new())),
        memory:Arc::new(RwLock::new(restored_memory)),
        active_strategy:Arc::new(RwLock::new(active))
    };
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT])
        .allow_headers([CONTENT_TYPE]);
    let app=Router::new().route("/health",get(health)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/jepa",get(get_jepa_prediction))
         .route("/internal/backtest",post(run_paper_backtest))
        .route("/internal/operating-cycle",get(get_operating_cycle).put(configure_operating_cycle))
        .route("/internal/operating-cycle/start",post(start_operating_cycle))
        .route("/internal/operating-cycle/stop",post(stop_operating_cycle))
        .route("/internal/paper-session",get(get_paper_session))
        .route("/internal/paper-session/history",get(get_paper_session_history))
        .route("/internal/backtests",get(list_backtest_runs))
        .route("/internal/backtests/:run_id",get(get_backtest_run))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(cors).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
