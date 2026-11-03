use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
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
use tower_http::trace::TraceLayer;

mod edge_strategy;
mod vector_engine;
use edge_strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};
use qoin_vector_engine::{
    market_data::{aggregate_horizon, MarketEvent as MarketEventRecord},
    multi_horizon::{evaluate_multi_horizon, Horizon, HorizonInput},
    EngineConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct MarketEvent {
    event_id: String,
    venue: String,
    symbol: String,
    #[serde(default)] asset_class: Option<String>,
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
    #[serde(default)] basis: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Default)]
struct SymbolState {
    symbol: String,
    asset_class: String,
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
    strategies: Collection<Document>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String>, asset_class: Option<String> }
#[derive(Deserialize)]
struct MarketHorizonQuery { symbol: Option<String>, asset_class: Option<String>, min_events: Option<usize> }
#[derive(Deserialize)]
struct InstrumentQuery { asset_class: Option<String> }

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    if let Some(asset_class) = e.asset_class.as_deref() {
        if !matches!(asset_class, "forex" | "crypto" | "stock") {
            return Err("asset_class must be forex, crypto, or stock".into());
        }
    }
    if !matches!(e.event_type.as_str(), "trade" | "book" | "funding" | "open_interest" | "liquidation" | "basis") {
        return Err("event_type must be trade, book, funding, open_interest, liquidation, or basis".into());
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
fn apply(state: &mut SymbolState, event: &MarketEvent) {
    state.symbol = event.symbol.clone();
    state.asset_class = event.asset_class.clone().unwrap_or_else(|| "unclassified".to_owned());
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
async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let ok=s.events.count_documents(doc!{},None).await.is_ok();
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if ok {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if ok{"ok"}else{"degraded"},
       "mongodb":ok,"strategy_version":strategy_version,
       "live_order_submission":"not wired"})))
}
async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    if let Err(message)=validate_event(&event){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    match s.events.insert_one(raw,None).await{
        Ok(_)=>{},Err(e) if e.to_string().contains("E11000")=>return(StatusCode::OK,Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id}))),
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    }
    let snapshot={
        let mut guard=match s.memory.write(){Ok(v)=>v,Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})))};
        let memory_key = format!("{}:{}", event.asset_class.as_deref().unwrap_or("unclassified"), event.symbol);
        let state=guard.entry(memory_key).or_default(); apply(state,&event); state.clone()
    };
    let mut state_doc=mongodb::bson::to_document(&snapshot).unwrap_or_default();
    state_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.states.insert_one(state_doc,None).await{
        tracing::error!(error=%e,"state snapshot persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
    }
    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let decision:EdgeDecision=match strategy.decide(&snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
        snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,snapshot.liquidation_events,snapshot.last_liquidation_side.as_deref()){
        Ok(v)=>v,Err(e)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":e})))
    };
    let mut decision_doc=mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("asset_class", event.asset_class.clone().unwrap_or_else(|| "unclassified".to_owned()));
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision})))
}
async fn get_instruments(
    State(s): State<AppState>,
    Query(query): Query<InstrumentQuery>,
) -> impl IntoResponse {
    let Some(asset_class) = query.asset_class.as_deref() else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"asset_class is required"})));
    };
    if !matches!(asset_class, "forex" | "crypto" | "stock") {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"asset_class must be forex, crypto, or stock"})));
    }
    match s.events.distinct("symbol", doc!{"asset_class": asset_class}, None).await {
        Ok(values) => {
            let mut symbols: Vec<String> = values.into_iter().filter_map(|value| match value {
                mongodb::bson::Bson::String(symbol) => Some(symbol),
                _ => None,
            }).collect();
            symbols.sort();
            symbols.dedup();
            (StatusCode::OK, Json(serde_json::json!({
                "asset_class": asset_class,
                "count": symbols.len(),
                "symbols": symbols,
                "source": "ingested_market_events"
            })))
        }
        Err(error) => {
            tracing::error!(asset_class=%asset_class, error=%error, "instrument catalogue query failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"instrument catalogue unavailable"})))
        }
    }
}

async fn evaluate_market_horizons(
    State(s): State<AppState>,
    Query(query): Query<MarketHorizonQuery>,
) -> impl IntoResponse {
    let symbol = query.symbol.unwrap_or_else(|| "BTCUSDT".to_owned());
    if symbol.trim().is_empty() || symbol.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"symbol must contain 1 to 64 characters"})));
    }
    let asset_class = match query.asset_class.as_deref() {
        Some(value @ ("forex" | "crypto" | "stock")) => value,
        Some(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"asset_class must be forex, crypto, or stock"}))),
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"asset_class is required to isolate market observations"}))),
    };
    let min_events = query.min_events.unwrap_or(20);
    if !(5..=10_000).contains(&min_events) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"min_events must be between 5 and 10000"})));
    }

    let decision_ts_ms = chrono::Utc::now().timestamp_millis();
    let cutoff_ts_ms = decision_ts_ms - (Horizon::OneWeek.duration_seconds() as i64 * 1_000);
    let options = FindOptions::builder()
        .sort(doc!{"ts_ms": -1})
        .limit(50_000)
        .build();
    let filter = doc!{
        "symbol": &symbol,
        "asset_class": asset_class,
        "ts_ms": {"$gte": cutoff_ts_ms, "$lte": decision_ts_ms}
    };
    let mut cursor = match s.events.find(filter, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(symbol=%symbol, error=%error, "multi-horizon event query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"market event storage unavailable"})));
        }
    };
    let documents: Vec<Document> = match cursor.try_collect().await {
        Ok(documents) => documents,
        Err(error) => {
            tracing::error!(symbol=%symbol, error=%error, "multi-horizon event cursor failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"market event retrieval failed"})));
        }
    };
    let event_limit_reached = documents.len() >= 50_000;
    let events: Vec<MarketEventRecord> = documents.into_iter()
        .filter_map(|document| mongodb::bson::from_document(document).ok())
        .collect();

    let horizons = [
        Horizon::OneMinute, Horizon::FiveMinutes, Horizon::FifteenMinutes,
        Horizon::ThirtyMinutes, Horizon::OneHour, Horizon::FourHours,
        Horizon::OneDay, Horizon::OneWeek,
    ];
    let mut observations: Vec<_> = horizons.iter()
        .map(|horizon| aggregate_horizon(&events, *horizon, decision_ts_ms, min_events))
        .collect();
    // The query is sorted newest-first and capped. If the cap is reached, the
    // oldest portion of one or more windows may be missing; suppress every
    // signal rather than presenting a potentially truncated sample as complete.
    if event_limit_reached {
        for observation in &mut observations {
            observation.ready = false;
            observation.reasons.push(
                "50,000-event query safety cap reached; signal suppressed because the historical window may be truncated".to_owned(),
            );
        }
    }
    let config = EngineConfig::default();
    let inputs: Vec<HorizonInput> = observations.iter()
        .filter(|item| item.ready)
        .map(|item| HorizonInput {
            horizon: item.horizon,
            observation: item.observation,
            config: config.clone(),
            config_version: "multi-horizon-market-data-v1".to_owned(),
            has_open_position: false,
        })
        .collect();
    let signals = if inputs.is_empty() {
        None
    } else {
        match evaluate_multi_horizon(&inputs) {
            Ok(result) => Some(result),
            Err(error) => {
                tracing::error!(symbol=%symbol, error=%error, "multi-horizon signal evaluation failed");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"signal evaluation failed"})));
            }
        }
    };

    (StatusCode::OK, Json(serde_json::json!({
        "symbol": symbol,
        "asset_class": asset_class,
        "decision_ts_ms": decision_ts_ms,
        "events_loaded": events.len(),
        "event_limit_reached": event_limit_reached,
        "mode": "research_only",
        "observations": observations,
        "signals": signals,
        "note": "Each horizon uses its own trailing event window. Missing book or trade evidence prevents that horizon from producing a signal. Missing funding, open interest, and basis are reported as neutral or unavailable; they are not fabricated."
    })))
}

async fn evaluate_horizons(Json(inputs): Json<Vec<HorizonInput>>) -> impl IntoResponse {
    match evaluate_multi_horizon(&inputs) {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default())),
        Err(error) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": error.to_string()}))),
    }
}
async fn get_state(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let asset_class=q.asset_class.unwrap_or_else(||"unclassified".into());
    let memory_key=format!("{asset_class}:{symbol}");
    if let Ok(g)=s.memory.read(){if let Some(v)=g.get(&memory_key){return(StatusCode::OK,Json(serde_json::to_value(v).unwrap_or_default()));}}
    let filter=doc!{"symbol":&symbol,"asset_class":&asset_class};
    match s.states.find_one(filter,FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"asset_class":asset_class,"event_count":0,"order_flow_imbalance":0.0,"state_quality":0.0}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state storage unavailable"})))
    }
}
async fn get_vector(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let asset_class=q.asset_class.unwrap_or_else(||"unclassified".into());
    match s.decisions.find_one(doc!{"symbol":&symbol,"asset_class":&asset_class},FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"asset_class":asset_class,"action":"wait","reason":"no decision available"}))),
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
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"asset_class":1,"symbol":1,"ts_ms":-1}).build(),None).await.expect("asset-class horizon event index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"asset_class":1,"symbol":1,"stored_at":-1}).build(),None).await.expect("asset-class state lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"asset_class":1,"symbol":1,"stored_at":-1}).build(),None).await.expect("asset-class decision lookup index");
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
    let state=AppState{
        events,states,decisions,strategies,
        memory:Arc::new(RwLock::new(HashMap::new())),
        active_strategy:Arc::new(RwLock::new(active))
    };
    let app=Router::new().route("/health",get(health)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/instruments",get(get_instruments)).route("/internal/market-horizons",get(evaluate_market_horizons)).route("/internal/multi-horizon",post(evaluate_horizons)).route("/internal/vector",get(get_vector))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
