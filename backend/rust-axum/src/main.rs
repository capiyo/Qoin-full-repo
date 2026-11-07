use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query, State},
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
use std::{collections::{HashMap, HashSet}, env, sync::{Arc, RwLock}, time::Duration};
use futures_util::{StreamExt, TryStreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tower_http::trace::TraceLayer;

mod edge_strategy;
mod jepa;
mod vector_engine;
use edge_strategy::{EdgeDecision, EdgeStrategy, STRATEGY_KEY};
use jepa::{FeatureHistory, JepaClient, JepaPredictionRecord};

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
    stream: broadcast::Sender<serde_json::Value>,
    events: Collection<Document>,
    states: Collection<Document>,
    decisions: Collection<Document>,
    strategies: Collection<Document>,
    jepa_predictions: Collection<Document>,
    jepa_client: Option<JepaClient>,
    jepa_history: Arc<RwLock<HashMap<String, FeatureHistory>>>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
}
#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }
#[derive(Deserialize)]
struct StreamQuery { symbol: Option<String>, symbols: Option<String> }

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
fn apply(state: &mut SymbolState, event: &MarketEvent) {
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
    let update=serde_json::json!({"type":"market.update","symbol":snapshot.symbol,"event":event,
        "state":snapshot,"decision":decision,"server_ts_ms":chrono::Utc::now().timestamp_millis()});
    let _=s.stream.send(update);
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

#[derive(Deserialize)]
struct HistoryQuery { symbol: Option<String>, limit: Option<i64> }
async fn get_vector_history(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(120).clamp(1,500);
    let options=FindOptions::builder().sort(doc!{"stored_at":-1}).limit(limit).build();
    let mut cursor=match s.decisions.find(doc!{"symbol":&symbol},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"vector history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{
                let stored_at=document.get_datetime("stored_at").ok().map(|value|value.to_chrono().to_rfc3339());
                document.remove("_id"); document.remove("stored_at");
                let mut value= mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default();
                if let (Some(object),Some(timestamp))=(value.as_object_mut(),stored_at){object.insert("stored_at".into(),serde_json::Value::String(timestamp));}
                items.push(value);
            }
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"vector history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector history read failed","symbol":symbol}))); }
        }
    }
    items.reverse();
    (StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"items":items,"count":items.len(),"limit":limit})))
}
async fn get_instruments()->impl IntoResponse {
    let finnhub_configured=env::var("FINNHUB_API_KEY").map(|v|!v.trim().is_empty()).unwrap_or(false);
    let mut items=serde_json::json!([
      {"symbol":"BTC/USD","engine_symbol":"BTCUSDT","name":"Bitcoin","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"Bitcoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSDT","name":"Ethereum","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"Ether perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSDT","name":"Solana","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"Solana perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"BNB/USD","engine_symbol":"BNBUSDT","name":"BNB","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"BNB perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"XRP/USD","engine_symbol":"XRPUSDT","name":"XRP","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"XRP perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"DOGE/USD","engine_symbol":"DOGEUSDT","name":"Dogecoin","asset_class":"Crypto","venue":"Binance USD-M","provider":"Binance public market stream","description":"Dogecoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"AAPL","engine_symbol":"AAPL","name":"Apple","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Apple Inc. · common stock","quote_currency":"USD"},
      {"symbol":"MSFT","engine_symbol":"MSFT","name":"Microsoft","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Microsoft Corporation · common stock","quote_currency":"USD"},
      {"symbol":"NVDA","engine_symbol":"NVDA","name":"NVIDIA","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"NVIDIA Corporation · common stock","quote_currency":"USD"},
      {"symbol":"AMZN","engine_symbol":"AMZN","name":"Amazon","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Amazon.com Inc. · common stock","quote_currency":"USD"},
      {"symbol":"GOOGL","engine_symbol":"GOOGL","name":"Alphabet","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Alphabet Inc. · Class A","quote_currency":"USD"},
      {"symbol":"META","engine_symbol":"META","name":"Meta Platforms","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Meta Platforms Inc. · common stock","quote_currency":"USD"},
      {"symbol":"TSLA","engine_symbol":"TSLA","name":"Tesla","asset_class":"Equities","venue":"NASDAQ","provider":"Finnhub","description":"Tesla Inc. · common stock","quote_currency":"USD"},
      {"symbol":"EUR/USD","engine_symbol":"OANDA:EUR_USD","name":"Euro / US Dollar","asset_class":"FX","venue":"OANDA","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"GBP/USD","engine_symbol":"OANDA:GBP_USD","name":"Pound / US Dollar","asset_class":"FX","venue":"OANDA","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"USD/JPY","engine_symbol":"OANDA:USD_JPY","name":"US Dollar / Yen","asset_class":"FX","venue":"OANDA","provider":"Finnhub","description":"Major FX pair","quote_currency":"JPY"},
      {"symbol":"AUD/USD","engine_symbol":"OANDA:AUD_USD","name":"Australian Dollar / US Dollar","asset_class":"FX","venue":"OANDA","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"SPX500","engine_symbol":"OANDA:SPX500_USD","name":"S&P 500","asset_class":"Indices","venue":"OANDA","provider":"Finnhub","description":"US large-cap equity index CFD reference","quote_currency":"USD"},
      {"symbol":"NAS100","engine_symbol":"OANDA:NAS100_USD","name":"NASDAQ 100","asset_class":"Indices","venue":"OANDA","provider":"Finnhub","description":"US technology-heavy equity index reference","quote_currency":"USD"},
      {"symbol":"US30","engine_symbol":"OANDA:US30_USD","name":"Dow Jones 30","asset_class":"Indices","venue":"OANDA","provider":"Finnhub","description":"US blue-chip equity index reference","quote_currency":"USD"},
      {"symbol":"GER40","engine_symbol":"OANDA:DE30_EUR","name":"DAX 40","asset_class":"Indices","venue":"OANDA","provider":"Finnhub","description":"German equity index reference","quote_currency":"EUR"},
      {"symbol":"XAU/USD","engine_symbol":"OANDA:XAU_USD","name":"Gold","asset_class":"Commodities","venue":"OANDA","provider":"Finnhub","description":"Gold spot reference","quote_currency":"USD"},
      {"symbol":"XAG/USD","engine_symbol":"OANDA:XAG_USD","name":"Silver","asset_class":"Commodities","venue":"OANDA","provider":"Finnhub","description":"Silver spot reference","quote_currency":"USD"},
      {"symbol":"WTI","engine_symbol":"OANDA:WTICO_USD","name":"WTI Crude Oil","asset_class":"Commodities","venue":"OANDA","provider":"Finnhub","description":"US crude oil reference","quote_currency":"USD"},
      {"symbol":"BRENT","engine_symbol":"OANDA:BCO_USD","name":"Brent Crude","asset_class":"Commodities","venue":"OANDA","provider":"Finnhub","description":"Brent crude oil reference","quote_currency":"USD"},
      {"symbol":"NATGAS","engine_symbol":"OANDA:NATGAS_USD","name":"Natural Gas","asset_class":"Commodities","venue":"OANDA","provider":"Finnhub","description":"Natural gas reference","quote_currency":"USD"}
    ]);
    if let Some(rows)=items.as_array_mut() {
        for row in rows {
            if row.get("asset_class").and_then(|v|v.as_str())!=Some("Crypto") {
                row["provider_status"]=serde_json::json!(if finnhub_configured {"configured_not_streaming"} else {"requires_FINNHUB_API_KEY"});
            } else { row["provider_status"]=serde_json::json!("streaming_when_Binance_adapter_connected"); }
        }
    }
    (StatusCode::OK,Json(serde_json::json!({"source":"Qoin instrument registry","items":items,"count":items.as_array().map(|v|v.len()).unwrap_or(0)})))
}
#[derive(Deserialize)]
struct NewsQuery { category: Option<String> }
async fn get_news(Query(query):Query<NewsQuery>)->impl IntoResponse {
    let key=env::var("FINNHUB_API_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "source":"Finnhub","status":"not_configured","items":[],
            "message":"Set FINNHUB_API_KEY to load sourced live headlines. No placeholder headlines are generated."
        })));
    }
    let category=query.category.unwrap_or_else(||"general".into());
    if !["general","forex","crypto","merger"].contains(&category.as_str()) {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"category must be general, forex, crypto, or merger"})));
    }
    let response=match reqwest::Client::new().get("https://finnhub.io/api/v1/news")
        .query(&[("category",category.as_str()),("token",key.as_str())]).send().await {
        Ok(value)=>value,
        Err(error)=>{tracing::warn!(error=%error,"Finnhub news request failed");
            return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_unavailable","items":[],"message":"News provider request failed."}))); }
    };
    if !response.status().is_success() {
        let status=response.status();
        tracing::warn!(status=%status,"Finnhub news provider returned non-success");
        return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_error","items":[],"message":"News provider rejected the request."})));
    }
    let payload:serde_json::Value=match response.json().await {
        Ok(value)=>value,
        Err(_)=>return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"invalid_provider_response","items":[]})))
    };
    let items=payload.as_array().cloned().unwrap_or_default().into_iter().filter_map(|item|{
        let headline=item.get("headline")?.as_str()?.trim();
        let url=item.get("url")?.as_str()?.trim();
        if headline.is_empty()||!url.starts_with("https://"){return None;}
        Some(serde_json::json!({
            "id":item.get("id").cloned().unwrap_or(serde_json::Value::Null),
            "headline":headline,"summary":item.get("summary").and_then(|v|v.as_str()).unwrap_or(""),
            "source":item.get("source").and_then(|v|v.as_str()).unwrap_or("Finnhub"),
            "url":url,"image":item.get("image").and_then(|v|v.as_str()).unwrap_or(""),
            "published_at_ms":item.get("datetime").and_then(|v|v.as_i64()).unwrap_or(0)*1000,
            "related":item.get("related").and_then(|v|v.as_str()).unwrap_or(""),
            "category":item.get("category").and_then(|v|v.as_str()).unwrap_or(category.as_str())
        }))
    }).collect::<Vec<_>>();
    (StatusCode::OK,Json(serde_json::json!({"source":"Finnhub","status":"ok","items":items,"count":items.len()})))
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
fn subscription_set(symbol: Option<String>, symbols: Option<String>) -> HashSet<String> {
    symbols.or(symbol).unwrap_or_default().split(',').map(str::trim)
        .filter(|v| !v.is_empty()).map(|v| v.to_ascii_uppercase()).collect()
}
async fn stream_ws(State(state): State<AppState>, Query(query): Query<StreamQuery>, ws: WebSocketUpgrade) -> impl IntoResponse {
    let symbols = subscription_set(query.symbol, query.symbols);
    ws.on_upgrade(move |socket| stream_connection(socket, state, symbols))
}
async fn stream_connection(mut socket: WebSocket, state: AppState, mut symbols: HashSet<String>) {
    let hello = serde_json::json!({"type":"stream.ready","protocol_version":1,
        "subscribed_symbols":symbols,"server_ts_ms":chrono::Utc::now().timestamp_millis(),
        "note":"Read-only market observations and research vectors; no order execution."});
    if socket.send(Message::Text(hello.to_string())).await.is_err() { return; }
    let mut updates = state.stream.subscribe();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(command) = serde_json::from_str::<serde_json::Value>(&text) {
                        if command.get("type").and_then(|v|v.as_str()) == Some("subscribe") {
                            let requested = command.get("symbols").and_then(|v|v.as_array())
                                .map(|items| items.iter().filter_map(|v|v.as_str())
                                    .map(|v|v.trim().to_ascii_uppercase()).filter(|v|!v.is_empty()).collect::<HashSet<_>>())
                                .or_else(||command.get("symbol").and_then(|v|v.as_str())
                                    .map(|v|[v.trim().to_ascii_uppercase()].into_iter().collect()));
                            if let Some(next)=requested { symbols=next;
                                let ack=serde_json::json!({"type":"stream.subscribed","subscribed_symbols":symbols,
                                    "server_ts_ms":chrono::Utc::now().timestamp_millis()});
                                if socket.send(Message::Text(ack.to_string())).await.is_err(){break;}
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(payload))) => if socket.send(Message::Pong(payload)).await.is_err(){break;},
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
            update = updates.recv() => match update {
                Ok(value) => {
                    let symbol=value.get("symbol").and_then(|v|v.as_str()).unwrap_or_default().to_ascii_uppercase();
                    if !symbols.is_empty() && !symbols.contains(&symbol) {continue;}
                    if socket.send(Message::Text(value.to_string())).await.is_err(){break;}
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    let gap=serde_json::json!({"type":"stream.gap","missed_updates":missed,
                        "server_ts_ms":chrono::Utc::now().timestamp_millis(),
                        "note":"Backpressure dropped updates. Refresh REST snapshots before trusting the live view."});
                    if socket.send(Message::Text(gap.to_string())).await.is_err(){break;}
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
fn stream_number(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|v|v.as_f64().or_else(||v.as_str().and_then(|s|s.parse::<f64>().ok())))
}
async fn spawn_binance_stream(axum_addr: String, symbols: Vec<String>) {
    if symbols.is_empty(){return;}
    let streams=symbols.iter().flat_map(|symbol|{
        let symbol=symbol.to_ascii_lowercase();
        [format!("{symbol}@trade"),format!("{symbol}@bookTicker"),
         format!("{symbol}@markPrice@1s"),format!("{symbol}@forceOrder")]
    }).collect::<Vec<_>>().join("/");
    let url=format!("wss://fstream.binance.com/stream?streams={streams}");
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    let mut backoff=1_u64;
    loop {
        match connect_async(&url).await {
            Ok((mut socket,_)) => {
                tracing::info!(symbols=?symbols,"connected to Binance USD-M public market streams");
                backoff=1;
                while let Some(message)=socket.next().await {
                    let message=match message {Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"Binance stream disconnected");break;}};
                    let text=match message.to_text(){Ok(v)=>v,Err(_)=>continue};
                    let envelope:serde_json::Value=match serde_json::from_str(text){Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"invalid Binance stream JSON");continue;}};
                    let data=envelope.get("data").unwrap_or(&envelope);
                    let event_name=data.get("e").and_then(|v|v.as_str()).unwrap_or_default();
                    let symbol=data.get("s").and_then(|v|v.as_str()).unwrap_or_default();
                    if symbol.is_empty(){continue;}
                    let timestamp=data.get("E").and_then(|v|v.as_i64()).unwrap_or_else(||chrono::Utc::now().timestamp_millis());
                    let event=if event_name=="trade" {
                        let id=data.get("t").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string());
                        Some(MarketEvent{event_id:format!("trade:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"trade".into(),ts_ms:timestamp,price:stream_number(data.get("p")),size:stream_number(data.get("q")),
                            side:Some(if data.get("m").and_then(|v|v.as_bool()).unwrap_or(false){"sell"}else{"buy"}.into()),
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,open_interest:None,liquidation_side:None})
                    } else if data.get("b").is_some()&&data.get("a").is_some()&&data.get("u").is_some() {
                        let id=data.get("u").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string());
                        Some(MarketEvent{event_id:format!("book:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"book".into(),ts_ms:timestamp,price:None,size:None,side:None,
                            bid:stream_number(data.get("b")),ask:stream_number(data.get("a")),
                            bid_size:stream_number(data.get("B")),ask_size:stream_number(data.get("A")),
                            funding_rate:None,open_interest:None,liquidation_side:None})
                    } else if event_name=="markPriceUpdate" {
                        Some(MarketEvent{event_id:format!("funding:{timestamp}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"funding".into(),ts_ms:timestamp,price:stream_number(data.get("p")),size:None,side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:stream_number(data.get("r")),
                            open_interest:None,liquidation_side:None})
                    } else if event_name=="forceOrder" {
                        let order=data.get("o").unwrap_or(data);
                        let side=order.get("S").and_then(|v|v.as_str()).unwrap_or_default();
                        let liquidation_side=match side{"SELL"=>Some("long"),"BUY"=>Some("short"),_=>None};
                        let id=order.get("T").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string());
                        Some(MarketEvent{event_id:format!("liquidation:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"liquidation".into(),ts_ms:order.get("T").and_then(|v|v.as_i64()).unwrap_or(timestamp),
                            price:stream_number(order.get("p")),size:stream_number(order.get("q")),side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,open_interest:None,
                            liquidation_side:liquidation_side.map(str::to_owned)})
                    } else {None};
                    if let Some(event)=event {
                        if let Err(error)=client.post(&endpoint).json(&event).send().await {
                            tracing::warn!(symbol=%event.symbol,error=%error,"could not submit Binance observation to local engine");
                        }
                    }
                }
            }
            Err(error)=>tracing::warn!(error=%error,retry_seconds=backoff,"Binance stream connection failed")
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff=(backoff*2).min(30);
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
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
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
    let (stream, _) = broadcast::channel(4096);
    let state=AppState{
        stream, events,states,decisions,strategies,jepa_predictions,jepa_client,
        jepa_history:Arc::new(RwLock::new(HashMap::new())),
        memory:Arc::new(RwLock::new(HashMap::new())),
        active_strategy:Arc::new(RwLock::new(active))
    };
    let app=Router::new().route("/health",get(health)).route("/internal/stream",get(stream_ws)).route("/internal/events",post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/vector/history",get(get_vector_history)).route("/internal/instruments",get(get_instruments))
        .route("/internal/news",get(get_news))
        .route("/internal/jepa",get(get_jepa_prediction))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let binance_enabled=env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true);
    if binance_enabled {
        let symbols=env::var("QOIN_BINANCE_STREAM_SYMBOLS").unwrap_or_else(|_|"BTCUSDT,ETHUSDT,SOLUSDT".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(|v|v.to_ascii_uppercase()).collect::<Vec<_>>();
        let adapter_addr=addr.clone();
        tokio::spawn(async move{spawn_binance_stream(adapter_addr,symbols).await;});
    } else { tracing::info!("Binance public market stream disabled by configuration"); }
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
