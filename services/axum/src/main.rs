use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, IndexOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::{Arc, RwLock}};
use tower_http::trace::TraceLayer;

mod vector_engine;
use vector_engine::decide;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct MarketEvent {
    event_id: String,
    venue: String,
    symbol: String,
    event_type: String,
    ts_ms: i64,
    #[serde(default)] price: Option<f64>,
    #[serde(default)] size: Option<f64>,
    #[serde(default)] side: Option<String>,
    #[serde(default)] bid: Option<f64>,
    #[serde(default)] ask: Option<f64>,
    #[serde(default)] bid_size: Option<f64>,
    #[serde(default)] ask_size: Option<f64>,
    #[serde(default)] funding_rate: Option<f64>,
    #[serde(default)] open_interest: Option<f64>,
    #[serde(default)] liquidation_side: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct SymbolState {
    symbol: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    last_price: Option<f64>,
    spread_bps: Option<f64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    event_intensity: f64,
    ewma_abs_return: f64,
    funding_rate: Option<f64>,
    open_interest: Option<f64>,
    liquidation_events: usize,
    last_liquidation_side: Option<String>,
    state_quality: f64,
}

#[derive(Clone)]
struct AppState {
    events: Collection<Document>,
    states: Collection<Document>,
    decisions: Collection<Document>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }

fn valid_number(value: Option<f64>) -> bool {
    value.map(|v| v.is_finite()).unwrap_or(true)
}
fn validate(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    for (name, value) in [("price", e.price), ("size", e.size), ("bid", e.bid),
        ("ask", e.ask), ("bid_size", e.bid_size), ("ask_size", e.ask_size),
        ("funding_rate", e.funding_rate), ("open_interest", e.open_interest)] {
        if !valid_number(value) { return Err(format!("{name} must be finite")); }
    }
    if let (Some(bid), Some(ask)) = (e.bid, e.ask) {
        if bid < 0.0 || ask < bid { return Err("book prices must be non-negative and ask >= bid".into()); }
    }
    if [e.price, e.size, e.bid, e.ask, e.bid_size, e.ask_size, e.open_interest]
        .into_iter().flatten().any(|v| v < 0.0) {
        return Err("prices, sizes, and open interest must be non-negative".into());
    }
    Ok(())
}
fn apply(state: &mut SymbolState, event: &MarketEvent) {
    state.symbol = event.symbol.clone();
    if let Some(price) = event.price {
        if price > 0.0 {
            if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
                let abs_return = (price / previous).ln().abs();
                state.ewma_abs_return = 0.94 * state.ewma_abs_return + 0.06 * abs_return;
            }
            state.last_price = Some(price);
        }
    }
    if let (Some(bid), Some(ask)) = (event.bid, event.ask) {
        let mid = (bid + ask) / 2.0;
        state.spread_bps = if mid > 0.0 { Some((ask - bid) / mid * 10_000.0) } else { None };
        if let (Some(bs), Some(as_)) = (event.bid_size, event.ask_size) {
            if bs + as_ > 0.0 { state.signed_flow += (bs - as_) / (bs + as_) * 0.1; }
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
            state.liquidation_events += 1;
            state.event_intensity += 2.0;
            state.last_liquidation_side = event.liquidation_side.clone();
            if event.liquidation_side.as_deref() == Some("long") { state.signed_flow -= event.size.unwrap_or(0.0); }
            if event.liquidation_side.as_deref() == Some("short") { state.signed_flow += event.size.unwrap_or(0.0); }
        }
        "funding" => if event.funding_rate.is_some() { state.funding_rate = event.funding_rate; },
        "open_interest" => if event.open_interest.is_some() { state.open_interest = event.open_interest; },
        "book" => {}
        _ => {}
    }
    state.event_count += 1;
    state.last_event_ts_ms = Some(state.last_event_ts_ms.unwrap_or(0).max(event.ts_ms));
    if state.total_trade_size > 0.0 {
        state.order_flow_imbalance = (state.signed_flow / state.total_trade_size).clamp(-1.0, 1.0);
    }
    state.state_quality = ((state.event_count as f64 / 100.0).sqrt()).min(1.0);
}
async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok = s.events.count_documents(doc! {}, None).await.is_ok();
    (if ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE },
        Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok {"ok"} else {"degraded"},"mongodb":ok,"live_order_submission":"not wired"})))
}
async fn ingest(State(s): State<AppState>, Json(event): Json<MarketEvent>) -> impl IntoResponse {
    if let Err(message) = validate(&event) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":message})));
    }
    let raw = match mongodb::bson::to_document(&event) {
        Ok(v) => v, Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"event serialization failed"}))),
    };
    match s.events.insert_one(raw, None).await {
        Ok(_) => {},
        Err(e) if e.to_string().contains("E11000") => {
            return (StatusCode::OK, Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id})));
        },
        Err(e) => {
            tracing::error!(error=%e,"market event persistence failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"event persistence failed"})));
        }
    }
    let snapshot = {
        let mut guard = match s.memory.write() {
            Ok(v) => v, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned"}))),
        };
        let state = guard.entry(event.symbol.clone()).or_default();
        apply(state, &event);
        state.clone()
    };
    let mut state_doc = mongodb::bson::to_document(&snapshot).unwrap_or_default();
    state_doc.insert("stored_at", mongodb::bson::DateTime::now());
    if let Err(e) = s.states.insert_one(state_doc, None).await {
        tracing::error!(error=%e,"state snapshot persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
    }
    let decision = decide(&snapshot.symbol, snapshot.order_flow_imbalance, snapshot.funding_rate,
        snapshot.spread_bps, snapshot.ewma_abs_return, snapshot.event_intensity,
        snapshot.liquidation_events, snapshot.last_liquidation_side.as_deref());
    let mut decision_doc = mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("stored_at", mongodb::bson::DateTime::now());
    if let Err(e) = s.decisions.insert_one(decision_doc, None).await {
        tracing::error!(error=%e,"vector decision persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    (StatusCode::OK, Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision})))
}
async fn get_state(State(s): State<AppState>, Query(q): Query<SymbolQuery>) -> impl IntoResponse {
    let symbol = q.symbol.unwrap_or_else(|| "BTCUSDT".into());
    if let Ok(guard) = s.memory.read() {
        if let Some(value) = guard.get(&symbol) {
            return (StatusCode::OK, Json(serde_json::to_value(value).unwrap_or_default()));
        }
    }
    match s.states.find_one(doc!{"symbol":&symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await {
        Ok(Some(mut d)) => { d.remove("_id"); d.remove("stored_at"); (StatusCode::OK, Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default())) },
        Ok(None) => (StatusCode::OK, Json(serde_json::json!({"symbol":symbol,"event_count":0,"order_flow_imbalance":0.0,"state_quality":0.0}))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state storage unavailable"}))),
    }
}
async fn get_vector(State(s): State<AppState>, Query(q): Query<SymbolQuery>) -> impl IntoResponse {
    let symbol = q.symbol.unwrap_or_else(|| "BTCUSDT".into());
    match s.decisions.find_one(doc!{"symbol":&symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await {
        Ok(Some(mut d)) => { d.remove("_id"); d.remove("stored_at"); (StatusCode::OK, Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default())) },
        Ok(None) => (StatusCode::OK, Json(serde_json::json!({"symbol":symbol,"action":"wait","reason":"no decision available"}))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision storage unavailable"}))),
    }
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into())).init();
    let uri = env::var("QOIN_MONGODB_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".into());
    let client = Client::with_uri_str(&uri).await.expect("connect to MongoDB");
    let db = client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_| "qoin".into()));
    db.run_command(doc!{"ping":1}, None).await.expect("MongoDB ping");
    let events: Collection<Document> = db.collection("market_events");
    let states: Collection<Document> = db.collection("market_states");
    let decisions: Collection<Document> = db.collection("trade_decisions");
    let event_index = mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build();
    events.create_index(event_index, None).await.expect("create event deduplication index");
    let app_state = AppState { events, states, decisions, memory: Arc::new(RwLock::new(HashMap::new())) };
    let app = Router::new().route("/health", get(health))
        .route("/internal/events", post(ingest))
        .route("/internal/state", get(get_state))
        .route("/internal/vector", get(get_vector))
        .with_state(app_state).layer(TraceLayer::new_for_http());
    let addr = env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_| "127.0.0.1:8081".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}");
    axum::serve(listener, app).await.expect("Axum server failed");
}
