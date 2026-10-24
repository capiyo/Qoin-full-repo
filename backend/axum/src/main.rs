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
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;

mod risk;
#[cfg(feature = "live-execution")]
mod trading;
use qoin_vector_engine::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};

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
    last_state_update_ts_ms: Option<i64>,
    last_event_id: Option<String>,
    last_price: Option<f64>,
    spread_bps: Option<f64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    book_imbalance: f64,
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
    ingest_lock: Arc<Mutex<()>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
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
fn apply(state: &mut SymbolState, event: &MarketEvent, half_life_ms: f64) -> Result<(), String> {
    if let Some(previous_ts) = state.last_event_ts_ms {
        if event.ts_ms < previous_ts {
            return Err("out-of-order event rejected".into());
        }
    }
    let previous_update = state.last_state_update_ts_ms.or(state.last_event_ts_ms);
    if let Some(previous_ts) = previous_update {
        let elapsed_ms = event.ts_ms.saturating_sub(previous_ts).max(0) as f64;
        let decay = 0.5_f64.powf(elapsed_ms / half_life_ms);
        state.signed_flow *= decay;
        state.total_trade_size *= decay;
        state.event_intensity *= decay;
        state.liquidation_events *= decay;
        state.ewma_abs_return *= decay;
    }
    state.symbol = event.symbol.clone();
    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            let abs_return = (price / previous).ln().abs();
            if abs_return.is_finite() {
                state.ewma_abs_return = 0.94 * state.ewma_abs_return + 0.06 * abs_return;
            }
        }
        state.last_price = Some(price);
    }
    if let (Some(bid), Some(ask)) = (event.bid, event.ask) {
        let mid = (bid + ask) / 2.0;
        state.spread_bps = if mid > 0.0 { Some((ask - bid) / mid * 10_000.0) } else { None };
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
            state.last_liquidation_side = event.liquidation_side.clone();
            match event.liquidation_side.as_deref() {
                Some("long") => { state.signed_flow -= size; state.total_trade_size += size; }
                Some("short") => { state.signed_flow += size; state.total_trade_size += size; }
                _ => {}
            }
        }
        "funding" => if event.funding_rate.is_some() { state.funding_rate = event.funding_rate; },
        "open_interest" => if event.open_interest.is_some() { state.open_interest = event.open_interest; },
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
async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    if let Err(message)=validate_event(&event){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let _ingest_guard = s.ingest_lock.lock().await;
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    match s.events.insert_one(raw,None).await{
        Ok(_)=>{},Err(e) if e.to_string().contains("E11000")=>{
            // A duplicate may be an event persisted before a previous state write failed.
            // Re-process it unless the durable in-memory checkpoint already includes it.
            let already_in_memory = s.memory.read().ok()
                .and_then(|guard| guard.get(&event.symbol).and_then(|state| state.last_event_id.clone()))
                .as_deref() == Some(event.event_id.as_str());
            if already_in_memory {
                match s.decisions.find_one(doc!{"event_id": &event.event_id, "symbol": &event.symbol}, None).await {
                    Ok(Some(_)) => return (StatusCode::OK, Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id}))),
                    Ok(None) => {},
                    Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"decision storage unavailable during duplicate recovery"}))),
                }
            }
        },
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    }
    let mut snapshot = {
        let guard = match s.memory.read() {
            Ok(v) => v,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned"}))),
        };
        guard.get(&event.symbol).cloned().unwrap_or_default()
    };
    let half_life_ms = env::var("QOIN_STATE_HALF_LIFE_MS").ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(30_000.0);
    if let Err(message) = apply(&mut snapshot, &event, half_life_ms) {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"error":message})));
    }
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
    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let decision:EdgeDecision=match strategy.decide(&snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
        snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,snapshot.liquidation_events.max(0.0) as usize,snapshot.last_liquidation_side.as_deref()){
        Ok(v)=>v,Err(e)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":e})))
    };
    let risk_assessment = risk::evaluate_entry(&risk::RiskLimits::target_policy(), &risk::RiskContext::default());
    let mut decision_doc=match mongodb::bson::to_document(&decision) { Ok(value) => value, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"decision serialization failed"}))) };
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    decision_doc.insert("event_id", event.event_id.clone());
    decision_doc.insert("risk_gate", mongodb::bson::to_bson(&risk_assessment).unwrap_or(mongodb::bson::Bson::Null));
    decision_doc.insert("execution_allowed", false);
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    match s.memory.write() {
        Ok(mut guard) => { guard.insert(event.symbol.clone(), snapshot.clone()); }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"state lock poisoned after persistence"}))),
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision,"risk_gate":risk_assessment,"execution_allowed":false})))
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
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("decision idempotency index");
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
    let mut recovered = HashMap::new();
    let mut symbols = states.distinct("symbol", None, None).await.expect("load state symbols");
    for value in symbols.drain(..) {
        if let Some(symbol) = value.as_str() {
            if let Some(mut document) = states.find_one(doc!{"symbol":symbol}, FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await.expect("load latest state") {
                document.remove("_id"); document.remove("stored_at"); document.remove("event_id");
                if let Ok(snapshot) = mongodb::bson::from_document::<SymbolState>(document) {
                    recovered.insert(symbol.to_owned(), snapshot);
                }
            }
        }
    }
    let state=AppState{events,states,decisions,strategies,memory:Arc::new(RwLock::new(recovered)),active_strategy:Arc::new(RwLock::new(active)),ingest_lock:Arc::new(Mutex::new(()))};
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

    fn event(id: &str, ts_ms: i64, event_type: &str) -> MarketEvent {
        MarketEvent {
            event_id: id.into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: event_type.into(), ts_ms, price: None, size: Some(2.0),
            side: Some("buy".into()), bid: None, ask: None, bid_size: None, ask_size: None,
            funding_rate: None, open_interest: None, liquidation_side: None,
        }
    }

    #[test]
    fn flow_and_intensity_decay_by_half_life() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("one", 1_000, "trade"), 1_000.0).unwrap();
        apply(&mut state, &event("two", 2_000, "trade"), 1_000.0).unwrap();
        assert!((state.signed_flow - 3.0).abs() < 1e-9);
        assert!((state.event_intensity - 1.5).abs() < 1e-9);
        assert!((state.total_trade_size - 3.0).abs() < 1e-9);
    }

    #[test]
    fn book_imbalance_does_not_inflate_trade_denominator() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("trade", 1_000, "trade"), 30_000.0).unwrap();
        let mut book = event("book", 2_000, "book");
        book.bid = Some(99.0); book.ask = Some(101.0);
        book.bid_size = Some(9.0); book.ask_size = Some(1.0);
        apply(&mut state, &book, 30_000.0).unwrap();
        assert!((state.total_trade_size - 2.0).abs() < 1e-9);
        assert!((state.order_flow_imbalance - 1.0).abs() < 1e-9);
        assert!((state.book_imbalance - 0.8).abs() < 1e-9);
    }

    #[test]
    fn out_of_order_event_is_rejected_without_mutation() {
        let mut state = SymbolState::default();
        apply(&mut state, &event("later", 2_000, "trade"), 30_000.0).unwrap();
        let before = state.clone();
        assert!(apply(&mut state, &event("earlier", 1_000, "trade"), 30_000.0).is_err());
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
}
