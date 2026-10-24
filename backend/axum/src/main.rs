use axum::{
    extract::{Query, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use futures_util::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::{Arc, RwLock}};
use tower_http::trace::TraceLayer;

use qoin_vector_engine::strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};
mod risk;

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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct SymbolState {
    symbol: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    last_decay_ts_ms: Option<i64>,
    last_book_ts_ms: Option<i64>,
    book_imbalance: Option<f64>,
    last_price: Option<f64>,
    last_price_ts_ms: Option<i64>,
    spread_bps: Option<f64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    event_intensity: f64,
    ewma_abs_return: f64,
    funding_rate: Option<f64>,
    open_interest: Option<f64>,
    liquidation_events: f64,
    last_liquidation_side: Option<String>,
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
    ingest_lock: Arc<tokio::sync::Mutex<()>>,
    pending_signals: Arc<RwLock<HashMap<String, (String, usize)>>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }

fn authorized(headers: &HeaderMap, token_env: &str) -> bool {
    let expected = match env::var(token_env) {
        Ok(value) if !value.trim().is_empty() => format!("Bearer {}", value),
        _ => return false,
    };
    headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok())
        .map(|provided| provided == expected).unwrap_or(false)
}
fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    match e.event_type.as_str() {
        "trade" => {
            if !matches!(e.side.as_deref(), Some("buy" | "sell")) || e.size.map(|v| v <= 0.0).unwrap_or(true) {
                return Err("trade events require positive size and side buy/sell".into());
            }
        }
        "book" => {
            if e.bid.is_none() || e.ask.is_none() || e.bid_size.is_none() || e.ask_size.is_none() {
                return Err("book events require bid, ask, bid_size, and ask_size".into());
            }
        }
        "liquidation" => {
            if !matches!(e.liquidation_side.as_deref(), Some("long" | "short")) || e.size.map(|v| v <= 0.0).unwrap_or(true) {
                return Err("liquidation events require positive size and liquidation_side long/short".into());
            }
        }
        "funding" if e.funding_rate.is_some() => {},
        "open_interest" if e.open_interest.is_some() => {},
        "funding" | "open_interest" => return Err("funding and open-interest events require their corresponding value".into()),
        _ => return Err("unsupported event_type".into()),
    }
    for (name, value) in [("price",e.price),("size",e.size),("bid",e.bid),("ask",e.ask),
        ("bid_size",e.bid_size),("ask_size",e.ask_size),("funding_rate",e.funding_rate),("open_interest",e.open_interest)] {
        if !valid_number(value) { return Err(format!("{name} must be finite")); }
    }
    if let (Some(bid),Some(ask))=(e.bid,e.ask) {
        if bid < 0.0 || ask < bid { return Err("book prices must be non-negative and ask >= bid".into()); }
    }
    if [e.price,e.size,e.bid,e.ask,e.bid_size,e.ask_size,e.open_interest].into_iter().flatten().any(|v| v < 0.0) {
   const FLOW_HALF_LIFE_MS: f64 = 5_000.0;
const INTENSITY_HALF_LIFE_MS: f64 = 2_000.0;
const LIQUIDATION_HALF_LIFE_MS: f64 = 30_000.0;

fn decay(value: f64, elapsed_ms: i64, half_life_ms: f64) -> f64 {
    if elapsed_ms <= 0 { return value; }
    value * 2.0_f64.powf(-(elapsed_ms as f64) / half_life_ms)
}

fn apply(state: &mut SymbolState, event: &MarketEvent) {
    state.symbol = event.symbol.clone();
    // All rolling quantities are bounded by explicit half-lives. Event-time
    // ordering is checked before this function, so elapsed time cannot be negative.
    if let Some(previous_ts) = state.last_decay_ts_ms {
        let elapsed = event.ts_ms.saturating_sub(previous_ts);
        state.signed_flow = decay(state.signed_flow, elapsed, FLOW_HALF_LIFE_MS);
        state.total_trade_size = decay(state.total_trade_size, elapsed, FLOW_HALF_LIFE_MS);
        state.event_intensity = decay(state.event_intensity, elapsed, INTENSITY_HALF_LIFE_MS);
        state.liquidation_events = decay(state.liquidation_events, elapsed, LIQUIDATION_HALF_LIFE_MS);
    }
    state.last_decay_ts_ms = Some(event.ts_ms);

    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            let abs_return = (price / previous).ln().abs();
            // Return volatility also decays with a stated 60-second half-life.
            let elapsed = state.last_event_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts)).unwrap_or(0);
            state.ewma_abs_return = decay(state.ewma_abs_return, elapsed, 60_000.0)
                + 0.06 * abs_return;
        }
        state.last_price = Some(price);
        state.last_price_ts_ms = Some(event.ts_ms);
    }
    if let (Some(bid), Some(ask)) = (event.bid, event.ask) {
        let mid = (bid + ask) / 2.0;
        state.spread_bps = if mid > 0.0 { Some((ask - bid) / mid * 10_000.0) } else { None };
        state.last_book_ts_ms = Some(event.ts_ms);
        if let (Some(bs), Some(as_)) = (event.bid_size, event.ask_size) {
            if bs + as_ > 0.0 {
                // Keep book imbalance separate: it is dimensionless and must not
                // enter the trade-flow numerator without a matching denominator.
                state.book_imbalance = Some(((bs - as_) / (bs + as_)).clamp(-1.0, 1.0));
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
            state.liquidation_events += 1.0;
            state.event_intensity += 2.0;
            state.last_liquidation_side = event.liquidation_side.clone();
            if event.liquidation_side.as_deref() == Some("long") {
                state.signed_flow -= event.size.unwrap_or(0.0);
                state.total_trade_size += event.size.unwrap_or(0.0);
            }
            if event.liquidation_side.as_deref() == Some("short") {
                state.signed_flow += event.size.unwrap_or(0.0);
                state.total_trade_size += event.size.unwrap_or(0.0);
            }
        }
        "funding" => if event.funding_rate.is_some() { state.funding_rate = event.funding_rate; },
        "open_interest" => if event.open_interest.is_some() { state.open_interest = event.open_interest; },
        "book" => {},
        _ => {}
    }
    state.event_count += 1;
    state.last_event_ts_ms = Some(event.ts_ms);
    state.order_flow_imbalance = if state.total_trade_size > f64::EPSILON {
        (state.signed_flow / state.total_trade_size).clamp(-1.0, 1.0)
    } else { 0.0 };
    // Quality is based on actual fresh data availability, not lifetime event count.
    let book_fresh = state.last_book_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts) <= 2_000).unwrap_or(false);
    let price_fresh = state.last_price_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts) <= 2_000).unwrap_or(false);
    state.state_quality = (if book_fresh { 0.5 } else { 0.0 }) + (if price_fresh { 0.5 } else { 0.0 });
}
async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok=s.events.count_documents(doc!{},None).await.is_ok();
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if ok {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok{"ok"}else{"degraded"},
       "mongodb":ok,"strategy_version":strategy_version,"live_order_submission":"not wired"})))
}
async fn ingest(State(s): State<AppState>, headers: HeaderMap, Json(event): Json<MarketEvent>) -> impl IntoResponse {
    if !authorized(&headers, "QOIN_INGEST_TOKEN") {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"valid ingestion bearer token required"})));
    }
    let _ingest_guard = s.ingest_lock.lock().await;
    if let Err(message) = validate_event(&event) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":message})));
    }

    let event_filter = doc!{"venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id};
    let existing_event = match s.events.find_one(event_filter.clone(), None).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"event idempotency lookup failed"}))),
    };
    let existing_decision = match s.decisions.find_one(doc!{"source_event_id":&event.event_id}, None).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision idempotency lookup failed"}))),
    };
    if existing_decision.is_some() {
        let _ = s.events.update_one(event_filter, doc!{"$set":doc!{"processing_status":"complete"}}, None).await;
        return (StatusCode::OK, Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id})));
    }

    // Do not allow a later event to pass an unresolved event-log record. The
    // original event can be retried safely because its ID is the idempotency key.
    let unresolved = match s.events.find_one(doc!{
        "symbol":&event.symbol, "processing_status":"pending", "event_id":{"$ne":&event.event_id}
    }, None).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"pending-event lookup failed"}))),
    };
    if unresolved.is_some() {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"earlier event is pending recovery; retry it before advancing this symbol"})));
    }

    let existing_snapshot = match s.states.find_one(doc!{"source_event_id":&event.event_id}, None).await {
        Ok(value) => value,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"snapshot idempotency lookup failed"}))),
    };
    let snapshot = if let Some(mut persisted) = existing_snapshot {
        persisted.remove("_id");
        persisted.remove("stored_at");
        match mongodb::bson::from_document::<SymbolState>(persisted) {
            Ok(value) => value,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"persisted event snapshot cannot be recovered"}))),
        }
    } else {
        let current = match s.memory.read() {
            Ok(guard) => guard.get(&event.symbol).cloned(),
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned"}))),
        };
        let current = if let Some(state) = current { Some(state) } else {
            match s.states.find_one(doc!{"symbol":&event.symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await {
                Ok(Some(mut doc)) => {
                    doc.remove("_id"); doc.remove("stored_at");
                    mongodb::bson::from_document::<SymbolState>(doc).ok()
                }
                Ok(None) => None,
                Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state recovery lookup failed"}))),
            }
        };
        if let Some(previous) = current.as_ref().and_then(|state| state.last_event_ts_ms) {
            if event.ts_ms <= previous {
                return (StatusCode::CONFLICT, Json(serde_json::json!({
                    "error":"out-of-order event rejected", "event_id":event.event_id,
                    "event_ts_ms":event.ts_ms, "last_event_ts_ms":previous
                })));
            }
        }
        let mut candidate = current.unwrap_or_default();
        apply(&mut candidate, &event);
        if existing_event.is_none() {
            let mut raw = match mongodb::bson::to_document(&event) {
                Ok(value) => value,
                Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"event serialization failed"}))),
            };
            raw.insert("processing_status", "pending");
            if let Err(error) = s.events.insert_one(raw, None).await {
                if !error.to_string().contains("E11000") {
                    tracing::error!(error=%error,"event persistence failed");
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"event persistence failed"})));
                }
            }
        }
        let mut state_doc = match mongodb::bson::to_document(&candidate) {
            Ok(value) => value,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state serialization failed","event_persisted":true}))),
        };
        state_doc.insert("stored_at", mongodb::bson::DateTime::now());
        state_doc.insert("source_event_id", event.event_id.clone());
        if let Err(error) = s.states.insert_one(state_doc, None).await {
            tracing::error!(error=%error,"state snapshot persistence failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
        }
        let _ = s.events.update_one(event_filter.clone(), doc!{"$set":doc!{"processing_status":"state_persisted"}}, None).await;
        candidate
    };

    match s.memory.write() {
        Ok(mut guard) => { guard.insert(event.symbol.clone(), snapshot.clone()); }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned after durable snapshot"}))),
    }
    let strategy = match s.active_strategy.read() {
        Ok(value) => value.clone(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"strategy lock poisoned"}))),
    };
    let mut decision: EdgeDecision = match strategy.decide(
        &snapshot.symbol, snapshot.order_flow_imbalance, snapshot.book_imbalance, snapshot.funding_rate,
        snapshot.spread_bps, snapshot.ewma_abs_return, snapshot.event_intensity,
        snapshot.liquidation_events, snapshot.last_liquidation_side.as_deref()
    ) {
        Ok(value) => value,
        Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":error}))),
    };

    if decision.action == "buy" || decision.action == "sell" {
        let eligible_event = matches!(event.event_type.as_str(), "trade" | "book");
        let fresh_book = snapshot.last_book_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts) <= 2_000).unwrap_or(false);
        let fresh_price = snapshot.last_price_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts) <= 2_000).unwrap_or(false);
        if !eligible_event {
            decision.reason = "entry gate requires a trade or book event".into();
            decision.action = "wait";
        } else if !fresh_book || !fresh_price || snapshot.state_quality < 1.0 {
            decision.reason = "entry gate vetoed: book/price state is stale or incomplete".into();
            decision.action = "wait";
        } else {
            match s.pending_signals.write() {
                Ok(mut pending) => {
                    let entry = pending.entry(event.symbol.clone()).or_insert((decision.action.to_owned(), 0));
                    if entry.0 == decision.action { entry.1 += 1; }
                    else { *entry = (decision.action.to_owned(), 1); }
                    if entry.1 < 3 {
                        decision.reason = format!("entry persistence not met ({}/3)", entry.1);
                        decision.action = "wait";
                    }
                }
                Err(_) => {
                    decision.reason = "entry gate vetoed: persistence state unavailable".into();
                    decision.action = "wait";
                }
            }
        }
    } else if let Ok(mut pending) = s.pending_signals.write() {
        pending.remove(&event.symbol);
    }

    // No reconciled equity/stop/exposure context is wired into this service yet.
    // Thus the independent risk gate vetoes every otherwise eligible entry.
    if decision.action == "buy" || decision.action == "sell" {
        if let Err(veto) = risk::evaluate_entry(None, risk::RiskLimits::default()) {
            decision.risk_veto = Some(format!("{veto:?}"));
            decision.reason = format!("risk gate vetoed entry: {veto:?}; portfolio context unavailable");
            decision.action = "wait";
        }
    }
    let mut decision_doc = match mongodb::bson::to_document(&decision) {
        Ok(value) => value,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"decision serialization failed"}))),
    };
    decision_doc.insert("stored_at", mongodb::bson::DateTime::now());
    decision_doc.insert("source_event_id", event.event_id.clone());
    if let Err(error) = s.decisions.insert_one(decision_doc, None).await {
        tracing::error!(error=%error,"decision persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    if let Err(error) = s.events.update_one(event_filter, doc!{"$set":doc!{"processing_status":"complete"}}, None).await {
        tracing::error!(error=%error,"event completion status update failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision persisted but event completion status failed"})));
    }
    (StatusCode::OK, Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision})))
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
async fn put_strategy(State(s):State<AppState>,headers:HeaderMap,Json(strategy):Json<EdgeStrategy>)->impl IntoResponse{
    if !authorized(&headers, "QOIN_ADMIN_TOKEN") {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"valid admin bearer token required"})));
    }
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

async fn recover_pending_events(
    events: &Collection<Document>,
    states: &Collection<Document>,
    decisions: &Collection<Document>,
    strategy: &EdgeStrategy,
) -> Result<HashMap<String, SymbolState>, String> {
    let options = mongodb::options::FindOptions::builder().sort(doc!{"symbol":1,"ts_ms":1,"event_id":1}).build();
    let mut cursor = events.find(doc!{"processing_status":{"$in":["pending","state_persisted"]}}, options).await
        .map_err(|e| format!("query pending events: {e}"))?;
    let mut recovered: HashMap<String, SymbolState> = HashMap::new();
    while let Some(raw) = cursor.try_next().await.map_err(|e| format!("read pending event: {e}"))? {
        let event: MarketEvent = mongodb::bson::from_document(raw)
            .map_err(|e| format!("decode pending event: {e}"))?;
        let persisted = states.find_one(doc!{"source_event_id":&event.event_id}, None).await
            .map_err(|e| format!("lookup pending event snapshot: {e}"))?;
        let snapshot = if let Some(mut doc) = persisted {
            doc.remove("_id"); doc.remove("stored_at");
            mongodb::bson::from_document::<SymbolState>(doc)
                .map_err(|e| format!("decode pending event snapshot: {e}"))?
        } else {
            let current = if let Some(value) = recovered.get(&event.symbol) {
                Some(value.clone())
            } else {
                match states.find_one(doc!{"symbol":&event.symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await
                    .map_err(|e| format!("load prior state during recovery: {e}"))? {
                    Some(mut doc) => {
                        doc.remove("_id"); doc.remove("stored_at");
                        Some(mongodb::bson::from_document::<SymbolState>(doc)
                            .map_err(|e| format!("decode prior state during recovery: {e}"))?)
                    }
                    None => None,
                }
            };
            if let Some(previous) = current.as_ref().and_then(|state| state.last_event_ts_ms) {
                if event.ts_ms <= previous {
                    return Err(format!("pending event {} is older than recovered state; refusing unsafe replay", event.event_id));
                }
            }
            let mut candidate = current.unwrap_or_default();
            apply(&mut candidate, &event);
            let mut state_doc = mongodb::bson::to_document(&candidate)
                .map_err(|e| format!("serialize recovered state: {e}"))?;
            state_doc.insert("stored_at", mongodb::bson::DateTime::now());
            state_doc.insert("source_event_id", event.event_id.clone());
            states.insert_one(state_doc, None).await
                .map_err(|e| format!("persist recovered state: {e}"))?;
            candidate
        };
        recovered.insert(event.symbol.clone(), snapshot.clone());

        let prior_decision = decisions.find_one(doc!{"source_event_id":&event.event_id}, None).await
            .map_err(|e| format!("lookup recovered decision: {e}"))?;
        if prior_decision.is_none() {
            let mut decision = strategy.decide(
                &snapshot.symbol, snapshot.order_flow_imbalance, snapshot.book_imbalance, snapshot.funding_rate,
                snapshot.spread_bps, snapshot.ewma_abs_return, snapshot.event_intensity,
                snapshot.liquidation_events, snapshot.last_liquidation_side.as_deref()
            ).map_err(|e| format!("recompute recovered decision: {e}"))?;
            if decision.action == "buy" || decision.action == "sell" {
                decision.risk_veto = Some("InvalidOrMissingContext".into());
                decision.reason = "recovery risk veto: reconciled portfolio context unavailable".into();
                decision.action = "wait";
            }
            let mut decision_doc = mongodb::bson::to_document(&decision)
                .map_err(|e| format!("serialize recovered decision: {e}"))?;
            decision_doc.insert("stored_at", mongodb::bson::DateTime::now());
            decision_doc.insert("source_event_id", event.event_id.clone());
            decisions.insert_one(decision_doc, None).await
                .map_err(|e| format!("persist recovered decision: {e}"))?;
        }
        events.update_one(
            doc!{"venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id},
            doc!{"$set":doc!{"processing_status":"complete"}},
            None
        ).await.map_err(|e| format!("mark recovered event complete: {e}"))?;
    }
    Ok(recovered)
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
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"source_event_id":1})
        .options(IndexOptions::builder().unique(true).partial_filter_expression(doc!{"source_event_id":{"$exists":true}}).build()).build(),None)
        .await.expect("state event idempotency index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"source_event_id":1})
        .options(IndexOptions::builder().unique(true).partial_filter_expression(doc!{"source_event_id":{"$exists":true}}).build()).build(),None)
        .await.expect("decision event idempotency index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    let baseline=EdgeStrategy::baseline();
    let active=match strategies.find_one(doc!{"strategy_key":STRATEGY_KEY},None).await.expect("load edge strategy map"){
        Some(doc)=>match mongodb::bson::from_document::<EdgeStrategy>(doc){
            Ok(v)=>{
                v.validate().expect("stored edge strategy must validate");
                // Migrate only the known hand-authored baseline, never a user-tuned map.
                if matches!(v.version.as_str(), "edge-map-v1" | "edge-map-v2") && v.status == "research_baseline" {
                    let mut migrated = EdgeStrategy::baseline();
                    migrated.updated_at_ms = chrono::Utc::now().timestamp_millis();
                    let d = mongodb::bson::to_document(&migrated).expect("serialize migrated baseline");
                    strategies.update_one(doc!{"strategy_key":STRATEGY_KEY}, doc!{"$set":d}, None)
                        .await.expect("persist versioned attention baseline migration");
                    migrated
                } else { v }
            },
            Err(e)=>panic!("stored edge strategy map is invalid: {e}")
        },
        None=>{
            let mut baseline = baseline;
            baseline.updated_at_ms = chrono::Utc::now().timestamp_millis();
            let d=mongodb::bson::to_document(&baseline).expect("serialize baseline strategy map");
            strategies.insert_one(d,None).await.expect("seed baseline edge strategy map");
            baseline
        }
    };
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded active edge strategy map from MongoDB");
    let recovered_memory = recover_pending_events(&events, &states, &decisions, &active).await
        .expect("pending event recovery must succeed before serving traffic");
    let state=AppState{events,states,decisions,strategies,memory:Arc::new(RwLock::new(recovered_memory)),active_strategy:Arc::new(RwLock::new(active)),ingest_lock:Arc::new(tokio::sync::Mutex::new(())),pending_signals:Arc::new(RwLock::new(HashMap::new()))};
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
mod state_tests {
    use super::*;

    fn event(ts_ms: i64, event_type: &str) -> MarketEvent {
        MarketEvent {
            event_id: format!("e-{ts_ms}"), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms, price: None, size: None, side: None,
            bid: None, ask: None, bid_size: None, ask_size: None, funding_rate: None,
            open_interest: None, liquidation_side: None,
        }
    }

    #[test]
    fn signed_flow_and_size_decay_with_same_half_life() {
        let mut state = SymbolState::default();
        state.signed_flow = 4.0;
        state.total_trade_size = 4.0;
        state.event_intensity = 4.0;
        state.liquidation_events = 2.0;
        state.last_decay_ts_ms = Some(1_000);
        state.last_event_ts_ms = Some(1_000);
        apply(&mut state, &event(6_000, "book"));
        assert!((state.signed_flow - 2.0).abs() < 1e-9);
        assert!((state.total_trade_size - 2.0).abs() < 1e-9);
        assert!((state.order_flow_imbalance - 1.0).abs() < 1e-9);
        assert!(state.event_intensity < 1.0);
        assert!(state.liquidation_events < 2.0 && state.liquidation_events > 1.0);
    }

    #[test]
    fn book_imbalance_does_not_pollute_trade_flow_denominator() {
        let mut state = SymbolState::default();
        let mut e = event(1_000, "book");
        e.bid = Some(99.0); e.ask = Some(101.0);
        e.bid_size = Some(9.0); e.ask_size = Some(1.0);
        apply(&mut state, &e);
        assert_eq!(state.book_imbalance, Some(0.8));
        assert_eq!(state.signed_flow, 0.0);
        assert_eq!(state.total_trade_size, 0.0);
        assert_eq!(state.order_flow_imbalance, 0.0);
    }

    #[test]
    fn decay_does_not_increase_values() {
        for value in [0.0, 0.5, 1.0, 100.0] {
            assert!(decay(value, 1_000, 5_000.0) <= value);
        }
    }
}
