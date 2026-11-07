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

mod backtest;
mod edge_strategy;
mod jepa;
mod vector_engine;
use edge_strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};
use backtest::{BacktestRequest, BacktestReport, run_backtest};
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
#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct SymbolState {
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
    jepa_client: Option<JepaClient>,
    jepa_history: Arc<RwLock<HashMap<String, FeatureHistory>>>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
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
        return Err("prices, sizes, and open interest must be non-negative".into());
    }
    Ok(())
}
pub(crate) fn apply(state: &mut SymbolState, event: &MarketEvent) {
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
    let snapshot={
        let mut guard=match s.memory.write(){Ok(v)=>v,Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})))};
        let state=guard.entry(event.symbol.clone()).or_default(); apply(state,&event); state.clone()
    };
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
            Ok(mut histories) => histories.entry(snapshot.symbol.clone()).or_default()
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
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision})))
}
async fn run_paper_backtest(State(s): State<AppState>, Json(request): Json<BacktestRequest>) -> impl IntoResponse {
    if let Err(message) = request.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": message})));
    }
    let strategy = match s.active_strategy.read() {
        Ok(value) => value.clone(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"strategy lock poisoned"}))),
    };
    let filter = doc! {"symbol": &request.symbol, "ts_ms": {"$gte": request.start_ts_ms, "$lte": request.end_ts_ms}};
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
    let run_id = format!("{}-{}", request.symbol, chrono::Utc::now().timestamp_millis());
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
    let summary = mongodb::bson::to_document(&serde_json::json!({
        "run_id": run_id,
        "symbol": report.symbol,
        "strategy_version": report.strategy_version,
        "start_ts_ms": report.start_ts_ms,
        "end_ts_ms": report.end_ts_ms,
        "initial_cash": report.initial_cash,
        "final_equity": report.final_equity,
        "realized_pnl": report.realized_pnl,
        "unrealized_pnl": report.unrealized_pnl,
        "net_pnl": report.net_pnl,
        "return_pct": report.return_pct,
        "max_drawdown_pct": report.max_drawdown_pct,
        "event_count": report.event_count,
        "signal_count": report.signal_count,
        "buy_signals": report.buy_signals,
        "sell_signals": report.sell_signals,
        "wait_signals": report.wait_signals,
        "executed_entries": report.executed_entries,
        "executed_exits": report.executed_exits,
        "skipped_entries": report.skipped_entries,
        "win_rate_pct": report.win_rate_pct,
        "profit_factor": report.profit_factor,
        "fees_paid": report.fees_paid,
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
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    backtest_runs.create_index(mongodb::IndexModel::builder().keys(doc!{"run_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("backtest run index");
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
    let state=AppState{
        events,states,decisions,strategies,jepa_predictions,backtest_runs,backtest_decisions,jepa_client,
        jepa_history:Arc::new(RwLock::new(HashMap::new())),
        memory:Arc::new(RwLock::new(HashMap::new())),
        active_strategy:Arc::new(RwLock::new(active))
    };
    let app=Router::new().route("/health",get(health)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/jepa",get(get_jepa_prediction))
        .route("/internal/backtest",post(run_paper_backtest))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
