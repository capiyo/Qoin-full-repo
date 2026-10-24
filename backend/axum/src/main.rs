use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::{Arc, RwLock}};
use tower_http::trace::TraceLayer;

mod edge_strategy;
mod vector_engine;
use edge_strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};

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
struct SymbolState {
    symbol: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    last_decay_ts_ms: Option<i64>,
    last_book_ts_ms: Option<i64>,
    book_imbalance: Option<f64>,
    last_price: Option<f64>,
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
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
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
    let book_fresh = state.last_book_ts_ms.map(|ts| event.ts_ms.saturating_sub(ts) <= 5_000).unwrap_or(false);
    let price_fresh = state.last_price.is_some();
    state.state_quality = (if book_fresh { 0.5 } else { 0.0 }) + (if price_fresh { 0.5 } else { 0.0 });
}
ize).clamp(-1.0,1.0);}
    state.state_quality=((state.event_count as f64/100.0).sqrt()).min(1.0);
}
async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok=s.events.count_documents(doc!{},None).await.is_ok();
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if ok {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok{"ok"}else{"degraded"},
       "mongodb":ok,"strategy_version":strategy_version,"live_order_submission":"not wired"})))
}
async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    // Serialize ingestion per process so event-time validation, persistence, and
    // state commit cannot race with another event for the same symbol.
    let _ingest_guard = s.ingest_lock.lock().await;
    if let Err(message)=validate_event(&event){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    match s.events.insert_one(raw,None).await{
        Ok(_)=>{},Err(e) if e.to_string().contains("E11000")=>return(StatusCode::OK,Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id}))),
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    }
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
    let snapshot = candidate;
    let mut state_doc = match mongodb::bson::to_document(&snapshot) {
        Ok(doc) => doc,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state serialization failed","event_persisted":true}))),
    };
    state_doc.insert("stored_at", mongodb::bson::DateTime::now());
    state_doc.insert("source_event_id", event.event_id.clone());
    if let Err(e)=s.states.insert_one(state_doc,None).await{
        tracing::error!(error=%e,"state snapshot persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
    }
    // Only publish the candidate to process memory after the durable snapshot exists.
    match s.memory.write() {
        Ok(mut guard) => { guard.insert(event.symbol.clone(), snapshot.clone()); }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned after durable snapshot"}))),
    }
    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let decision:EdgeDecision=match strategy.decide(&snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
        snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,snapshot.liquidation_events,snapshot.last_liquidation_side.as_deref()){
        Ok(v)=>v,Err(e)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":e})))
    };
    let mut decision_doc=mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision})))
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
async fn put_strategy(State(s):State<AppState>,Json(strategy):Json<EdgeStrategy>)->impl IntoResponse{
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
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
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
    let state=AppState{events,states,decisions,strategies,memory:Arc::new(RwLock::new(HashMap::new())),active_strategy:Arc::new(RwLock::new(active)),ingest_lock:Arc::new(tokio::sync::Mutex::new(()))};
    let app=Router::new().route("/health",get(health)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
