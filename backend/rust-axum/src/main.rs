use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
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
use std::{collections::{HashMap, HashSet}, env, sync::{Arc, RwLock}, time::Duration};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::sync::{broadcast, Mutex};
use zeroize::{Zeroize, Zeroizing};
use tokio_tungstenite::connect_async;
use tower_http::trace::TraceLayer;

mod algorithm_catalogue;
mod algorithm_entitlements;
mod edge_strategy;
mod backtest;
mod set_ledger;
mod order_safety;
mod execution_lifecycle;
mod broker_contract;
mod broker_adapter_skeleton;
mod broker_catalogue;
mod firebase_identity;
mod credential_vault;
mod broker_env;
mod broker_allocation;
mod broker_allocation_policy;
mod broker_fanout;
mod paper_session;
use edge_strategy::{EdgeStrategy, STRATEGY_KEY};
use paper_session::{PaperAccountConfig, PaperSession};
use qoin_vector_engine::{decide as decide_signal, EngineConfig, ForceObservation, Signal};

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
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct SymbolState {
    symbol: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    #[serde(default)] last_event_id: Option<String>,
    #[serde(default)] last_event_venue: Option<String>,
    last_price: Option<f64>,
    #[serde(default)] mark_price: Option<f64>,
    #[serde(default)] basis_bps: Option<f64>,
    spread_bps: Option<f64>,
    #[serde(default)] last_book_ts_ms: Option<i64>,
    #[serde(default)] liquidity_imbalance: f64,
    #[serde(default)] liquidity_available: bool,
    #[serde(default)] last_trade_ts_ms: Option<i64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    event_intensity: f64,
    ewma_abs_return: f64,
    funding_rate: Option<f64>,
    open_interest: Option<f64>,
    #[serde(default)] open_interest_change: f64,
    #[serde(default)] liquidation_pressure: f64,
    liquidation_events: usize,
    last_liquidation_side: Option<String>,
    state_quality: f64,
}
#[derive(Clone)]
struct AppState {
    stream: broadcast::Sender<serde_json::Value>,
    database: mongodb::Database,
    events: Collection<Document>,
    set_ledger_records: Collection<Document>,
    states: Collection<Document>,
    decisions: Collection<Document>,
    strategies: Collection<Document>,
    orders: Collection<Document>,
    order_lifecycle_events: Collection<Document>,
    trading_controls: Collection<Document>,
    paper_sessions: Collection<Document>,
    paper_session_decisions: Collection<Document>,
    paper_session_trades: Collection<Document>,
    active_paper_sessions: Arc<Mutex<HashMap<String, PaperSession>>>,
    users: Collection<Document>,
    broker_accounts: Collection<Document>,
    broker_connections: Collection<Document>,
    broker_connection_audit: Collection<Document>,
    credential_vault: Option<credential_vault::CredentialVault>,
    broker_registry: broker_contract::BrokerRegistry,
    broker_environment: Arc<broker_env::BrokerEnvironment>,
    firebase_verifier: Option<firebase_identity::FirebaseIdentityVerifier>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    symbol_ingest_locks: Arc<Vec<tokio::sync::Mutex<()>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
    signal_config: EngineConfig,
    signal_config_version: String,
    recovery_replayed_events: u64,
    recovery_skipped_events: u64,
}
#[derive(Deserialize)]
struct SignalRequest { observation: ForceObservation, has_open_position: bool }

#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String> }
#[derive(Deserialize)]
struct StreamQuery { symbol: Option<String>, symbols: Option<String> }

fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.event_id.len() > 256 || e.venue.len() > 64 || e.symbol.len() > 96 {
        return Err("event_id, venue, or symbol exceeds its maximum length".into());
    }
    if !matches!(e.event_type.as_str(), "trade" | "book" | "funding" | "mark_price" | "open_interest" | "liquidation") {
        return Err("event_type must be trade, book, funding, mark_price, open_interest, or liquidation".into());
    }
    if e.side.as_deref().is_some_and(|side| !matches!(side, "buy" | "sell")) {
        return Err("side must be buy or sell when supplied".into());
    }
    if e.liquidation_side.as_deref().is_some_and(|side| !matches!(side, "long" | "short")) {
        return Err("liquidation_side must be long or short when supplied".into());
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
fn event_follows_state(event: &MarketEvent, previous: &SymbolState) -> bool {
    let previous_key=(
        previous.last_event_ts_ms.unwrap_or(0),
        previous.last_event_id.as_deref().unwrap_or(""),
        previous.last_event_venue.as_deref().unwrap_or(""),
    );
    let incoming_key=(event.ts_ms,event.event_id.as_str(),event.venue.as_str());
    incoming_key > previous_key
}

fn apply(state: &mut SymbolState, event: &MarketEvent) {
    // Exponential half-life keeps flow and liquidation pressure local in time.
    if let Some(previous_ts) = state.last_event_ts_ms {
        let elapsed_ms = event.ts_ms.saturating_sub(previous_ts) as f64;
        let decay = (-std::f64::consts::LN_2 * elapsed_ms / 60_000.0).exp();
        state.signed_flow *= decay;
        state.total_trade_size *= decay;
        state.liquidation_pressure *= decay;
        let oi_decay = (-std::f64::consts::LN_2 * elapsed_ms / 300_000.0).exp();
        state.open_interest_change *= oi_decay;
    }
    state.symbol = event.symbol.clone();
    if event.event_type == "trade" {
        state.last_trade_ts_ms = Some(event.ts_ms);
    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            state.ewma_abs_return = 0.94 * state.ewma_abs_return + 0.06 * (price / previous).ln().abs();
        }
        state.last_price = Some(price);
        state.basis_bps = state.mark_price.filter(|mark| *mark > 0.0)
            .map(|mark| (mark - price) / price * 10_000.0);
    }
    }
    if event.event_type == "mark_price" {
        if let Some(mark) = event.price.filter(|value| *value > 0.0) {
            state.mark_price = Some(mark);
            state.basis_bps = state.last_price.filter(|last| *last > 0.0)
                .map(|last| (mark - last) / last * 10_000.0);
        }
    }
    if let (Some(bid),Some(ask))=(event.bid,event.ask) {
        let mid=(bid+ask)/2.0;
        state.spread_bps=if mid>0.0 {Some((ask-bid)/mid*10_000.0)} else {None};
        state.last_book_ts_ms=Some(event.ts_ms);
        state.liquidity_imbalance = match (event.bid_size,event.ask_size) {
            (Some(bs),Some(as_)) if bs+as_>0.0 => { state.liquidity_available=true; (bs-as_)/(bs+as_) },
            _ => { state.liquidity_available=false; 0.0 },
        };
    }
    match event.event_type.as_str() {
        "trade" => { let size=event.size.unwrap_or(0.0); match event.side.as_deref() {
            Some("buy")=>state.signed_flow+=size, Some("sell")=>state.signed_flow-=size, _=>{} }
            state.total_trade_size+=size; state.event_intensity+=1.0; }
        "liquidation" => { state.liquidation_events+=1; state.event_intensity+=2.0;
            state.last_liquidation_side=event.liquidation_side.clone();
            match event.liquidation_side.as_deref() { Some("long")=>state.liquidation_pressure-=1.0,
                Some("short")=>state.liquidation_pressure+=1.0, _=>{} } }
        "funding" => if event.funding_rate.is_some(){state.funding_rate=event.funding_rate;},
        "mark_price" => if event.funding_rate.is_some() { state.funding_rate = event.funding_rate; },
        "open_interest" => if let Some(current) = event.open_interest {
            if let Some(previous) = state.open_interest.filter(|value| *value > 0.0) {
                state.open_interest_change = ((current - previous) / previous).clamp(-3.0, 3.0);
            }
            state.open_interest = Some(current);
        },
        "book" => {}, _ => {}
    }
    state.event_count+=1;
    state.last_event_ts_ms=Some(event.ts_ms);
    state.last_event_id=Some(event.event_id.clone());
    state.last_event_venue=Some(event.venue.clone());
    if state.total_trade_size>0.0 && state.last_trade_ts_ms.is_some_and(|last_trade| event.ts_ms.saturating_sub(last_trade)<=60_000) {
        state.order_flow_imbalance=(state.signed_flow/state.total_trade_size).clamp(-1.0,1.0);
    } else { state.order_flow_imbalance=0.0; }
    state.state_quality=((state.event_count as f64/100.0).sqrt()).min(1.0);
}
fn constant_time_token_match(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() { return false; }
    expected.iter().zip(supplied).fold(0u8, |difference, (a, b)| difference | (a ^ b)) == 0
}

fn execution_token_authorized(headers: &HeaderMap) -> bool {
    let expected = match env::var("QOIN_EXECUTION_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => return false,
    };
    let supplied = headers.get("x-qoin-execution-token").and_then(|value| value.to_str().ok()).unwrap_or("");
    constant_time_token_match(expected.as_bytes(), supplied.as_bytes())
}

fn ledger_maintenance_token_authorized(headers: &HeaderMap) -> bool {
    let expected = match env::var("QOIN_LEDGER_MAINTENANCE_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => return false,
    };
    let supplied = headers.get("x-qoin-ledger-maintenance-token")
        .and_then(|value| value.to_str().ok()).unwrap_or("");
    constant_time_token_match(expected.as_bytes(), supplied.as_bytes())
}

#[derive(Deserialize)]
struct TradingControlRequest { enabled: bool }

async fn set_trading_enabled(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TradingControlRequest>,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    let update = doc!{"$set":{"enabled":request.enabled,"updated_at":mongodb::bson::DateTime::now()}};
    match s.trading_controls.update_one(doc!{"_id":"execution"}, update,
        UpdateOptions::builder().upsert(true).build()).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({
            "execution_enabled":request.enabled,
            "message":if request.enabled{"operator enabled order submission"}else{"kill switch engaged; new orders blocked"}
        }))),
        Err(error) => {
            tracing::error!(error=%error, "could not update trading kill switch");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"trading control store unavailable"})))
        }
    }
}

#[derive(Deserialize)]
struct BrokerReadinessQuery { brokers: Option<String> }

async fn get_broker_readiness(
    State(s): State<AppState>,
    Query(query): Query<BrokerReadinessQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let catalogue = broker_catalogue::catalogue();
    let items = catalogue["items"].as_array().cloned().unwrap_or_default();
    let requested: Vec<String> = query.brokers.unwrap_or_default().split(',')
        .map(|id| id.trim().to_ascii_lowercase())
        .filter(|id| !id.is_empty()).collect();
    let mut seen = HashSet::new();
    let selected: Vec<serde_json::Value> = requested.iter().filter(|id| seen.insert((*id).clone()))
        .map(|id| {
            items.iter().find(|item| item["id"].as_str() == Some(id.as_str())).cloned()
                .unwrap_or_else(|| serde_json::json!({"id":id,"known":false,"execution_ready":false,
                    "reason":"provider is not in the supported catalogue"}))
        }).collect();
    let ready_count = selected.iter().filter(|item| item["execution_ready"] == true).count();
    let requested_set: HashSet<String> = requested.iter().cloned().collect();
    let mut account_cursor = match s.broker_accounts.find(
        doc!{"owner_uid":&owner_uid,"status":"active","trading_enabled":true}, None
    ).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, "user broker account readiness lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account readiness unavailable"})));
        }
    };
    let mut connected_account_count = 0usize;
    loop {
        match account_cursor.try_next().await {
            Ok(Some(account)) => {
                let broker_id = account.get_str("broker_id").unwrap_or_default();
                if requested_set.is_empty() || requested_set.contains(broker_id) {
                    connected_account_count += 1;
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, "user broker account readiness cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account readiness unavailable"})));
            }
        }
    }
    let configured = s.broker_registry.configured_brokers().unwrap_or_default();
    let configured_set: HashSet<String> = configured.iter().cloned().collect();
    let configured_count = selected.iter().filter(|item| item["id"].as_str().is_some_and(|id| configured_set.contains(id))).count();
    let mut blockers = vec![
        "No provider-specific execution adapter is registered in this runtime.".to_string(),
        "Environment credentials do not prove broker authentication, account permissions, or subaccount ownership.".to_string(),
        "Live dispatch remains disabled until provider conformance, account mapping, and per-user risk controls pass.".to_string(),
    ];
    if s.firebase_verifier.is_none() {
        blockers.push("QOIN_FIREBASE_PROJECT_ID is not configured; server-side Firebase identity verification is unavailable.".into());
    }
    if configured.is_empty() {
        blockers.push("No provider adapters are registered in this runtime.".into());
    }
    (StatusCode::OK, Json(serde_json::json!({
        "selected_count": selected.len(),
        "execution_ready_count": ready_count,
        "connected_account_count": connected_account_count,
        "configured_adapter_count": configured_count,
        "configured_environment_provider_count": s.broker_environment.configured_provider_ids().len(),
        "configured_adapters": configured,
        "can_submit": false,
        "dispatch_enabled": false,
        "items": selected,
        "blockers": blockers,
        "message": "Selection preferences are passed to the execution-readiness API, but no selected provider is currently eligible for order submission."
    })))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BrokerOrderSubmission {
    broker_id: String,
    order: broker_contract::NormalizedOrder,
    preflight: broker_contract::ExecutionPreflight,
}

async fn firebase_uid_from_headers(
    verifier: Option<&firebase_identity::FirebaseIdentityVerifier>,
    headers: &HeaderMap,
) -> Result<String, StatusCode> {
    let verifier = verifier.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let authorization = headers.get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let token = authorization.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;
    verifier.verify_id_token(token).await.map_err(|error| {
        tracing::warn!(reason=%error, "Firebase identity verification rejected an order request");
        StatusCode::UNAUTHORIZED
    })
}


async fn ensure_user_record(users: &Collection<Document>, owner_uid: &str) -> Result<(), StatusCode> {
    let options = UpdateOptions::builder().upsert(true).build();
    users.update_one(
        doc!{"_id": owner_uid},
        doc!{"$setOnInsert": {"status":"active","created_at":mongodb::bson::DateTime::now(),"updated_at":mongodb::bson::DateTime::now()}},
        Some(options)
    ).await.map_err(|error| {
        tracing::error!(error=%error, "Qoin user record initialization failed");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let user = users.find_one(doc!{"_id": owner_uid}, None).await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if user.get_str("status").unwrap_or("active") != "active" { return Err(StatusCode::FORBIDDEN); }
    Ok(())
}

async fn get_qoin_me(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid, Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let user = match s.users.find_one(doc!{"_id":&owner_uid}, None).await {
        Ok(Some(user)) => user,
        _ => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"user record unavailable"}))),
    };
    (StatusCode::OK, Json(serde_json::json!({
        "uid":owner_uid,"status":user.get_str("status").unwrap_or("active"),
        "created_at":user.get_datetime("created_at").ok().map(|value|value.to_string())
    })))
}

async fn get_broker_configuration(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    (StatusCode::OK, Json(s.broker_environment.public_status()))
}

#[derive(Debug, Deserialize)]
struct ProvisionBrokerAccountRequest {
    owner_uid: String,
    broker_id: String,
    account_id: String,
    account_name: String,
    account_type: String,
    base_currency: String,
    #[serde(default)]
    trading_enabled: bool,
}

async fn provision_broker_account(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProvisionBrokerAccountRequest>,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"operator authorization required"})));
    }
    let owner_uid = request.owner_uid.trim();
    let broker_id = request.broker_id.trim().to_ascii_lowercase();
    let account_id = request.account_id.trim();
    let account_name = request.account_name.trim();
    let account_type = request.account_type.trim().to_ascii_lowercase();
    let base_currency = request.base_currency.trim().to_ascii_uppercase();
    if owner_uid.is_empty() || owner_uid.len() > 128
        || broker_id.is_empty() || broker_id.len() > 64
        || !broker_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        || account_id.is_empty() || account_id.len() > 256
        || account_name.is_empty() || account_name.len() > 80
        || !matches!(account_type.as_str(), "live" | "demo")
        || base_currency.len() < 3 || base_currency.len() > 12
        || !base_currency.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid broker account provisioning fields"})));
    }
    if !s.broker_environment.account_authorized(&broker_id, account_id) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error":"broker subaccount is not present in backend environment allowlist"})));
    }
    match s.users.find_one(doc!{"_id":owner_uid,"status":"active"}, None).await {
        Ok(Some(_)) => {},
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"target user must sign in to Qoin before account provisioning"}))),
        Err(error) => {
            tracing::error!(error=%error, "target user lookup failed during broker account provisioning");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"target user lookup unavailable"})));
        }
    }
    let now = mongodb::bson::DateTime::now();
    let account = doc!{
        "owner_uid":owner_uid,"broker_id":&broker_id,"account_id":account_id,
        "account_name":account_name,"account_type":&account_type,"base_currency":&base_currency,
        "status":"active","trading_enabled":request.trading_enabled,
        "created_at":now,"updated_at":now
    };
    match s.broker_accounts.insert_one(account, None).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({
            "owner_uid":owner_uid,"broker_id":broker_id,"account_id":account_id,
            "account_name":account_name,"account_type":account_type,"base_currency":base_currency,
            "status":"active","trading_enabled":request.trading_enabled
        }))),
        Err(error) if error.to_string().contains("E11000") => {
            (StatusCode::CONFLICT, Json(serde_json::json!({"error":"broker subaccount is already assigned to a Firebase user"})))
        }
        Err(error) => {
            tracing::error!(error=%error, "broker account provisioning failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account provisioning unavailable"})))
        }
    }
}

#[derive(Deserialize)]
struct BrokerCredentialInput {
    #[serde(default)] api_key: Option<String>,
    #[serde(default)] api_secret: Option<String>,
    #[serde(default)] api_token: Option<String>,
    #[serde(default)] passphrase: Option<String>,
}

impl BrokerCredentialInput {
    fn validate(&self) -> Result<(), String> {
        let key = self.api_key.as_deref().unwrap_or_default();
        let secret = self.api_secret.as_deref().unwrap_or_default();
        let token = self.api_token.as_deref().unwrap_or_default();
        let passphrase = self.passphrase.as_deref().unwrap_or_default();
        if key.trim().is_empty() != secret.trim().is_empty() {
            return Err("api_key and api_secret must be supplied together".into());
        }
        if key.trim().is_empty() && token.trim().is_empty() {
            return Err("provide an API key/secret pair or an API token".into());
        }
        if !key.trim().is_empty() && !token.trim().is_empty() {
            return Err("use one provider authentication method per connection".into());
        }
        if key.len() > 4096 || secret.len() > 8192 || token.len() > 8192 || passphrase.len() > 4096 {
            return Err("broker credential field exceeds its maximum length".into());
        }
        Ok(())
    }

    fn encrypted_payload(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut payload = serde_json::Map::new();
        if let Some(value) = self.api_key.as_deref() { if !value.is_empty() { payload.insert("api_key".into(), value.into()); } }
        if let Some(value) = self.api_secret.as_deref() { if !value.is_empty() { payload.insert("api_secret".into(), value.into()); } }
        if let Some(value) = self.api_token.as_deref() { if !value.is_empty() { payload.insert("api_token".into(), value.into()); } }
        if let Some(value) = self.passphrase.as_deref() { if !value.is_empty() { payload.insert("passphrase".into(), value.into()); } }
        serde_json::to_vec(&payload).map_err(|_| "credential serialization failed".into())
    }
}

impl Drop for BrokerCredentialInput {
    fn drop(&mut self) {
        if let Some(value) = self.api_key.as_mut() { value.zeroize(); }
        if let Some(value) = self.api_secret.as_mut() { value.zeroize(); }
        if let Some(value) = self.api_token.as_mut() { value.zeroize(); }
        if let Some(value) = self.passphrase.as_mut() { value.zeroize(); }
    }
}

#[derive(Deserialize)]
struct CreateBrokerConnectionRequest {
    broker_id: String,
    #[serde(default)] account_id: Option<String>,
    #[serde(default)] label: Option<String>,
    credentials: BrokerCredentialInput,
}

#[derive(Deserialize)]
struct RotateBrokerConnectionRequest {
    connection_id: String,
    credentials: BrokerCredentialInput,
}

#[derive(Deserialize)]
struct RevokeBrokerConnectionRequest {
    connection_id: String,
}

fn known_broker_id(broker_id: &str) -> bool {
    broker_catalogue::catalogue()["items"].as_array()
        .is_some_and(|items| items.iter().any(|item| item["id"].as_str() == Some(broker_id)))
}

async fn encrypt_broker_credentials(
    vault: Option<&credential_vault::CredentialVault>,
    credentials: &BrokerCredentialInput,
) -> Result<Document, (StatusCode, &'static str)> {
    let vault = vault.ok_or((StatusCode::SERVICE_UNAVAILABLE, "broker credential encryption is not configured"))?;
    let plaintext = Zeroizing::new(credentials.encrypted_payload()
        .map_err(|_| (StatusCode::BAD_REQUEST, "broker credential payload is invalid"))?);
    let envelope = vault.encrypt(plaintext.as_slice())
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "broker credential encryption failed"))?;
    mongodb::bson::to_document(&envelope)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "broker credential envelope serialization failed"))
}

async fn create_broker_connection(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateBrokerConnectionRequest>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let broker_id = request.broker_id.trim().to_ascii_lowercase();
    let account_id = request.account_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let label = request.label.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if !known_broker_id(&broker_id) || account_id.is_some_and(|value| value.len() > 256)
        || label.is_some_and(|value| value.len() > 80) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"broker ID, account ID, or label is invalid"})));
    }
    let envelope = match encrypt_broker_credentials(s.credential_vault.as_ref(), &request.credentials).await {
        Ok(value) => value,
        Err((status, message)) => return (status, Json(serde_json::json!({"error":message}))),
    };
    let connection_id = mongodb::bson::oid::ObjectId::new();
    let now = mongodb::bson::DateTime::now();
    let mut document = doc!{
        "_id":connection_id,
        "owner_uid":&owner_uid,
        "broker_id":&broker_id,
        "status":"pending_validation",
        "validation_state":"not_attempted",
        "trading_enabled":false,
        "execution_ready":false,
        "credential_envelope":envelope,
        "credential_key_id":s.credential_vault.as_ref().map(|vault|vault.active_key_id()).unwrap_or("unconfigured"),
        "created_at":now,
        "updated_at":now
    };
    if let Some(value) = account_id { document.insert("account_id", value); }
    if let Some(value) = label { document.insert("label", value); }
    if let Err(error) = s.broker_connections.insert_one(document, None).await {
        tracing::error!(error=%error, owner_uid=%owner_uid, broker_id=%broker_id, "encrypted broker connection persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker connection could not be stored"})));
    }
    if let Err(error) = s.broker_connection_audit.insert_one(doc!{
        "owner_uid":&owner_uid,"connection_id":connection_id,"broker_id":&broker_id,
        "action":"credential_created_pending_validation","at":now
    }, None).await {
        tracing::error!(error=%error, connection_id=%connection_id, "broker connection audit write failed");
        let _ = s.broker_connections.delete_one(doc!{"_id":connection_id,"owner_uid":&owner_uid}, None).await;
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker connection audit could not be persisted"})));
    }
    (StatusCode::CREATED, Json(serde_json::json!({
        "connection_id":connection_id.to_hex(),
        "broker_id":broker_id,
        "account_id":account_id,
        "label":label,
        "status":"pending_validation",
        "validation_state":"not_attempted",
        "trading_enabled":false,
        "execution_ready":false
    })))
}

async fn get_broker_connections(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let options = FindOptions::builder().sort(doc!{"updated_at":-1}).limit(100).build();
    let mut cursor = match s.broker_connections.find(doc!{"owner_uid":&owner_uid}, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, "broker connection listing failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker connections unavailable"})));
        }
    };
    let mut items = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                let connection_id = document.get_object_id("_id").ok().map(|id| id.to_hex());
                document.remove("_id");
                document.remove("credential_envelope");
                document.remove("credential_key_id");
                items.push(serde_json::json!({
                    "connection_id":connection_id,
                    "broker_id":document.get_str("broker_id").unwrap_or_default(),
                    "account_id":document.get_str("account_id").ok(),
                    "label":document.get_str("label").ok(),
                    "status":document.get_str("status").unwrap_or("unknown"),
                    "validation_state":document.get_str("validation_state").unwrap_or("unknown"),
                    "trading_enabled":document.get_bool("trading_enabled").unwrap_or(false),
                    "execution_ready":document.get_bool("execution_ready").unwrap_or(false),
                    "created_at":document.get_datetime("created_at").ok().map(|value|value.to_string()),
                    "updated_at":document.get_datetime("updated_at").ok().map(|value|value.to_string())
                }));
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, "broker connection cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker connections unavailable"})));
            }
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"items":items})))
}

async fn rotate_broker_connection(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RotateBrokerConnectionRequest>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let connection_id = match mongodb::bson::oid::ObjectId::parse_str(request.connection_id.trim()) {
        Ok(value) => value,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"connection_id is invalid"}))),
    };
    let envelope = match encrypt_broker_credentials(s.credential_vault.as_ref(), &request.credentials).await {
        Ok(value) => value,
        Err((status, message)) => return (status, Json(serde_json::json!({"error":message}))),
    };
    let now = mongodb::bson::DateTime::now();
    let result = s.broker_connections.update_one(
        doc!{"_id":connection_id,"owner_uid":&owner_uid,"status":{"$ne":"revoked"}},
        doc!{"$set":{
            "credential_envelope":envelope,
            "credential_key_id":s.credential_vault.as_ref().map(|vault|vault.active_key_id()).unwrap_or("unconfigured"),
            "status":"pending_validation",
            "validation_state":"not_attempted",
            "trading_enabled":false,
            "execution_ready":false,
            "updated_at":now
        }},
        None
    ).await;
    match result {
        Ok(result) if result.matched_count == 1 => {
            if let Err(error) = s.broker_connection_audit.insert_one(doc!{
                "owner_uid":&owner_uid,"connection_id":connection_id,"action":"credential_rotated_pending_validation","at":now
            }, None).await {
                tracing::error!(error=%error, connection_id=%connection_id, "credential rotation audit write failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"credential rotation audit could not be persisted"})));
            }
            (StatusCode::OK, Json(serde_json::json!({"connection_id":connection_id.to_hex(),"status":"pending_validation","trading_enabled":false,"execution_ready":false})))
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"broker connection not found"}))),
        Err(error) => {
            tracing::error!(error=%error, connection_id=%connection_id, "broker credential rotation failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker credential rotation unavailable"})))
        }
    }
}

async fn revoke_broker_connection(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RevokeBrokerConnectionRequest>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let connection_id = match mongodb::bson::oid::ObjectId::parse_str(request.connection_id.trim()) {
        Ok(value) => value,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"connection_id is invalid"}))),
    };
    let now = mongodb::bson::DateTime::now();
    let result = s.broker_connections.update_one(
        doc!{"_id":connection_id,"owner_uid":&owner_uid,"status":{"$ne":"revoked"}},
        doc!{"$set":{"status":"revoked","validation_state":"revoked","trading_enabled":false,"execution_ready":false,"revoked_at":now,"updated_at":now},
             "$unset":{"credential_envelope":"","credential_key_id":""}},
        None
    ).await;
    match result {
        Ok(result) if result.matched_count == 1 => {
            if let Err(error) = s.broker_connection_audit.insert_one(doc!{
                "owner_uid":&owner_uid,"connection_id":connection_id,"action":"credential_revoked","at":now
            }, None).await {
                tracing::error!(error=%error, connection_id=%connection_id, "credential revocation audit write failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"credential revocation audit could not be persisted"})));
            }
            (StatusCode::OK, Json(serde_json::json!({"connection_id":connection_id.to_hex(),"status":"revoked","trading_enabled":false,"execution_ready":false})))
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"broker connection not found"}))),
        Err(error) => {
            tracing::error!(error=%error, connection_id=%connection_id, "broker credential revocation failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker credential revocation unavailable"})))
        }
    }
}

async fn get_broker_accounts(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid, Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let options = FindOptions::builder().sort(doc!{"updated_at":-1}).limit(250).build();
    let mut cursor = match s.broker_accounts.find(doc!{"owner_uid":&owner_uid},Some(options)).await {
        Ok(cursor) => cursor,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker accounts unavailable"}))),
    };
    let mut items = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(document)) => items.push(serde_json::json!({
                "account_id":document.get_str("account_id").unwrap_or_default(),
                                "broker_id":document.get_str("broker_id").unwrap_or_default(),
                "account_name":document.get_str("account_name").unwrap_or_default(),
                "account_type":document.get_str("account_type").unwrap_or("unknown"),
                "base_currency":document.get_str("base_currency").unwrap_or_default(),
                "status":document.get_str("status").unwrap_or("unknown"),
                "trading_enabled":document.get_bool("trading_enabled").unwrap_or(false)
            })),
            Ok(None) => break,
            Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker accounts unavailable"}))),
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"items":items})))
}

async fn append_order_lifecycle_event(
    s: &AppState,
    owner_uid: &str,
    client_order_id: &str,
    state: &str,
    broker_id: &str,
    account_id: &str,
    metadata: Document,
) -> Result<(), String> {
    if owner_uid.trim().is_empty() || client_order_id.trim().is_empty() || state.trim().is_empty() {
        return Err("order lifecycle event identity is incomplete".into());
    }
    let mut document = doc!{
        "event_id":mongodb::bson::oid::ObjectId::new(),
        "owner_uid":owner_uid,
        "client_order_id":client_order_id,
        "state":state,
        "broker_id":broker_id,
        "account_id":account_id,
        "at":mongodb::bson::DateTime::now(),
        "schema_version":1_i32
    };
    for (key, value) in metadata { document.insert(key, value); }
    s.order_lifecycle_events.update_one(
        doc!{"owner_uid":owner_uid,"client_order_id":client_order_id,"state":state},
        doc!{"$setOnInsert":document},
        UpdateOptions::builder().upsert(true).build(),
    ).await.map_err(|_| "durable order lifecycle event could not be persisted".to_string())?;
    Ok(())
}

async fn submit_live_order(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(submission): Json<BrokerOrderSubmission>,
) -> impl IntoResponse {
    // A signed-in user is not an execution operator. Require the independent
    // server-held execution token in addition to Firebase identity and the
    // persisted kill-switch state before any order intent can be reserved.
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error":"operator execution authorization is required",
            "dispatch_started":false
        })));
    }
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required for order submission"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable; order rejected"})));
    }
    let enabled = match s.trading_controls.find_one(doc!{"_id":"execution"}, None).await {
        Ok(Some(control)) => control.get_bool("enabled").unwrap_or(false),
        Ok(None) => false,
        Err(error) => {
            tracing::error!(error=%error, "trading kill-switch lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"trading control unavailable; order rejected"})));
        }
    };
    if !enabled {
        return (StatusCode::LOCKED, Json(serde_json::json!({"error":"trading kill switch is engaged; order rejected"})));
    }
    let broker_id = submission.broker_id.trim().to_ascii_lowercase();
    if broker_id.is_empty() || broker_id.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"broker_id is required"})));
    }
    let _owned_account = match s.broker_accounts.find_one(
        doc!{"owner_uid":&owner_uid,"account_id":&submission.order.account_id,"broker_id":&broker_id,"status":"active","trading_enabled":true},
        None
    ).await {
        Ok(Some(account)) => account,
        Ok(None) => return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error":"selected broker account is not provisioned and active for this Firebase user",
            "dispatch_started":false
        }))),
        Err(error) => {
            tracing::error!(error=%error, "broker account ownership lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                "error":"broker account ownership could not be verified; order rejected",
                "dispatch_started":false
            })));
        }
    };
    if !s.broker_environment.account_authorized(&broker_id, &submission.order.account_id) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error":"broker credentials or the selected subaccount are not configured by Qoin",
            "dispatch_started":false
        })));
    }
    let adapter = match s.broker_registry.resolve(&broker_id) {
        Ok(adapter) => adapter,
        Err(_error) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error":"selected broker adapter is not registered; dispatch was not started","broker_id":broker_id,"dispatch_started":false
        }))),
    };
    if !adapter.broker_id().eq_ignore_ascii_case(&broker_id) {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"resolved adapter identity mismatch","dispatch_started":false})));
    }
    let request_doc = match mongodb::bson::to_document(&submission) {
        Ok(document) => document,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"normalized order serialization failed"}))),
    };
    let ledger = doc!{
        "client_order_id": &submission.order.client_order_id,
        "owner_uid": &owner_uid,
        "broker_id": &broker_id,
        "account_id": &submission.order.account_id,
        "status": "submitting",
        "request": request_doc,
        "created_at": mongodb::bson::DateTime::now(),
        "updated_at": mongodb::bson::DateTime::now(),
    };
    match s.orders.insert_one(ledger, None).await {
        Ok(_) => {},
        Err(error) if error.to_string().contains("E11000") => {
            let existing = s.orders.find_one(doc!{
                "client_order_id":&submission.order.client_order_id,
                "owner_uid":&owner_uid
            }, None).await.ok().flatten();
            return (StatusCode::CONFLICT, Json(serde_json::json!({
                "error":"client_order_id has already been reserved; reconcile only through an order owned by this account",
                "client_order_id":submission.order.client_order_id,
                "existing_status":existing.as_ref().and_then(|record|record.get_str("status").ok()).unwrap_or("not_visible"),
                "reconciliation_required":true
            })));
        },
        Err(error) => {
            tracing::error!(error=%error, "could not persist normalized broker order intent");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"durable order ledger unavailable; no order submitted"})));
        }
    }
    if let Err(error) = append_order_lifecycle_event(
        &s, &owner_uid, &submission.order.client_order_id, "pending_submission",
        &broker_id, &submission.order.account_id, Document::new(),
    ).await {
        tracing::error!(error=%error, client_order_id=%submission.order.client_order_id,
            "order lifecycle reservation event failed; dispatch suppressed");
        let _ = s.orders.update_one(
            doc!{"client_order_id":&submission.order.client_order_id,"owner_uid":&owner_uid},
            doc!{"$set":{"status":"reconciliation_required","updated_at":mongodb::bson::DateTime::now()}},
            None
        ).await;
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error":"order lifecycle ledger unavailable; no broker request was sent",
            "client_order_id":submission.order.client_order_id,
            "dispatch_started":false
        })));
    }
    let result = broker_contract::submit_risk_checked(
        adapter.as_ref(),
        &owner_uid,
        &broker_contract::PortfolioRiskPolicy::default(),
        submission.order.clone(),
        submission.preflight.clone(),
    ).await;
    match result {
        Ok(receipt) => {
            let mut lifecycle = match execution_lifecycle::OrderLifecycle::new(
                submission.order.client_order_id.clone(),
                submission.order.quantity,
            ) {
                Ok(value) => value,
                Err(_) => {
                    tracing::error!(client_order_id=%submission.order.client_order_id,
                        "broker acknowledged an order that failed local lifecycle initialization");
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                        "error":"broker acknowledgement requires reconciliation because local lifecycle validation failed",
                        "client_order_id":submission.order.client_order_id,
                        "reconciliation_required":true
                    })));
                }
            };
            if lifecycle.apply(execution_lifecycle::BrokerEvent::Accepted {
                broker_order_id: receipt.broker_receipt.broker_order_id.clone(),
            }).is_err() {
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"broker acknowledgement failed local lifecycle validation; reconcile before retrying",
                    "client_order_id":submission.order.client_order_id,
                    "reconciliation_required":true
                })));
            }
            if let Err(error) = append_order_lifecycle_event(
                &s, &owner_uid, &submission.order.client_order_id, "accepted",
                &broker_id, &submission.order.account_id,
                doc!{"broker_order_id":&receipt.broker_receipt.broker_order_id,"broker_status":&receipt.broker_receipt.status,"lifecycle_status":"accepted"},
            ).await {
                tracing::error!(error=%error, client_order_id=%submission.order.client_order_id,
                    "broker acknowledgement received but immutable lifecycle event failed");
                let _ = s.orders.update_one(
                    doc!{"client_order_id":&submission.order.client_order_id,"owner_uid":&owner_uid},
                    doc!{"$set":{"status":"reconciliation_required","updated_at":mongodb::bson::DateTime::now()}},
                    None
                ).await;
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"broker acknowledgement received but lifecycle persistence failed; reconcile before retrying",
                    "client_order_id":submission.order.client_order_id,
                    "reconciliation_required":true
                })));
            }
            let stored = mongodb::bson::to_bson(&receipt).unwrap_or(mongodb::bson::Bson::Null);
            if let Err(error) = s.orders.update_one(
                doc!{"client_order_id":&submission.order.client_order_id,"owner_uid":&owner_uid},
                doc!{"$set":{"status":"accepted","broker_receipt":stored,"updated_at":mongodb::bson::DateTime::now()}},
                None
            ).await {
                tracing::error!(error=%error, client_order_id=%submission.order.client_order_id,
                    "broker accepted order but Qoin ledger update failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"broker acknowledgement received but ledger update failed; reconcile before retrying",
                    "client_order_id":submission.order.client_order_id,"reconciliation_required":true
                })));
            }
            (StatusCode::OK, Json(serde_json::to_value(receipt).unwrap_or_default()))
        }
        Err(error) => {
            let outcome_unknown = matches!(error, broker_contract::ContractError::SubmissionOutcomeUnknown(_));
            let lifecycle_state = if outcome_unknown { "reconciliation_required" } else { "rejected" };
            let reason_code = if outcome_unknown { "broker_submission_outcome_unknown" } else { "preflight_or_capability_rejected" };
            let _ = append_order_lifecycle_event(
                &s, &owner_uid, &submission.order.client_order_id, lifecycle_state,
                &broker_id, &submission.order.account_id,
                doc!{"reason_code":reason_code},
            ).await;
            if let Err(ledger_error) = s.orders.update_one(
                doc!{"client_order_id":&submission.order.client_order_id,"owner_uid":&owner_uid},
                doc!{"$set":{"status":lifecycle_state,"execution_error":reason_code,"updated_at":mongodb::bson::DateTime::now()}},
                None
            ).await {
                tracing::error!(error=%ledger_error, client_order_id=%submission.order.client_order_id,
                    "order rejection state could not be persisted");
            }
            if outcome_unknown {
                (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                    "error":"broker submission outcome is unknown","broker_id":broker_id,
                    "client_order_id":submission.order.client_order_id,"reconciliation_required":true
                })))
            } else {
                (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({
                    "error":"order was rejected before broker dispatch","broker_id":broker_id,
                    "client_order_id":submission.order.client_order_id,"dispatch_started":false
                })))
            }
        }
    }
}

async fn reconcile_live_order(
    State(s): State<AppState>,
    Path(client_order_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required for order reconciliation"}))),
    };
    let record = match s.orders.find_one(doc!{"client_order_id":&client_order_id, "owner_uid":&owner_uid}, None).await {
        Ok(Some(record)) => record,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"order not found"}))),
        Err(error) => {
            tracing::error!(error=%error, "order ledger lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"order ledger unavailable"})));
        }
    };
    let request_doc = match record.get_document("request") {
        Ok(document) => document.clone(),
        Err(_) => return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored normalized order is malformed"}))),
    };
    let submission = match mongodb::bson::from_document::<BrokerOrderSubmission>(request_doc) {
        Ok(value) => value,
        Err(_) => return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored broker order cannot be decoded"}))),
    };
    let stored_broker_id=record.get_str("broker_id").unwrap_or_default();
    let stored_account_id=record.get_str("account_id").unwrap_or_default();
    if !stored_broker_id.eq_ignore_ascii_case(&submission.broker_id)
        || stored_account_id != submission.order.account_id
    {
        return (StatusCode::CONFLICT, Json(serde_json::json!({
            "error":"stored order ownership metadata does not match its immutable request",
            "client_order_id":client_order_id,
            "reconciliation_required":true
        })));
    }
    let _account_mapping=match s.broker_accounts.find_one(doc!{
        "owner_uid":&owner_uid,"broker_id":stored_broker_id,"account_id":stored_account_id
    },None).await {
        Ok(Some(account))=>account,
        Ok(None)=>return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error":"broker account is no longer mapped to this Firebase user",
            "client_order_id":client_order_id,
            "reconciliation_required":true
        }))),
        Err(error)=>{
            tracing::error!(error=%error,"broker account mapping lookup failed during reconciliation");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account mapping unavailable"})));
        }
    };
    if !s.broker_environment.account_authorized(stored_broker_id,stored_account_id) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error":"server-side broker credential allowlist is not configured for this account",
            "client_order_id":client_order_id,
            "reconciliation_required":true
        })));
    }
    let adapter = match s.broker_registry.resolve(&submission.broker_id) {
        Ok(adapter) => adapter,
        Err(error) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error":error.to_string(),"client_order_id":client_order_id,"reconciliation_required":true
        }))),
    };
    match broker_contract::lookup_order_by_client_order_id(
        adapter.as_ref(), &owner_uid, &submission.order.account_id, &client_order_id
    ).await {
        Ok(Some(receipt)) => {
            if let Err(error) = append_order_lifecycle_event(
                &s, &owner_uid, &client_order_id, "reconciled",
                stored_broker_id, stored_account_id,
                doc!{"broker_order_id":&receipt.broker_order_id,"broker_status":&receipt.status},
            ).await {
                tracing::error!(error=%error, client_order_id=%client_order_id,
                    "broker reconciliation succeeded but lifecycle event persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"broker reconciliation succeeded but lifecycle persistence failed",
                    "client_order_id":client_order_id,
                    "reconciliation_required":true
                })));
            }
            if let Err(error) = s.orders.update_one(
                doc!{"client_order_id":&client_order_id,"owner_uid":&owner_uid},
                doc!{"$set":{"status":"reconciled","broker_receipt":mongodb::bson::to_bson(&receipt).unwrap_or(mongodb::bson::Bson::Null),
                    "reconciled_at":mongodb::bson::DateTime::now(),"updated_at":mongodb::bson::DateTime::now()}},
                None
            ).await {
                tracing::error!(error=%error, client_order_id=%client_order_id, "could not persist broker reconciliation");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker queried but reconciliation persistence failed","client_order_id":client_order_id})));
            }
            (StatusCode::OK, Json(serde_json::to_value(receipt).unwrap_or_default()))
        }
        Ok(None) => {
            let _ = append_order_lifecycle_event(
                &s, &owner_uid, &client_order_id, "not_found_authoritative",
                stored_broker_id, stored_account_id,
                doc!{"reason_code":"complete_client_id_search_returned_no_order"},
            ).await;
            (StatusCode::CONFLICT, Json(serde_json::json!({
                "status":"not_found_authoritative","client_order_id":client_order_id,
                "message":"The adapter confirmed a complete client-order-ID search; do not automatically resubmit."
            })))
        },
        Err(_error) => {
            let _ = append_order_lifecycle_event(
                &s, &owner_uid, &client_order_id, "reconciliation_failed",
                stored_broker_id, stored_account_id,
                doc!{"reason_code":"broker_reconciliation_unavailable"},
            ).await;
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error":"broker reconciliation is unavailable","client_order_id":client_order_id,"reconciliation_required":true
            })))
        }
    }
}

async fn evaluate_signal(State(s): State<AppState>, Json(request): Json<SignalRequest>) -> Result<Json<Signal>, (StatusCode, String)> {
    let result = decide_signal(
        request.observation,
        &s.signal_config,
        &s.signal_config_version,
        request.has_open_position,
    ).map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(result))
}

async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let database_ok=s.database.run_command(doc!{"ping":1},None).await.is_ok();
    let recovery_ok=s.recovery_skipped_events == 0;
    let order_entry_enabled=s.trading_controls.find_one(doc!{"_id":"execution"},None).await
        .ok().flatten().and_then(|control|control.get_bool("enabled").ok()).unwrap_or(false);
    let healthy=database_ok && recovery_ok;
    let strategy_version=s.signal_config_version.clone();
    let registered_adapters=s.broker_registry.configured_brokers().unwrap_or_default();
    (if healthy {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if healthy{"ok"}else{"degraded"},
       "mongodb":database_ok,
       "market_recovery":{"status":if recovery_ok{"ok"}else{"degraded"},
         "replayed_events":s.recovery_replayed_events,"skipped_events":s.recovery_skipped_events},
       "strategy_version":strategy_version,
       "market_streams":{"binance_usdm_enabled":env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true),
          "coinbase_advanced_trade_enabled":env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false),
          "kraken_spot_enabled":env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false),
          "finnhub_key_configured":env::var("FINNHUB_API_KEY").map(|v|!v.trim().is_empty()).unwrap_or(false)},
       "broker_execution":{"registered_adapter_count":registered_adapters.len(),
          "registered_adapters":registered_adapters,
          "configured_environment_provider_count":s.broker_environment.configured_provider_ids().len(),
          "live_order_submission":"disabled_until_provider_adapter_conformance_and_explicit_enablement"},
       "new_order_entry_enabled":order_entry_enabled,
       "research_signal_api":"enabled", "research_signal_config_version":s.signal_config_version})))
}

/// Append the immutable V2 observation and temporal-relation records associated
/// with an event. The bounded neighbour query is supported by the raw-event
/// symbol/time index; deterministic record IDs make this safe to retry.
async fn persist_set_ledger_records(
    events: &Collection<Document>,
    ledger: &Collection<Document>,
    event: &MarketEvent,
) -> Result<usize,String> {
    // Always derive from the canonical event already stored in the immutable
    // source log. A duplicate delivery with a reused ID but altered payload must
    // never create memberships or relation edges from caller-supplied data.
    let canonical_document=events.find_one(doc!{
        "venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id
    },None).await.map_err(|error|error.to_string())?
        .ok_or_else(||"canonical source event is missing after ingestion".to_string())?;
    let mut canonical_document=canonical_document;
    canonical_document.remove("_id");
    let canonical_event=mongodb::bson::from_document::<MarketEvent>(canonical_document)
        .map_err(|error|error.to_string())?;
    validate_event(&canonical_event)?;
    let event=&canonical_event;
    let from_ts = event.ts_ms.saturating_sub(60_000);
    let to_ts = event.ts_ms.saturating_add(60_000);
    // Fetch bounded nearest candidates independently on each side of the
    // event-time pivot. A single ascending scan would bias truncation toward
    // older events and could omit nearer future observations.
    let before_options=FindOptions::builder()
        .sort(doc!{"ts_ms":-1,"event_id":-1,"venue":-1})
        .limit(5001)
        .build();
    let after_options=FindOptions::builder()
        .sort(doc!{"ts_ms":1,"event_id":1,"venue":1})
        .limit(5001)
        .build();
    let mut neighbours=Vec::new();
    for (filter,options) in [
        (doc!{"symbol":&event.symbol,"ts_ms":{"$gte":from_ts,"$lte":event.ts_ms}},before_options),
        (doc!{"symbol":&event.symbol,"ts_ms":{"$gt":event.ts_ms,"$lte":to_ts}},after_options),
    ] {
        let mut cursor=events.find(filter,Some(options)).await.map_err(|error|error.to_string())?;
        while let Some(mut document)=cursor.try_next().await.map_err(|error|error.to_string())? {
            document.remove("_id");
            if let Ok(candidate)=mongodb::bson::from_document::<MarketEvent>(document) {
                if validate_event(&candidate).is_ok() { neighbours.push(candidate); }
            }
        }
    }
    let relation_scan_truncated=neighbours.len()>5000;
    neighbours.sort_by(|left,right|left.ts_ms.abs_diff(event.ts_ms).cmp(&right.ts_ms.abs_diff(event.ts_ms))
        .then_with(||left.ts_ms.cmp(&right.ts_ms))
        .then_with(||left.event_id.cmp(&right.event_id))
        .then_with(||left.venue.cmp(&right.venue)));
    neighbours.truncate(5000);
    let records=set_ledger::durable_records(event,&neighbours,mongodb::bson::DateTime::now())?;
    let mut inserted=0usize;
    for mut record in records {
        record.insert("relation_scan_truncated",relation_scan_truncated);
        record.insert("relation_scan_candidate_count_lower_bound",if relation_scan_truncated {5001_i64} else {neighbours.len() as i64});
        match ledger.insert_one(record,None).await {
            Ok(_)=>inserted+=1,
            Err(error) if error.to_string().contains("E11000")=>{},
            Err(error)=>return Err(error.to_string()),
        }
    }
    Ok(inserted)
}

#[derive(Deserialize)]
struct PaperSessionStartRequest {
    cycle_id: String,
    accounts: Vec<PaperAccountConfig>,
}

async fn persist_paper_session(s: &AppState, owner_uid: &str, session: &PaperSession) -> Result<(), String> {
    if owner_uid.trim().is_empty() {
        return Err("paper session owner is required".into());
    }
    let mut document = mongodb::bson::to_document(session).map_err(|error| error.to_string())?;
    document.insert("session_key", owner_uid);
    document.insert("owner_uid", owner_uid);
    document.insert("updated_at", mongodb::bson::DateTime::now());
    s.paper_sessions.update_one(
        doc!{"session_key":owner_uid,"owner_uid":owner_uid},
        doc!{"$set":document},
        UpdateOptions::builder().upsert(true).build(),
    ).await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn get_paper_session(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let sessions = s.active_paper_sessions.lock().await;
    let snapshot = sessions.get(&owner_uid).map(PaperSession::snapshot)
        .unwrap_or_else(|| PaperSession::default().snapshot());
    (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()))
}

#[derive(Deserialize)]
struct PaperHistoryQuery {
    #[serde(default)] cycle_id: Option<String>,
    #[serde(default)] limit: Option<i64>,
}

async fn get_paper_trade_history(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PaperHistoryQuery>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    if query.cycle_id.as_ref().is_some_and(|id| id.trim().is_empty() || id.len() > 128) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"cycle_id is invalid"})));
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=500).contains(&limit) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"limit must be between 1 and 500"})));
    }
    let mut filter = doc!{"owner_uid":&owner_uid};
    if let Some(cycle_id) = query.cycle_id.as_deref() {
        filter.insert("cycle_id", cycle_id);
    }
    let options = FindOptions::builder().sort(doc!{"exit_ts_ms":-1}).limit(limit).build();
    let mut cursor = match s.paper_session_trades.find(filter, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, owner_uid=%owner_uid, "paper trade history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade history unavailable"})));
        }
    };
    let mut items = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                document.remove("_id");
                document.remove("owner_uid");
                document.remove("stored_at");
                items.push(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or(serde_json::Value::Null));
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, owner_uid=%owner_uid, "paper trade history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade history unavailable"})));
            }
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"items":items,"limit":limit})))
}

async fn get_paper_decision_history(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PaperHistoryQuery>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    if query.cycle_id.as_ref().is_some_and(|id| id.trim().is_empty() || id.len() > 128) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"cycle_id is invalid"})));
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=500).contains(&limit) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"limit must be between 1 and 500"})));
    }
    let mut filter = doc!{"owner_uid":&owner_uid};
    if let Some(cycle_id) = query.cycle_id.as_deref() {
        filter.insert("cycle_id", cycle_id);
    }
    let options = FindOptions::builder().sort(doc!{"ts_ms":-1}).limit(limit).build();
    let mut cursor = match s.paper_session_decisions.find(filter, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, owner_uid=%owner_uid, "paper decision history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper decision history unavailable"})));
        }
    };
    let mut items = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                document.remove("_id");
                document.remove("owner_uid");
                document.remove("stored_at");
                items.push(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or(serde_json::Value::Null));
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, owner_uid=%owner_uid, "paper decision history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper decision history unavailable"})));
            }
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"items":items,"limit":limit})))
}

async fn start_paper_session(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PaperSessionStartRequest>,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    if request.cycle_id.trim().is_empty() || request.cycle_id.len() > 128 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"cycle_id must contain 1 to 128 characters"})));
    }
    let mut sessions = s.active_paper_sessions.lock().await;
    if let Some(current) = sessions.get_mut(&owner_uid) {
        if current.is_active() {
            return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"a paper session is already active; stop it before starting or resuming"})));
        }
        if current.cycle_id() == Some(request.cycle_id.as_str()) {
            current.resume();
            if let Err(error) = persist_paper_session(&s, &owner_uid, current).await {
                tracing::error!(error=%error, owner_uid=%owner_uid, "paper session resume persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session persistence failed"})));
            }
            let snapshot = current.snapshot();
            return (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()));
        }
        if current.has_unsettled_state() {
            return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"the previous paper session has an open position or pending action; resume that cycle before creating another"})));
        }
    }
    let session = match PaperSession::start(request.cycle_id, &request.accounts) {
        Ok(session) => session,
        Err(error) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error}))),
    };
    if let Err(error) = persist_paper_session(&s, &owner_uid, &session).await {
        tracing::error!(error=%error, owner_uid=%owner_uid, "paper session start persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session persistence failed"})));
    }
    let snapshot = session.snapshot();
    sessions.insert(owner_uid, session);
    (StatusCode::OK, Json(serde_json::to_value(snapshot).unwrap_or_default()))
}

async fn stop_paper_session(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let session = {
        let mut sessions = s.active_paper_sessions.lock().await;
        let current = sessions.entry(owner_uid.clone()).or_default();
        current.stop();
        current.clone()
    };
    if let Err(error) = persist_paper_session(&s, &owner_uid, &session).await {
        tracing::error!(error=%error, owner_uid=%owner_uid, "paper session stop persistence failed");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper session persistence failed"})));
    }
    (StatusCode::OK, Json(serde_json::to_value(session.snapshot()).unwrap_or_default()))
}

async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    // Serialize the full persistence/decision pipeline per canonical symbol.
    // The memory lock alone only protects state mutation; without this guard,
    // concurrent events could persist snapshots and decisions out of order.
    // A fixed set of lock stripes bounds memory even if malformed/internal
    // callers submit an unbounded number of distinct symbol strings.
    let lock_stripe = event.symbol.bytes().fold(0usize, |hash, byte|
        hash.wrapping_mul(31).wrapping_add(byte as usize)) % s.symbol_ingest_locks.len();
    let _symbol_guard = s.symbol_ingest_locks[lock_stripe].lock().await;
    if let Err(message)=validate_event(&event){tracing::warn!(symbol=%event.symbol,event_type=%event.event_type,reason=%message,"invalid market event rejected");return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}

    // Idempotent retries are handled before event-time ordering checks.
    let duplicate_filter=doc!{"venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id};
    match s.events.find_one(duplicate_filter.clone(),None).await {
        Ok(Some(_)) => {
            if let Err(error)=persist_set_ledger_records(&s.events,&s.set_ledger_records,&event).await {
                tracing::error!(error=%error,symbol=%event.symbol,event_id=%event.event_id,"duplicate event ledger repair failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable set ledger append failed","event_persisted":true})));
            }
            return (StatusCode::OK,Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id,"ledger_repair_attempted":true})));
        }
        Ok(None) => {}
        Err(error) => {
            tracing::error!(error=%error,"duplicate event lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event lookup unavailable"})));
        }
    }
    if let Ok(memory)=s.memory.read() {
        if let Some(previous)=memory.get(&event.symbol) {
            if !event_follows_state(&event, previous) {
                return (StatusCode::CONFLICT,Json(serde_json::json!({
                    "error":"out-of-order market event rejected",
                    "last_event_ts_ms":previous.last_event_ts_ms,
                    "last_event_id":previous.last_event_id.as_deref(),
                    "incoming_event_ts_ms":event.ts_ms,
                    "incoming_event_id":event.event_id
                })));
            }
        }
    } else {
        return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})));
    }
    let raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    let newly_inserted = match s.events.insert_one(raw,None).await{
        Ok(_)=>true,
        Err(e) if e.to_string().contains("E11000")=>false,
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    };
    // Repair the derived ledger on duplicate delivery too: this closes the
    // crash window between raw-event persistence and ledger-record persistence.
    if let Err(error)=persist_set_ledger_records(&s.events,&s.set_ledger_records,&event).await {
        tracing::error!(error=%error,symbol=%event.symbol,event_id=%event.event_id,"durable V2 ledger append failed");
        return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable set ledger append failed","event_persisted":true})));
    }
    if !newly_inserted {
        return (StatusCode::OK,Json(serde_json::json!({"status":"duplicate_ignored","event_id":event.event_id,"ledger_repair_attempted":true})));
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
    // The seven-force vector engine is the single decision authority for every
    // newly ingested event. All features are derived from the causal rolling
    // state rebuilt from the immutable event log; unavailable basis is zero,
    // and a missing/stale spread forces WAIT inside the engine.
    let observation = ForceObservation {
        ofi: snapshot.order_flow_imbalance,
        liquidity: snapshot.liquidity_imbalance,
        volatility: snapshot.ewma_abs_return / 0.01,
        funding: snapshot.funding_rate.unwrap_or(0.0) * 100.0,
        open_interest_change: snapshot.open_interest_change * 100.0,
        liquidation_pressure: snapshot.liquidation_pressure / 10.0,
        basis: snapshot.basis_bps.map(|basis| basis / 25.0).unwrap_or(0.0),
        spread_bps: snapshot.spread_bps,
        spread_age_ms: snapshot.last_book_ts_ms.zip(snapshot.last_event_ts_ms)
            .map(|(book_ts, latest_ts)| latest_ts.saturating_sub(book_ts)),
    };
    let signal = match decide_signal(observation, &s.signal_config, &s.signal_config_version, false) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(error=%error, symbol=%snapshot.symbol, "authoritative vector engine rejected market state");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"vector decision failed","event_persisted":true,"state_persisted":true})));
        }
    };
    let paper_decision = signal.clone();
    let decision = serde_json::json!({
        "symbol": snapshot.symbol,
        "input_event": {
            "event_id": event.event_id,
            "venue": event.venue,
            "event_type": event.event_type,
            "event_ts_ms": event.ts_ms,
            "state_event_count": snapshot.event_count,
            "state_last_event_ts_ms": snapshot.last_event_ts_ms
        },
        "strategy_key": "qoin-vector-baseline",
        "strategy_version": signal.config_version,
        "model_version": "qoin-seven-force-vector-v1",
        "decision": signal.decision,
        "score": signal.score,
        "alignment": signal.alignment,
        "active_force_count": signal.active_force_count,
        "raw_inputs": signal.raw_inputs,
        "feature_trace": signal.feature_trace,
        "strategy_parameters": s.signal_config,
        "features": signal.features,
        "feature_names": signal.feature_names,
        "attention": signal.attention,
        "attended_features": signal.attended_features,
        "decision_vector": signal.decision_vector,
        "magnitude": signal.magnitude,
        "interaction_contribution": signal.interaction_contribution,
        "angle_radians": signal.angle_radians,
        "angle_degrees": signal.angle_degrees,
        "angular_threshold_radians": s.signal_config.max_angle_radians,
        "diagnostics": signal.diagnostics,
        "action": signal.action,
        "reason": signal.rationale,
        "research_only": true,
        "liquidation_side": snapshot.last_liquidation_side,
        "data_coverage": {
            "spread_available": snapshot.spread_bps.is_some(),
            "spread_age_ms": snapshot.last_book_ts_ms.zip(snapshot.last_event_ts_ms).map(|(book_ts, latest_ts)| latest_ts.saturating_sub(book_ts)),
            "liquidity_available": snapshot.liquidity_available,
            "order_flow_available": snapshot.last_trade_ts_ms.zip(snapshot.last_event_ts_ms)
                 .is_some_and(|(trade_ts, latest_ts)| latest_ts.saturating_sub(trade_ts) <= 60_000) && snapshot.total_trade_size > 0.0,
            "volatility_available": snapshot.last_price.is_some(),
            "funding_available": snapshot.funding_rate.is_some(),
            "open_interest_available": snapshot.open_interest.is_some(),
            "liquidation_available": snapshot.liquidation_events > 0,
            "basis_available": snapshot.basis_bps.is_some()
        }
    });
    let mut decision_doc=mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
        tracing::error!(error=%e,"decision persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":true})));
    }
    {
        // Keep the session-map lock through the durable writes. Otherwise two
        // different symbol-ingest stripes could persist snapshots out of order
        // and overwrite a newer per-user paper ledger with an older snapshot.
        let mut sessions = s.active_paper_sessions.lock().await;
        for (owner_uid, paper) in sessions.iter_mut() {
            let Some((mirror, trade)) = paper.process_event(&event, &paper_decision) else { continue; };
            let mut mirror_doc = match mongodb::bson::to_document(&mirror) {
                Ok(document) => document,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"paper decision serialization failed"}))),
            };
            mirror_doc.insert("owner_uid", owner_uid);
            mirror_doc.insert("stored_at", mongodb::bson::DateTime::now());
            if let Err(error) = s.paper_session_decisions.update_one(
                doc!{"owner_uid":owner_uid,"cycle_id":&mirror.cycle_id,"venue":&mirror.venue,"symbol":&mirror.symbol,"event_id":&mirror.event_id},
                doc!{"$setOnInsert":mirror_doc},
                UpdateOptions::builder().upsert(true).build(),
            ).await {
                tracing::error!(error=%error, owner_uid=%owner_uid, event_id=%event.event_id, "paper decision persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper decision persistence failed","event_persisted":true})));
            }
            if let Some(trade) = trade {
                let mut trade_doc = match mongodb::bson::to_document(&trade) {
                    Ok(document) => document,
                    Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"paper trade serialization failed"}))),
                };
                trade_doc.insert("owner_uid", owner_uid);
                trade_doc.insert("stored_at", mongodb::bson::DateTime::now());
                if let Err(error) = s.paper_session_trades.update_one(
                    doc!{"owner_uid":owner_uid,"cycle_id":&trade.cycle_id,"venue":&trade.venue,"symbol":&trade.symbol,"exit_ts_ms":trade.exit_ts_ms},
                    doc!{"$setOnInsert":trade_doc},
                    UpdateOptions::builder().upsert(true).build(),
                ).await {
                    tracing::error!(error=%error, owner_uid=%owner_uid, event_id=%event.event_id, "paper trade persistence failed");
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper trade persistence failed","event_persisted":true})));
                }
            }
            if let Err(error) = persist_paper_session(&s, owner_uid, paper).await {
                tracing::error!(error=%error, owner_uid=%owner_uid, event_id=%event.event_id, "paper account state persistence failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"paper account state persistence failed","event_persisted":true})));
            }
        }
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
    match s.decisions.find_one(doc!{"symbol":&symbol,"strategy_key":"qoin-vector-baseline"},FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"strategy_key":"qoin-vector-baseline","model_version":"qoin-seven-force-vector-v1","action":"wait","reason":"no seven-force engine decision available yet"}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision storage unavailable"})))
    }
}

#[derive(Deserialize)]
struct HistoryQuery { symbol: Option<String>, limit: Option<i64> }

#[derive(Deserialize)]
struct BacktestQuery {
    symbol: Option<String>,
    venue: Option<String>,
    from_ts_ms: Option<i64>,
    to_ts_ms: Option<i64>,
    limit: Option<i64>,
    initial_capital: Option<f64>,
    fee_bps: Option<f64>,
    slippage_bps: Option<f64>,
    latency_ms: Option<i64>,
    max_participation_rate: Option<f64>,
    market_impact_bps: Option<f64>,
}
async fn run_backtest(State(s): State<AppState>, Query(q): Query<BacktestQuery>) -> impl IntoResponse {
    let symbol = q.symbol.unwrap_or_else(|| "BTCUSDT".to_string());
    if symbol.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"symbol is required"})));
    }
    let from_ts_ms = q.from_ts_ms.unwrap_or(0);
    let to_ts_ms = q.to_ts_ms.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    if from_ts_ms < 0 || to_ts_ms <= 0 || from_ts_ms > to_ts_ms {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid replay time range"})));
    }
    let limit = q.limit.unwrap_or(50_000);
    if !(1..=50_000).contains(&limit) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"limit must be between 1 and 50000"})));
    }
    let mut filter = doc! {"symbol": &symbol, "ts_ms": {"$gte": from_ts_ms, "$lte": to_ts_ms}};
    if let Some(venue) = q.venue.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        filter.insert("venue", venue);
    }
    let options = FindOptions::builder()
        .sort(doc! {"ts_ms": 1, "event_id": 1})
        .limit(limit + 1)
        .build();
    let mut cursor = match s.events.find(filter, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, symbol=%symbol, "backtest event query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"historical events unavailable"})));
        }
    };
    let mut events = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                document.remove("_id");
                match mongodb::bson::from_document::<backtest::ReplayEvent>(document) {
                    Ok(event) => events.push(event),
                    Err(error) => {
                        tracing::error!(error=%error, symbol=%symbol, "stored event could not be decoded for replay");
                        return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored event schema is not replay-compatible"})));
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, symbol=%symbol, "backtest event cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"historical event read failed"})));
            }
        }
    }
    if events.len() as i64 > limit {
        return (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({
            "error":"replay window exceeds the event limit; narrow the timestamp range or select a venue",
            "limit":limit,
            "truncated":true
        })));
    }
    match backtest::replay_with_vector_engine(
        events,
        &s.signal_config,
        &s.signal_config_version,
        q.initial_capital.unwrap_or(10_000.0),
        q.fee_bps.unwrap_or(10.0),
        q.slippage_bps.unwrap_or(5.0),
        q.latency_ms.unwrap_or(0),
        q.max_participation_rate.unwrap_or(0.10),
        q.market_impact_bps.unwrap_or(5.0),
    ) {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default())),
        Err(message) => (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":message}))),
    }
}

async fn spawn_binance_open_interest(axum_addr: String, symbols: Vec<String>) {
    if symbols.is_empty() { return; }
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    loop {
        for symbol in &symbols {
            let response=client.get("https://fapi.binance.com/fapi/v1/openInterest")
                .query(&[("symbol",symbol.as_str())]).send().await;
            let response=match response {
                Ok(value) if value.status().is_success()=>value,
                Ok(value)=>{tracing::warn!(symbol=%symbol,status=%value.status(),"Binance open-interest endpoint rejected request");continue;}
                Err(error)=>{tracing::warn!(symbol=%symbol,error=%error,"Binance open-interest request failed");continue;}
            };
            let payload:serde_json::Value=match response.json().await {
                Ok(value)=>value,Err(error)=>{tracing::warn!(symbol=%symbol,error=%error,"invalid Binance open-interest JSON");continue;}
            };
            let Some(open_interest)=stream_number(payload.get("openInterest")) else {
                tracing::warn!(symbol=%symbol,payload=?payload,"Binance open-interest response missing numeric value");
                continue;
            };
            let Some(ts_ms)=payload.get("time").and_then(|v|v.as_i64()) else {
                tracing::warn!(symbol=%symbol,"Binance open-interest response missing exchange timestamp");
                continue;
            };
            let event=MarketEvent {
                event_id:format!("open_interest:{ts_ms}"),venue:"binance_usdm".into(),symbol:symbol.clone(),
                event_type:"open_interest".into(),ts_ms,price:None,size:None,side:None,
                bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,
                open_interest:Some(open_interest),liquidation_side:None,
            };
            submit_market_event(&client,&endpoint,&event,"binance_usdm_open_interest").await;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
fn parse_kraken_trade_message(payload: &serde_json::Value) -> Vec<MarketEvent> {
    if payload.get("channel").and_then(|v|v.as_str()) != Some("trade") ||
       payload.get("type").and_then(|v|v.as_str()) != Some("update") {
        return Vec::new();
    }
    let Some(rows)=payload.get("data").and_then(|v|v.as_array()) else { return Vec::new(); };
    let mut parsed=Vec::new();
    for row in rows {
        let (Some(product),Some(side),Some(time))=(
            row.get("symbol").and_then(|v|v.as_str()),
            row.get("side").and_then(|v|v.as_str()),
            row.get("timestamp").and_then(|v|v.as_str()),
        ) else { continue; };
        let Some(timestamp)=chrono::DateTime::parse_from_rfc3339(time).ok().map(|v|v.timestamp_millis()) else { continue; };
        let side=match side.to_ascii_lowercase().as_str() {"buy"=>"buy","sell"=>"sell",_=>continue};
        let (Some(price),Some(size))=(stream_number(row.get("price")),stream_number(row.get("qty"))) else {continue;};
        let Some(trade_id)=row.get("trade_id").map(|v|v.to_string()) else {continue;};
        let observation=MarketEvent{
            event_id:format!("kraken_trade:{trade_id}"),
            venue:"kraken_spot".into(),
            symbol:format!("{}_KRAKEN",product.replace('/',"").replace('-',"").to_ascii_uppercase()),
            event_type:"trade".into(),ts_ms:timestamp,price:Some(price),size:Some(size),
            side:Some(side.into()),bid:None,ask:None,bid_size:None,ask_size:None,
            funding_rate:None,open_interest:None,liquidation_side:None,
        };
        if validate_event(&observation).is_ok(){parsed.push(observation);}
    }
    parsed
}

async fn spawn_kraken_stream(axum_addr:String,symbols:Vec<String>){
    if symbols.is_empty(){return;}
    let url="wss://ws.kraken.com/v2";
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    let mut backoff=1_u64;
    loop{
        match connect_async(url).await{
            Ok((mut socket,_))=>{
                tracing::info!(symbols=?symbols,"connected to Kraken Spot public market stream");
                backoff=1;
                let subscription=serde_json::json!({"method":"subscribe","params":{"channel":"trade","symbol":symbols}});
                if let Err(error)=socket.send(tokio_tungstenite::tungstenite::Message::Text(subscription.to_string())).await{
                    tracing::warn!(error=%error,"Kraken subscription send failed");
                }else{
                    while let Some(message)=socket.next().await{
                        let message=match message{Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"Kraken stream disconnected");break;}};
                        let text=match message.to_text(){Ok(v)=>v,Err(_)=>continue};
                        let payload:serde_json::Value=match serde_json::from_str(text){
                            Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"invalid Kraken stream JSON");continue;}
                        };
                        for event in parse_kraken_trade_message(&payload){
                            submit_market_event(&client,&endpoint,&event,"kraken_spot").await;
                        }
                    }
                }
            }
            Err(error)=>tracing::warn!(error=%error,retry_seconds=backoff,"Kraken stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff=(backoff*2).min(30);
    }
}

fn parse_coinbase_trade_message(payload: &serde_json::Value) -> Vec<MarketEvent> {
    if payload.get("channel").and_then(|v|v.as_str()) != Some("market_trades") {
        return Vec::new();
    }
    let Some(events) = payload.get("events").and_then(|v|v.as_array()) else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    for event in events {
        let Some(trades) = event.get("trades").and_then(|v|v.as_array()) else { continue; };
        for trade in trades {
            let (Some(trade_id), Some(product), Some(side), Some(time)) = (
                trade.get("trade_id").and_then(|v|v.as_str()),
                trade.get("product_id").and_then(|v|v.as_str()),
                trade.get("side").and_then(|v|v.as_str()),
                trade.get("time").and_then(|v|v.as_str()),
            ) else { continue; };
            let Some(timestamp) = chrono::DateTime::parse_from_rfc3339(time).ok()
                .map(|value| value.timestamp_millis()) else { continue; };
            let symbol = format!("{}_COINBASE", product.replace('-', "").replace('/', "").to_ascii_uppercase());
            let side = match side.to_ascii_uppercase().as_str() {
                "BUY" => "buy",
                "SELL" => "sell",
                _ => continue,
            };
            let (Some(price), Some(size)) = (
                stream_number(trade.get("price")),
                stream_number(trade.get("size")),
            ) else { continue; };
            let observation = MarketEvent {
                event_id: format!("coinbase_trade:{trade_id}"),
                venue: "coinbase_advanced_trade".into(),
                symbol,
                event_type: "trade".into(),
                ts_ms: timestamp,
                price: Some(price),
                size: Some(size),
                side: Some(side.into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None, liquidation_side: None,
            };
            if validate_event(&observation).is_ok() {
                parsed.push(observation);
            }
        }
    }
    parsed
}

async fn spawn_coinbase_stream(axum_addr: String, products: Vec<String>) {
    if products.is_empty() { return; }
    let url = "wss://advanced-trade-ws.coinbase.com";
    let client = reqwest::Client::new();
    let endpoint = format!("http://{axum_addr}/internal/events");
    let mut backoff = 1_u64;
    loop {
        match connect_async(url).await {
            Ok((mut socket, _)) => {
                tracing::info!(products=?products, "connected to Coinbase Advanced Trade public market stream");
                backoff = 1;
                let subscription = serde_json::json!({
                    "type":"subscribe",
                    "channel":"market_trades",
                    "product_ids":products
                });
                if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Text(subscription.to_string())).await {
                    tracing::warn!(error=%error, "Coinbase subscription send failed");
                } else {
                    while let Some(message) = socket.next().await {
                        let message = match message {
                            Ok(value) => value,
                            Err(error) => { tracing::warn!(error=%error, "Coinbase stream disconnected"); break; }
                        };
                        if message.is_ping() {
                            if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Pong(message.into_data())).await {
                                tracing::warn!(error=%error, "Coinbase websocket pong failed");
                                break;
                            }
                            continue;
                        }
                        let text = match message.to_text() { Ok(value) => value, Err(_) => continue };
                        let payload: serde_json::Value = match serde_json::from_str(text) {
                            Ok(value) => value,
                            Err(error) => { tracing::warn!(error=%error, "invalid Coinbase market stream JSON"); continue; }
                        };
                        for event in parse_coinbase_trade_message(&payload) {
                            submit_market_event(&client, &endpoint, &event, "coinbase_advanced_trade").await;
                        }
                    }
                }
            }
            Err(error) => tracing::warn!(error=%error,retry_seconds=backoff,"Coinbase stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

async fn spawn_finnhub_stream(axum_addr: String, token: String, symbols: Vec<String>) {
    if symbols.is_empty() { return; }
    let url = format!("wss://ws.finnhub.io?token={token}");
    let client = reqwest::Client::new();
    let endpoint = format!("http://{axum_addr}/internal/events");
    let mut backoff = 1_u64;
    loop {
        match connect_async(url.as_str()).await {
            Ok((mut socket, _)) => {
                tracing::info!(symbols=?symbols, "connected to Finnhub public market trade stream");
                backoff = 1;
                for symbol in &symbols {
                    let command = serde_json::json!({"type":"subscribe","symbol":symbol});
                    if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Text(command.to_string())).await {
                        tracing::warn!(symbol=%symbol,error=%error,"Finnhub subscription send failed");
                        break;
                    }
                }
                while let Some(message) = socket.next().await {
                    let message = match message {
                        Ok(value) => value,
                        Err(error) => { tracing::warn!(error=%error,"Finnhub stream disconnected"); break; }
                    };
                    let text = match message.to_text() { Ok(value) => value, Err(_) => continue };
                    let payload: serde_json::Value = match serde_json::from_str(text) {
                        Ok(value) => value,
                        Err(error) => { tracing::warn!(error=%error,"invalid Finnhub stream JSON"); continue; }
                    };
                    let message_type=payload.get("type").and_then(|v|v.as_str()).unwrap_or_default();
                    if message_type=="error" {
                        tracing::warn!(provider_message=?payload.get("msg"),"Finnhub reported a subscription or entitlement error");
                        continue;
                    }
                    if message_type != "trade" { continue; }
                    let Some(rows) = payload.get("data").and_then(|v|v.as_array()) else { continue; };
                    for (row_index, row) in rows.iter().enumerate() {
                        let Some(symbol) = row.get("s").and_then(|v|v.as_str()) else { continue; };
                        let Some(ts_ms) = row.get("t").and_then(|v|v.as_i64()) else {
                            tracing::warn!(symbol=%symbol,"Finnhub trade missing provider timestamp; observation skipped");
                            continue;
                        };
                        let price = stream_number(row.get("p"));
                        let size = stream_number(row.get("v"));
                        let event_id = format!("trade:{}:{}:{}:{}",ts_ms,price.unwrap_or_default(),size.unwrap_or_default(),row_index);
                        let event = MarketEvent {
                            event_id, venue:"finnhub".into(), symbol:symbol.into(), event_type:"trade".into(), ts_ms,
                            price, size, side:None, bid:None, ask:None, bid_size:None, ask_size:None,
                            funding_rate:None, open_interest:None, liquidation_side:None,
                        };
                        submit_market_event(&client,&endpoint,&event,"finnhub").await;
                    }
                }
            }
            Err(error) => tracing::warn!(error=%error,retry_seconds=backoff,"Finnhub market stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}
async fn get_events(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(100).clamp(1,1000);
    let options=FindOptions::builder().sort(doc!{"ts_ms":-1}).limit(limit).build();
    let mut cursor=match s.events.find(doc!{"symbol":&symbol},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"raw event history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"raw event history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{document.remove("_id");items.push(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default());}
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"raw event history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"raw event history read failed","symbol":symbol}))); }
        }
    }
    items.reverse();
    (StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"items":items,"count":items.len(),"limit":limit})))
}
#[derive(Deserialize)]
struct LedgerCorrectionRequest {
    symbol: String,
    venue: String,
    event_id: String,
    expected_source_hash: String,
    corrected_event: MarketEvent,
    reason: String,
}

/// Append a proposed correction without mutating raw source or historical ledger
/// records. Promotion is deliberately separate: V1 and the active V2 projection
/// continue to use canonical source evidence until a reviewed migration exists.
async fn propose_set_ledger_correction(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LedgerCorrectionRequest>,
) -> impl IntoResponse {
    if !ledger_maintenance_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    if request.symbol.trim().is_empty() || request.symbol.len()>96
        || request.venue.trim().is_empty() || request.venue.len()>64
        || request.event_id.trim().is_empty() || request.event_id.len()>256
        || request.reason.trim().is_empty() || request.reason.len()>1000
        || request.expected_source_hash.len()!=64
        || !request.expected_source_hash.bytes().all(|byte|byte.is_ascii_hexdigit()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"symbol, venue, event_id, a SHA-256 expected_source_hash, and a reason of at most 1000 characters are required"})));
    }
    if let Err(error)=validate_event(&request.corrected_event) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":format!("corrected_event is invalid: {error}")})));
    }
    if request.corrected_event.symbol!=request.symbol
        || request.corrected_event.venue!=request.venue
        || request.corrected_event.event_id!=request.event_id {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"a correction cannot change the source symbol, venue, or event_id identity"})));
    }
    let source=match s.events.find_one(doc!{"symbol":&request.symbol,"venue":&request.venue,"event_id":&request.event_id},None).await {
        Ok(Some(mut document))=>{document.remove("_id");match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event)=>event,
            Err(error)=>{tracing::error!(error=%error,"canonical source event decode failed during correction");return (StatusCode::UNPROCESSABLE_ENTITY,Json(serde_json::json!({"error":"canonical source event is malformed"})));}
        }},
        Ok(None)=>return (StatusCode::NOT_FOUND,Json(serde_json::json!({"error":"canonical source event not found"}))),
        Err(error)=>{tracing::error!(error=%error,"canonical source event lookup failed for correction");return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"canonical source lookup unavailable"})));}
    };
    if let Err(error)=validate_event(&source) {
        tracing::error!(error=%error,"canonical source event failed validation during correction");
        return (StatusCode::UNPROCESSABLE_ENTITY,Json(serde_json::json!({"error":"canonical source event is invalid"})));
    }
    let current_hash=match set_ledger::source_hash(&source) {
        Ok(hash)=>hash,
        Err(error)=>{tracing::error!(error=%error,"source hash failed during correction");return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"source fingerprint unavailable"})));}
    };
    if current_hash!=request.expected_source_hash {
        return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"source changed since review; reload evidence before proposing a correction","current_source_hash":current_hash})));
    }
    let observation_id=format!("observation|{}:{}:{}",request.venue,request.symbol,request.event_id);
    let observation=match s.set_ledger_records.find_one(doc!{"_id":&observation_id},None).await {
        Ok(Some(record))=>record,
        Ok(None)=>return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"source observation has not been backfilled into the durable ledger"}))),
        Err(error)=>{tracing::error!(error=%error,"observation lookup failed during correction");return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable observation lookup unavailable"})));}
    };
    if observation.get_str("source_event_hash_sha256").ok()!=Some(current_hash.as_str()) {
        return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"durable observation does not match canonical source; run integrity audit before proposing a correction"})));
    }
    let corrected_hash=match set_ledger::source_hash(&request.corrected_event) {
        Ok(hash)=>hash,
        Err(error)=>{tracing::error!(error=%error,"corrected source hash failed");return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"corrected source fingerprint unavailable"})));}
    };
    if corrected_hash==current_hash {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"corrected_event is identical to current source evidence"})));
    }
    let correction_id=format!("correction|{}|{}",observation_id,corrected_hash);
    let corrected_document=match mongodb::bson::to_document(&request.corrected_event) {
        Ok(document)=>document,
        Err(error)=>{tracing::error!(error=%error,"corrected event serialization failed");return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"corrected event serialization failed"})));}
    };
    let record=doc!{
        "_id":&correction_id,
        "record_id":&correction_id,
        "record_type":"source_correction",
        "schema_version":"qoin-relation-ledger-v2-alpha3",
        "symbol":&request.symbol,
        "venue":&request.venue,
        "source_event_id":&request.event_id,
        "supersedes_observation_id":&observation_id,
        "expected_source_hash":&current_hash,
        "corrected_source_hash":&corrected_hash,
        "corrected_event":corrected_document,
        "reason":request.reason.trim(),
        "status":"pending_review",
        "applies_to_active_projection":false,
        "created_at":mongodb::bson::DateTime::now(),
        "immutability":"append_only_review_then_supersede"
    };
    match s.set_ledger_records.insert_one(record,None).await {
        Ok(_)=>(StatusCode::CREATED,Json(serde_json::json!({"status":"correction_proposed","correction_id":correction_id,"source_hash":current_hash,"corrected_hash":corrected_hash,"active_projection_changed":false,"review_required":true}))),
        Err(error) if error.to_string().contains("E11000")=>(StatusCode::OK,Json(serde_json::json!({"status":"correction_already_proposed","correction_id":correction_id,"active_projection_changed":false,"review_required":true}))),
        Err(error)=>{tracing::error!(error=%error,"append-only correction record persistence failed");(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"correction record could not be persisted"})))}
    }
}


#[derive(Deserialize)]
struct LedgerCorrectionQuery {
    symbol: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
}

async fn list_set_ledger_corrections(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LedgerCorrectionQuery>,
) -> impl IntoResponse {
    if !ledger_maintenance_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    if q.symbol.as_deref().is_some_and(|value| value.trim().is_empty() || value.len()>96) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"symbol is invalid"})));
    }
    let status=q.status.unwrap_or_else(||"pending_review".into());
    if !matches!(status.as_str(),"pending_review"|"all") {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"status must be pending_review or all"})));
    }
    let limit=q.limit.unwrap_or(100).clamp(1,500);
    let mut filter=doc!{"record_type":"source_correction"};
    if status!="all" {filter.insert("status",status.as_str());}
    if let Some(symbol)=q.symbol.as_deref() {filter.insert("symbol",symbol);}
    let options=FindOptions::builder().sort(doc!{"created_at":-1,"record_id":1}).limit(limit+1).build();
    let mut cursor=match s.set_ledger_records.find(filter,Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"V2 correction queue query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"correction queue unavailable"})));}
    };
    let mut records=Vec::new();
    let mut truncated=false;
    loop {
        match cursor.try_next().await {
            Ok(Some(mut record))=>{
                if records.len()>=limit as usize {truncated=true;break;}
                record.remove("_id");
                records.push(mongodb::bson::from_document::<serde_json::Value>(record).unwrap_or_default());
            }
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"V2 correction queue cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"correction queue read failed"})));}
        }
    }
    let count=records.len();
    (StatusCode::OK,Json(serde_json::json!({
        "items":records,"count":count,"limit":limit,"truncated":truncated,
        "status_filter":status,"symbol":q.symbol
    })))
}

#[derive(Deserialize)]
struct LedgerBackfillQuery {
    symbol: Option<String>,
    after_ts_ms: Option<i64>,
    after_event_id: Option<String>,
    after_venue: Option<String>,
    batch_size: Option<i64>,
}

async fn backfill_set_ledger(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LedgerBackfillQuery>,
) -> impl IntoResponse {
    if !ledger_maintenance_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    let symbol = q.symbol.unwrap_or_default();
    if symbol.trim().is_empty() || symbol.len() > 96 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"a valid symbol is required"})));
    }
    let batch_size = q.batch_size.unwrap_or(100).clamp(1, 250);
    let mut filter = doc!{"symbol":&symbol};
    if let Some(ts) = q.after_ts_ms {
        let event_id = q.after_event_id.as_deref().unwrap_or("");
        let venue = q.after_venue.as_deref().unwrap_or("");
        filter.insert("$or", mongodb::bson::to_bson(&vec![
            doc!{"ts_ms":{"$gt":ts}},
            doc!{"ts_ms":ts,"event_id":{"$gt":event_id}},
            doc!{"ts_ms":ts,"event_id":event_id,"venue":{"$gt":venue}},
        ]).unwrap_or(mongodb::bson::Bson::Null));
    } else if q.after_event_id.is_some() || q.after_venue.is_some() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"after_event_id and after_venue require after_ts_ms"})));
    }
    let options=FindOptions::builder()
        .sort(doc!{"ts_ms":1,"event_id":1,"venue":1})
        .limit(batch_size+1)
        .build();
    let mut cursor=match s.events.find(filter,Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{
            tracing::error!(error=%error,symbol=%symbol,"V2 ledger backfill source query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"backfill source query failed"})));
        }
    };
    let mut documents=Vec::new();
    while let Some(document)=match cursor.try_next().await {
        Ok(value)=>value,
        Err(error)=>{
            tracing::error!(error=%error,symbol=%symbol,"V2 ledger backfill cursor failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"backfill source read failed"})));
        }
    } {
        documents.push(document);
    }
    let has_more=documents.len()>batch_size as usize;
    documents.truncate(batch_size as usize);
    let mut last_cursor:Option<(i64,String,String)>=None;
    let mut skipped_invalid=0usize;
    let mut processed=0usize;
    let mut appended=0usize;
    for mut document in documents {
        document.remove("_id");
        let cursor_key=match (document.get_i64("ts_ms"),document.get_str("event_id"),document.get_str("venue")) {
            (Ok(ts),Ok(event_id),Ok(venue))=>(ts,event_id.to_owned(),venue.to_owned()),
            _=>return (StatusCode::UNPROCESSABLE_ENTITY,Json(serde_json::json!({
                "error":"legacy source row is missing a stable backfill cursor key",
                "last_completed_cursor":last_cursor.map(|(ts,event_id,venue)|serde_json::json!({
                    "after_ts_ms":ts,"after_event_id":event_id,"after_venue":venue
                }))
            })))
        };
        let event=match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event) if validate_event(&event).is_ok()=>event,
            _=>{
                skipped_invalid+=1;
                last_cursor=Some(cursor_key);
                continue;
            }
        };
        match persist_set_ledger_records(&s.events,&s.set_ledger_records,&event).await {
            Ok(count)=>appended+=count,
            Err(error)=>{
                tracing::error!(error=%error,symbol=%symbol,event_id=%event.event_id,"V2 ledger backfill append failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
                    "error":"backfill append failed; resume from last_completed_cursor",
                    "last_completed_cursor":last_cursor.map(|(ts,event_id,venue)|serde_json::json!({
                        "after_ts_ms":ts,"after_event_id":event_id,"after_venue":venue
                    }))
                })));
            }
        }
        processed+=1;
        last_cursor=Some(cursor_key);
    }
    let raw_count=match s.events.count_documents(doc!{"symbol":&symbol},None).await {
        Ok(count)=>count,
        Err(error)=>{
            tracing::error!(error=%error,symbol=%symbol,"V2 ledger backfill source count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"backfill source count failed"})));
        }
    };
    let observation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation"},None).await {
        Ok(count)=>count,
        Err(error)=>{
            tracing::error!(error=%error,symbol=%symbol,"V2 ledger backfill observation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"backfill ledger count failed"})));
        }
    };
    (StatusCode::OK,Json(serde_json::json!({
        "symbol":symbol,
        "batch_size":batch_size,
        "processed_valid_events":processed,
        "skipped_invalid_events":skipped_invalid,
        "new_records_inserted":appended,
        "source_event_count":raw_count,
        "durable_observation_count":observation_count,
        "remaining_observation_count_lower_bound":raw_count.saturating_sub(observation_count),
        "has_more":has_more,
        "next_cursor":last_cursor.map(|(ts,event_id,venue)|serde_json::json!({
            "after_ts_ms":ts,"after_event_id":event_id,"after_venue":venue
        })),
        "resume_note":"Repeat with next_cursor fields until has_more is false. Re-running a page is safe because ledger record IDs are deterministic."
    })))
}

#[derive(Deserialize)]
struct LedgerIntegrityQuery {
    symbol: Option<String>,
    limit: Option<i64>,
}

async fn audit_set_ledger(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LedgerIntegrityQuery>,
) -> impl IntoResponse {
    if !ledger_maintenance_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    let symbol=q.symbol.unwrap_or_default();
    if symbol.trim().is_empty() || symbol.len()>96 {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"a valid symbol is required"})));
    }
    let limit=q.limit.unwrap_or(1000).clamp(1,5000);
    let source_count=match s.events.count_documents(doc!{"symbol":&symbol},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger integrity source count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity source count failed"})));}
    };
    let observation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation"},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger integrity observation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity observation count failed"})));}
    };
    let truncated_relation_observations=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation","relation_edges_truncated":true},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation coverage count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"relation coverage count failed"})));}
    };
    let source_options=FindOptions::builder().sort(doc!{"ts_ms":1,"event_id":1,"venue":1}).limit(limit).build();
    let mut source_cursor=match s.events.find(doc!{"symbol":&symbol},Some(source_options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger integrity source query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity source query failed"})));}
    };
    let mut scanned=0usize;
    let mut invalid=0usize;
    let mut missing=0usize;
    let mut hash_mismatches=0usize;
    let mut sample=Vec::new();
    while let Some(mut document)=match source_cursor.try_next().await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger integrity source cursor failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity source read failed"})));}
    } {
        document.remove("_id");
        let event=match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event) if validate_event(&event).is_ok()=>event,
            _=>{invalid+=1;continue;}
        };
        scanned+=1;
        let source_key=format!("{}:{}:{}",event.venue,event.symbol,event.event_id);
        let record_id=format!("observation|{}",source_key);
        let record=match s.set_ledger_records.find_one(doc!{"_id":&record_id},None).await {
            Ok(value)=>value,
            Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger observation lookup failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity observation lookup failed"})));}
        };
        match record {
            None=>{
                missing+=1;
                if sample.len()<25 {sample.push(serde_json::json!({"record_id":record_id,"issue":"missing_observation"}));}
            }
            Some(record)=>{
                let expected=match set_ledger::source_hash(&event) {
                    Ok(value)=>value,
                    Err(error)=>{tracing::error!(error=%error,"source hash calculation failed");
                        return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"source hash calculation failed"})));}
                };
                if record.get_str("source_event_hash_sha256").ok()!=Some(expected.as_str()) {
                    hash_mismatches+=1;
                    if sample.len()<25 {sample.push(serde_json::json!({"record_id":record_id,"issue":"source_hash_mismatch"}));}
                }
            }
        }
    }
    let relation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"relation_edge"},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity relation count failed"})));}
    };
    let relation_options=FindOptions::builder().limit(limit).build();
    let mut relation_cursor=match s.set_ledger_records.find(doc!{"symbol":&symbol,"record_type":"relation_edge"},Some(relation_options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity relation query failed"})));}
    };
    let mut relations_scanned=0usize;
    let mut malformed_relations=0usize;
    let mut relation_endpoints=Vec::<(String,String)>::new();
    let mut endpoint_ids=HashSet::<String>::new();
    while let Some(record)=match relation_cursor.try_next().await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation cursor failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity relation read failed"})));}
    } {
        relations_scanned+=1;
        let source=record.get_str("source_event_key").ok().map(|key|format!("observation|{key}"));
        let target=record.get_str("target_event_key").ok().map(|key|format!("observation|{key}"));
        let relation_id=record.get_str("relation_id").ok();
        let expected_sets=match relation_id {
            Some("buy_trades_before_liquidations")=>Some(("buy_trades","liquidations")),
            Some("sell_trades_before_liquidations")=>Some(("sell_trades","liquidations")),
            Some("funding_before_open_interest")=>Some(("funding_events","open_interest_events")),
            _=>None,
        };
        let source_ts=record.get_i64("source_event_time_ms").ok();
        let target_ts=record.get_i64("target_event_time_ms").ok();
        let lag=record.get_i64("lag_ms").ok();
        let window=record.get_i64("window_ms").ok();
        let expected_id=match (&source,&target,relation_id) {
            (Some(source),Some(target),Some(relation_id))=>Some(format!("relation|{relation_id}|{}|{}",source.trim_start_matches("observation|"),target.trim_start_matches("observation|"))),
            _=>None,
        };
        let valid=source.is_some() && target.is_some() && source!=target && expected_sets.is_some()
            && expected_sets==Some((record.get_str("source_set").unwrap_or(""),record.get_str("target_set").unwrap_or("")))
            && source_ts.zip(target_ts).zip(lag).zip(window).is_some_and(|(((source_ts,target_ts),lag),window)|
                target_ts.saturating_sub(source_ts)==lag && lag>0 && window>0 && window<=60_000 && lag<=window)
            && expected_id.as_deref()==record.get_str("record_id").ok();
        if !valid {
            malformed_relations+=1;
            if sample.len()<25 {sample.push(serde_json::json!({"record_id":record.get_str("record_id").unwrap_or("unknown"),"issue":"malformed_relation_semantics"}));}
            continue;
        }
        let (Some(source),Some(target))=(source,target) else {continue;};
        endpoint_ids.insert(source.clone());
        endpoint_ids.insert(target.clone());
        relation_endpoints.push((source,target));
    }
    // Resolve all relation endpoints in one bounded query instead of issuing
    // two database round trips per edge during large integrity scans.
    let mut found_endpoints=HashSet::<String>::new();
    if !endpoint_ids.is_empty() {
        let ids=endpoint_ids.into_iter().collect::<Vec<_>>();
        let endpoint_options=FindOptions::builder().limit(ids.len() as i64).build();
        let mut endpoint_cursor=match s.set_ledger_records.find(doc!{"_id":{"$in":&ids},"record_type":"observation"},Some(endpoint_options)).await {
            Ok(cursor)=>cursor,
            Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation endpoint batch query failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity endpoint batch query failed"})));}
        };
        while let Some(record)=match endpoint_cursor.try_next().await {
            Ok(value)=>value,
            Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"ledger relation endpoint batch cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"integrity endpoint batch read failed"})));}
        } {
            if let Ok(id)=record.get_str("_id") {found_endpoints.insert(id.to_owned());}
        }
    }
    let mut orphan_relations=0usize;
    for (source,target) in relation_endpoints {
        if !found_endpoints.contains(&source) || !found_endpoints.contains(&target) {
            orphan_relations+=1;
            if sample.len()<25 {sample.push(serde_json::json!({"source":source,"target":target,"issue":"relation_endpoint_missing"}));}
        }
    }
    (StatusCode::OK,Json(serde_json::json!({
        "symbol":symbol,
        "source_event_count":source_count,
        "observation_record_count":observation_count,
        "source_events_scanned":scanned,
        "invalid_source_events_skipped":invalid,
        "missing_observations":missing,
        "source_hash_mismatches":hash_mismatches,
        "relation_record_count":relation_count,
        "relation_records_scanned":relations_scanned,
        "orphan_relations":orphan_relations,
        "malformed_relations":malformed_relations,
        "truncated_relation_observations":truncated_relation_observations,
        "relation_coverage_complete":truncated_relation_observations==0,
        "scan_limit":limit,
        "source_scan_complete":scanned as u64+invalid as u64==source_count,
        "relation_scan_complete":relations_scanned as u64==relation_count,
        "integrity_passed":missing==0 && hash_mismatches==0 && orphan_relations==0 && malformed_relations==0 && invalid==0 && scanned as u64+invalid as u64==source_count && relations_scanned as u64==relation_count,
        "sample_issues":sample
    })))
}

/// Rebuild the finite-set projection exclusively from durable observation records.
/// Hash checks make missing, malformed, or altered source evidence visible.
async fn reconstruct_set_ledger(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    if symbol.trim().is_empty() || symbol.len()>96 {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"a valid symbol is required"})));
    }
    let limit=q.limit.unwrap_or(500).clamp(1,1000);
    let options=FindOptions::builder().sort(doc!{"event_time_ms":1,"record_id":1}).limit(limit+1).build();
    let mut cursor=match s.set_ledger_records.find(doc!{"symbol":&symbol,"record_type":"observation"},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable observation reconstruction query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable observation ledger unavailable","symbol":symbol})));}
    };
    let mut events=Vec::<MarketEvent>::new();
    let mut invalid_observations=0usize;
    let mut hash_mismatches=0usize;
    let mut scanned=0usize;
    let mut truncated=false;
    while let Some(record)=match cursor.try_next().await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable observation reconstruction cursor failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable observation ledger read failed","symbol":symbol})));}
    } {
        scanned+=1;
        if scanned>limit as usize {truncated=true;break;}
        let source=match record.get_document("source_event") {
            Ok(document)=>document.clone(),
            Err(_)=>{invalid_observations+=1;continue;},
        };
        let event=match mongodb::bson::from_document::<MarketEvent>(source) {
            Ok(event) if validate_event(&event).is_ok()=>event,
            _=>{invalid_observations+=1;continue;},
        };
        let expected_hash=match set_ledger::source_hash(&event) {
            Ok(hash)=>hash,
            Err(error)=>{tracing::error!(error=%error,"durable source hash calculation failed");
                return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"source hash calculation failed"})));}
        };
        if record.get_str("source_event_hash_sha256").ok()!=Some(expected_hash.as_str()) {
            // Never let altered evidence silently influence a reconstructed set.
            hash_mismatches+=1;
            continue;
        }
        events.push(event);
    }
    let observation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation"},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable observation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable observation count unavailable"})));}
    };
    let source_count=match s.events.count_documents(doc!{"symbol":&symbol},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"raw source event count failed during reconstruction");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"raw source event count unavailable"})));}
    };
    let relation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"relation_edge"},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable relation count failed during reconstruction");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable relation count unavailable"})));}
    };
    let truncated_relation_observations=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation","relation_edges_truncated":true},None).await {
        Ok(count)=>count,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"relation coverage count failed during reconstruction");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"relation coverage count unavailable"})));}
    };
    let projection=set_ledger::build(&symbol,&events,invalid_observations+hash_mismatches,limit as usize,truncated);
    let mut response=serde_json::to_value(projection).unwrap_or_default();
    if let Some(object)=response.as_object_mut() {
        object.insert("reconstruction_source".into(),serde_json::json!("mongodb.set_relation_ledger.observation.source_event"));
        object.insert("durable_observation_count".into(),serde_json::json!(observation_count));
        object.insert("durable_relation_record_count".into(),serde_json::json!(relation_count));
        object.insert("truncated_relation_observations".into(),serde_json::json!(truncated_relation_observations));
        object.insert("relation_coverage_complete".into(),serde_json::json!(truncated_relation_observations==0));
        object.insert("raw_source_event_count".into(),serde_json::json!(source_count));
        object.insert("observations_scanned".into(),serde_json::json!(events.len()+invalid_observations+hash_mismatches));
        object.insert("invalid_observations".into(),serde_json::json!(invalid_observations));
        object.insert("source_hash_mismatches".into(),serde_json::json!(hash_mismatches));
        object.insert("observation_scan_complete".into(),serde_json::json!(!truncated && events.len()+invalid_observations+hash_mismatches==observation_count as usize));
        object.insert("coverage_complete".into(),serde_json::json!(!truncated && invalid_observations==0 && hash_mismatches==0 && observation_count==source_count && truncated_relation_observations==0));
        object.insert("note".into(),serde_json::json!("Sets and relation summaries were reconstructed from durable observation records. Persisted relation edges remain separately auditable; this projection does not authorize decisions or orders."));
    }
    (StatusCode::OK,Json(response))
}
async fn get_set_ledger(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    if symbol.trim().is_empty() {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"symbol is required"})));
    }
    let limit=q.limit.unwrap_or(500).clamp(1,1000);
    let options=FindOptions::builder().sort(doc!{"ts_ms":-1,"event_id":-1,"venue":-1}).limit(limit + 1).build();
    let mut cursor=match s.events.find(doc!{"symbol":&symbol},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"set ledger event query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"set ledger evidence unavailable","symbol":symbol}))); }
    };
    let mut events=Vec::new();
    let mut skipped_invalid_events=0usize;
    let mut scanned_documents=0usize;
    let mut universe_truncated=false;
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{
                scanned_documents += 1;
                if scanned_documents > limit as usize { universe_truncated = true; break; }
                document.remove("_id");
                match mongodb::bson::from_document::<MarketEvent>(document) {
                    Ok(event) if validate_event(&event).is_ok()=>events.push(event),
                    Ok(_)=>skipped_invalid_events+=1,
                    Err(error)=>{skipped_invalid_events+=1;tracing::warn!(error=%error,symbol=%symbol,"skipping malformed event in set ledger");}
                }
            }
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"set ledger event cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"set ledger evidence read failed","symbol":symbol}))); }
        }
    }
    events.reverse();
    let ledger=set_ledger::build(&symbol,&events,skipped_invalid_events,limit as usize,universe_truncated);
    let mut response=serde_json::to_value(ledger).unwrap_or_default();
    let durable_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol},None).await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable ledger count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable set ledger unavailable","symbol":symbol}))); }
    };
    let relation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"relation_edge"},None).await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable relation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable relation ledger unavailable","symbol":symbol}))); }
    };
    let source_event_count=match s.events.count_documents(doc!{"symbol":&symbol},None).await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable history source count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable history source count unavailable","symbol":symbol}))); }
    };
    let observation_count=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"record_type":"observation"},None).await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable history observation count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable history observation count unavailable","symbol":symbol}))); }
    };
    let durable_history_complete=source_event_count==observation_count;
    let truncated_relation_scans=match s.set_ledger_records.count_documents(doc!{"symbol":&symbol,"relation_scan_truncated":true},None).await {
        Ok(value)=>value,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable relation coverage count failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable relation coverage count unavailable","symbol":symbol}))); }
    };
    let record_options=FindOptions::builder().sort(doc!{"event_time_ms":-1,"record_type":1}).limit(limit as i64).build();
    let mut record_cursor=match s.set_ledger_records.find(doc!{"symbol":&symbol},Some(record_options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable ledger records query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable ledger records unavailable","symbol":symbol}))); }
    };
    let mut durable_records=Vec::new();
    loop {
        match record_cursor.try_next().await {
            Ok(Some(mut document))=>{document.remove("_id");durable_records.push(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default());}
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"durable ledger cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"durable ledger read failed","symbol":symbol}))); }
        }
    }
    if let Some(object)=response.as_object_mut() {
        object.insert("persistence".into(),serde_json::json!("append_only_mongodb_records_for_ingested_events"));
        object.insert("durable_record_count".into(),serde_json::json!(durable_count));
        object.insert("durable_relation_count".into(),serde_json::json!(relation_count));
        object.insert("durable_records".into(),serde_json::Value::Array(durable_records));
        object.insert("source_event_count".into(),serde_json::json!(source_event_count));
        object.insert("durable_observation_count".into(),serde_json::json!(observation_count));
        object.insert("truncated_relation_scans".into(),serde_json::json!(truncated_relation_scans));
        object.insert("relation_coverage_complete".into(),serde_json::json!(truncated_relation_scans==0));
        object.insert("durable_history_complete".into(),serde_json::json!(durable_history_complete));
        object.insert("durable_history_note".into(),serde_json::json!(if durable_history_complete {
            "Every raw source event has an observation record. Check relation_scan_truncated on records for bounded-neighbour coverage."
        } else {
            "Historical source events remain without observation records; run the authenticated resumable backfill and then audit integrity."
        }));
    }
    (StatusCode::OK,Json(response))
}
async fn get_vector_history(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(120).clamp(1,500);
    let options=FindOptions::builder().sort(doc!{"stored_at":-1}).limit(limit).build();
    let mut cursor=match s.decisions.find(doc!{"symbol":&symbol,"strategy_key":"qoin-vector-baseline"},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"vector history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{
                let stored_at=document.get_datetime("stored_at").ok().map(|value|chrono::DateTime::<chrono::Utc>::from_timestamp_millis(value.timestamp_millis()).map(|v|v.to_rfc3339()).unwrap_or_default());
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
    let binance_enabled=env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true);
    let mut items=serde_json::json!([
      {"symbol":"BTC/USD","engine_symbol":"BTCUSDT","name":"Bitcoin","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Bitcoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSDT","name":"Ethereum","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Ether perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"BTC/USD","engine_symbol":"BTCUSD_COINBASE","name":"Bitcoin","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Bitcoin spot · USD quote","quote_currency":"USD"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSD_COINBASE","name":"Ethereum","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Ether spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSD_COINBASE","name":"Solana","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Solana spot · USD quote","quote_currency":"USD"},
      {"symbol":"BTC/USD","engine_symbol":"BTCUSD_KRAKEN","name":"Bitcoin","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Bitcoin spot · USD quote","quote_currency":"USD"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSD_KRAKEN","name":"Ethereum","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Ether spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSD_KRAKEN","name":"Solana","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Solana spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSDT","name":"Solana","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Solana perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"BNB/USD","engine_symbol":"BNBUSDT","name":"BNB","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"BNB perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"XRP/USD","engine_symbol":"XRPUSDT","name":"XRP","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"XRP perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"DOGE/USD","engine_symbol":"DOGEUSDT","name":"Dogecoin","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Dogecoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"AAPL","engine_symbol":"AAPL","name":"Apple","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Apple Inc. · common stock","quote_currency":"USD"},
      {"symbol":"MSFT","engine_symbol":"MSFT","name":"Microsoft","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Microsoft Corporation · common stock","quote_currency":"USD"},
      {"symbol":"NVDA","engine_symbol":"NVDA","name":"NVIDIA","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"NVIDIA Corporation · common stock","quote_currency":"USD"},
      {"symbol":"AMZN","engine_symbol":"AMZN","name":"Amazon","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Amazon.com Inc. · common stock","quote_currency":"USD"},
      {"symbol":"GOOGL","engine_symbol":"GOOGL","name":"Alphabet","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Alphabet Inc. · Class A","quote_currency":"USD"},
      {"symbol":"META","engine_symbol":"META","name":"Meta Platforms","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Meta Platforms Inc. · common stock","quote_currency":"USD"},
      {"symbol":"TSLA","engine_symbol":"TSLA","name":"Tesla","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Tesla Inc. · common stock","quote_currency":"USD"},
      {"symbol":"EUR/USD","engine_symbol":"OANDA:EUR_USD","name":"Euro / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"GBP/USD","engine_symbol":"OANDA:GBP_USD","name":"Pound / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"USD/JPY","engine_symbol":"OANDA:USD_JPY","name":"US Dollar / Yen","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"JPY"},
      {"symbol":"AUD/USD","engine_symbol":"OANDA:AUD_USD","name":"Australian Dollar / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"SPX500","engine_symbol":"OANDA:SPX500_USD","name":"S&P 500","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US large-cap equity index CFD reference","quote_currency":"USD"},
      {"symbol":"NAS100","engine_symbol":"OANDA:NAS100_USD","name":"NASDAQ 100","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US technology-heavy equity index reference","quote_currency":"USD"},
      {"symbol":"US30","engine_symbol":"OANDA:US30_USD","name":"Dow Jones 30","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US blue-chip equity index reference","quote_currency":"USD"},
      {"symbol":"GER40","engine_symbol":"OANDA:DE30_EUR","name":"DAX 40","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"German equity index reference","quote_currency":"EUR"},
      {"symbol":"XAU/USD","engine_symbol":"OANDA:XAU_USD","name":"Gold","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Gold spot reference","quote_currency":"USD"},
      {"symbol":"XAG/USD","engine_symbol":"OANDA:XAG_USD","name":"Silver","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Silver spot reference","quote_currency":"USD"},
      {"symbol":"WTI","engine_symbol":"OANDA:WTICO_USD","name":"WTI Crude Oil","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US crude oil reference","quote_currency":"USD"},
      {"symbol":"BRENT","engine_symbol":"OANDA:BCO_USD","name":"Brent Crude","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Brent crude oil reference","quote_currency":"USD"},
      {"symbol":"NATGAS","engine_symbol":"OANDA:NATGAS_USD","name":"Natural Gas","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Natural gas reference","quote_currency":"USD"}
    ]);
    if let Some(rows)=items.as_array_mut() {
        for row in rows {
            let venue_id=row.get("venue_id").and_then(|v|v.as_str()).unwrap_or_default();
            if venue_id=="coinbase_advanced_trade" {
                let enabled=env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
                row["provider_status"]=serde_json::json!(if enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else if venue_id=="kraken_spot" {
                let enabled=env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
                row["provider_status"]=serde_json::json!(if enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else if venue_id=="binance_usdm" {
                row["provider_status"]=serde_json::json!(if binance_enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else {
                row["provider_status"]=serde_json::json!(if finnhub_configured {"provider_key_configured"} else {"requires_FINNHUB_API_KEY"});
            }
        }
    }
    (StatusCode::OK,Json(serde_json::json!({"source":"Qoin instrument registry","items":items,"count":items.as_array().map(|v|v.len()).unwrap_or(0)})))
}
#[derive(Deserialize)]
struct NewsQuery { category: Option<String> }
async fn get_calendar()->impl IntoResponse {
    let key=env::var("FINNHUB_API_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "source":"Finnhub","status":"not_configured","items":[],
            "message":"Set FINNHUB_API_KEY to load sourced economic-calendar events."
        })));
    }
    let from=chrono::Utc::now().date_naive();
    let to=from+chrono::Duration::days(7);
    let from_text=from.format("%Y-%m-%d").to_string();
    let to_text=to.format("%Y-%m-%d").to_string();
    let response=match reqwest::Client::new().get("https://finnhub.io/api/v1/calendar/economic")
        .query(&[("from",from_text.as_str()),("to",to_text.as_str()),("token",key.as_str())]).send().await {
        Ok(value)=>value,
        Err(error)=>{tracing::warn!(error=%error,"Finnhub economic calendar request failed");
            return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_unavailable","items":[]}))); }
    };
    if !response.status().is_success() {
        tracing::warn!(status=%response.status(),"Finnhub calendar provider returned non-success");
        return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_error","items":[]})));
    }
    let payload:serde_json::Value=match response.json().await {
        Ok(value)=>value,
        Err(_)=>return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"invalid_provider_response","items":[]})))
    };
    let items=payload.get("economicCalendar").and_then(|v|v.get("events")).and_then(|v|v.as_array())
        .cloned().unwrap_or_default().into_iter().map(|event|serde_json::json!({
            "country":event.get("country").and_then(|v|v.as_str()).unwrap_or(""),
            "event":event.get("event").and_then(|v|v.as_str()).unwrap_or(""),
            "impact":event.get("impact").and_then(|v|v.as_str()).unwrap_or(""),
            "actual":event.get("actual").cloned().unwrap_or(serde_json::Value::Null),
            "previous":event.get("prev").cloned().unwrap_or(serde_json::Value::Null),
            "estimate":event.get("estimate").cloned().unwrap_or(serde_json::Value::Null),
            "time":event.get("time").and_then(|v|v.as_str()).unwrap_or(""),
            "unit":event.get("unit").and_then(|v|v.as_str()).unwrap_or("")
        })).collect::<Vec<_>>();
    (StatusCode::OK,Json(serde_json::json!({"source":"Finnhub economic calendar","status":"ok","from":from_text,"to":to_text,"items":items,"count":items.len()})))
}
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

fn subscription_set(symbol: Option<String>, symbols: Option<String>) -> HashSet<String> {
    symbols.or(symbol).unwrap_or_default().split(',').map(str::trim)
        .filter(|v| !v.is_empty()).map(|v| v.to_ascii_uppercase()).collect()
}
async fn stream_ws(
    State(state): State<AppState>,
    Query(query): Query<StreamQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // The public Drogon gateway proxies this endpoint. Authenticate before
    // upgrading so an unauthenticated client cannot subscribe to live feeds.
    let _owner_uid = match firebase_uid_from_headers(state.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return status.into_response(),
    };
    let symbols = subscription_set(query.symbol, query.symbols);
    ws.on_upgrade(move |socket| stream_connection(socket, state, symbols)).into_response()
}
#[cfg(test)]
mod stream_auth_tests {
    use super::*;

    #[tokio::test]
    async fn stream_auth_rejects_missing_bearer_token() {
        let verifier = firebase_identity::FirebaseIdentityVerifier::new("qoin-test-project".into()).unwrap();
        let result = firebase_uid_from_headers(Some(&verifier), &HeaderMap::new()).await;
        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
    }

    #[tokio::test]
    async fn stream_auth_fails_closed_when_firebase_is_unconfigured() {
        let result = firebase_uid_from_headers(None, &HeaderMap::new()).await;
        assert_eq!(result, Err(StatusCode::SERVICE_UNAVAILABLE));
    }
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
async fn submit_market_event(client: &reqwest::Client, endpoint: &str, event: &MarketEvent, provider: &str) {
    match client.post(endpoint).json(event).send().await {
        Ok(response) if !response.status().is_success() => {
            let status=response.status();
            let body=response.text().await.unwrap_or_default();
            tracing::warn!(provider=%provider,symbol=%event.symbol,event_id=%event.event_id,status=%status,
                response=%body.chars().take(300).collect::<String>(),"provider observation rejected by engine ingestion");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(provider=%provider,symbol=%event.symbol,event_id=%event.event_id,
            error=%error,"provider observation could not reach engine ingestion"),
    }
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
                    let Some(timestamp)=data.get("E").and_then(|v|v.as_i64()) else { continue; };
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
                            event_type:"mark_price".into(),ts_ms:timestamp,price:stream_number(data.get("p")),size:None,side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:stream_number(data.get("r")),
                            open_interest:None,liquidation_side:None})
                    } else if event_name=="forceOrder" {
                        let order=data.get("o").unwrap_or(data);
                        let side=order.get("S").and_then(|v|v.as_str()).unwrap_or_default();
                        let liquidation_side=match side{"SELL"=>Some("long"),"BUY"=>Some("short"),_=>None};
                        let id=format!("{}:{}:{}:{}",order.get("T").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string()),side,order.get("p").and_then(|v|v.as_str()).unwrap_or(""),order.get("q").and_then(|v|v.as_str()).unwrap_or(""));
                        Some(MarketEvent{event_id:format!("liquidation:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"liquidation".into(),ts_ms:order.get("T").and_then(|v|v.as_i64()).unwrap_or(timestamp),
                            price:stream_number(order.get("p")),size:stream_number(order.get("q")),side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,open_interest:None,
                            liquidation_side:liquidation_side.map(str::to_owned)})
                    } else {None};
                    if let Some(event)=event {
                        submit_market_event(&client,&endpoint,&event,"binance_usdm").await;
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
    (StatusCode::OK,Json(serde_json::json!({"status":"legacy_strategy_updated","legacy_only":true,"applies_to_active_vector_engine":false,"strategy":stored,"live_trading":false})))
}
#[tokio::main]
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(error=%error, "failed to install Ctrl-C shutdown handler");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => { signal.recv().await; }
            Err(error) => {
                tracing::error!(error=%error, "failed to install SIGTERM shutdown handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl-C; graceful shutdown starting"),
        _ = terminate => tracing::info!("received SIGTERM; graceful shutdown starting"),
    }
}

async fn main(){
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init();
    let mut signal_config = EngineConfig::default();
    if let Ok(value) = env::var("QOIN_SIGNAL_THRESHOLD") { signal_config.signal_threshold = value.parse().expect("QOIN_SIGNAL_THRESHOLD must be numeric"); }
    if let Ok(value) = env::var("QOIN_INPUT_CLIP") { signal_config.input_clip = value.parse().expect("QOIN_INPUT_CLIP must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_SPREAD_BPS") { signal_config.max_spread_bps = value.parse().expect("QOIN_MAX_SPREAD_BPS must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_SPREAD_AGE_MS") { signal_config.max_spread_age_ms = value.parse().expect("QOIN_MAX_SPREAD_AGE_MS must be an integer"); }
    if let Ok(value) = env::var("QOIN_TOP_K") { signal_config.top_k = value.parse().expect("QOIN_TOP_K must be an integer"); }
    if let Ok(value) = env::var("QOIN_MIN_ALIGNMENT") { signal_config.min_alignment = value.parse().expect("QOIN_MIN_ALIGNMENT must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_ANGLE_RADIANS") { signal_config.max_angle_radians = value.parse().expect("QOIN_MAX_ANGLE_RADIANS must be numeric"); }
    let signal_config_version = env::var("QOIN_CONFIG_VERSION").unwrap_or_else(|_| "baseline-v2".into());
    decide_signal(ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
        open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0, spread_bps: Some(0.0), spread_age_ms: Some(0) },
        &signal_config, &signal_config_version, false).expect("Qoin signal engine configuration must be valid");

    let uri=env::var("QOIN_MONGODB_URI").unwrap_or_else(|_|"mongodb://127.0.0.1:27017".into());
    let client=Client::with_uri_str(&uri).await.expect("connect to MongoDB");
    let db=client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_|"qoin".into()));
    db.run_command(doc!{"ping":1},None).await.expect("MongoDB ping");
    let events:Collection<Document>=db.collection("market_events");
    let set_ledger_records:Collection<Document>=db.collection("set_relation_ledger");
    let states:Collection<Document>=db.collection("market_states");
    let decisions:Collection<Document>=db.collection("trade_decisions");
    let strategies:Collection<Document>=db.collection("edge_strategy");
    let orders:Collection<Document>=db.collection("live_orders");
    let order_lifecycle_events:Collection<Document>=db.collection("order_lifecycle_events");
    let trading_controls:Collection<Document>=db.collection("trading_controls");
    let paper_sessions:Collection<Document>=db.collection("paper_sessions_v2");
    let paper_session_decisions:Collection<Document>=db.collection("paper_session_decisions_v2");
    let paper_session_trades:Collection<Document>=db.collection("paper_session_trades_v2");
    let users:Collection<Document>=db.collection("qoin_users");
    let broker_accounts:Collection<Document>=db.collection("broker_accounts");
    let broker_connections:Collection<Document>=db.collection("broker_connections");
    let broker_connection_audit:Collection<Document>=db.collection("broker_connection_audit");
    broker_accounts.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"updated_at":-1}).build(),None).await
        .expect("Firebase user broker-account listing index");
    broker_connections.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"updated_at":-1}).build(),None).await
        .expect("Firebase user broker-connection listing index");
    broker_connections.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"broker_id":1,"account_id":1}).build(),None).await
        .expect("broker connection account lookup index");
    broker_connection_audit.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"at":-1}).build(),None).await
        .expect("broker connection audit index");
    broker_accounts.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"broker_id":1,"account_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("a broker subaccount can belong to only one Firebase user");
    trading_controls.update_one(doc!{"_id":"execution"},
        doc!{"$setOnInsert":{"enabled":env::var("QOIN_TRADING_ENABLED").map(|v|v=="true").unwrap_or(false),
            "updated_at":mongodb::bson::DateTime::now()}},
        UpdateOptions::builder().upsert(true).build()).await.expect("initialize trading kill switch");
    paper_sessions.create_index(mongodb::IndexModel::builder().keys(doc!{"session_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("owner-scoped paper session key index");
    paper_sessions.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"active":1})
        .build(),None).await.expect("owner-scoped paper session recovery index");
    paper_session_decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("owner-scoped paper decision idempotency index");
    paper_session_trades.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"venue":1,"symbol":1,"exit_ts_ms":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("owner-scoped paper trade idempotency index");
    paper_session_decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"ts_ms":-1})
        .build(),None).await.expect("owner-scoped paper decision history index");
    paper_session_trades.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"exit_ts_ms":-1})
        .build(),None).await.expect("owner-scoped paper trade history index");
    orders.create_index(mongodb::IndexModel::builder().keys(doc!{"client_order_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("durable live order idempotency index");
    order_lifecycle_events.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"client_order_id":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("immutable owner-scoped order lifecycle event identity index");
    order_lifecycle_events.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"client_order_id":1,"at":1}).build(),None).await
        .expect("owner-scoped order lifecycle timeline index");
    order_lifecycle_events.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"client_order_id":1,"state":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("idempotent order lifecycle state index");
    orders.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"updated_at":-1}).build(),None).await
        .expect("owner-scoped order history index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"ts_ms":1,"event_id":1}).build(),None).await.expect("historical replay time index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"ts_ms":1,"event_id":1,"venue":1}).build(),None).await.expect("V2 ledger backfill keyset index");
    set_ledger_records.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"event_time_ms":-1,"record_type":1}).build(),None).await.expect("durable V2 ledger symbol/time index");
    set_ledger_records.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"record_type":1,"relation_id":1}).build(),None).await.expect("durable V2 relation lookup index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"stored_at":-1}).build(),None).await.expect("latest state recovery index");
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
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded legacy edge-map compatibility settings; active decisions use qoin-vector-engine");
    // Rebuild rolling state from the immutable event log on every process start.
    // Snapshot-only recovery can preserve a stale snapshot when a crash occurs
    // after event insertion but before snapshot persistence. Replaying the raw
    // log closes that specific recovery gap and uses deterministic event-time
    // ordering across providers. Malformed legacy rows are skipped and surfaced.
    let replay_options = FindOptions::builder()
        .sort(doc!{"symbol":1,"ts_ms":1,"event_id":1,"venue":1})
        .build();
    let mut event_cursor = events.find(None, Some(replay_options)).await
        .expect("open immutable market event log for recovery");
    let mut restored_memory: HashMap<String, SymbolState> = HashMap::new();
    let mut replayed_events: u64 = 0;
    let mut skipped_events: u64 = 0;
    while let Some(document) = event_cursor.try_next().await
        .expect("read immutable market event log during recovery") {
        match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event) if validate_event(&event).is_ok() => {
                let state = restored_memory.entry(event.symbol.clone()).or_default();
                apply(state, &event);
                replayed_events += 1;
            }
            Ok(event) => {
                skipped_events += 1;
                tracing::warn!(symbol=%event.symbol,event_id=%event.event_id,
                    "skipping invalid persisted market event during recovery");
            }
            Err(error) => {
                skipped_events += 1;
                tracing::warn!(error=%error,
                    "skipping malformed persisted market event during recovery");
            }
        }
    }
    tracing::info!(restored_symbols=restored_memory.len(),replayed_events,skipped_events,
        "rebuilt market snapshots from immutable MongoDB event log");
    let mut persisted_paper_sessions: HashMap<String, PaperSession> = HashMap::new();
    match paper_sessions.find(doc!{"owner_uid":{"$type":"string"}}, None).await {
        Ok(mut cursor) => loop {
            match cursor.try_next().await {
                Ok(Some(mut document)) => {
                    let owner_uid = document.get_str("owner_uid").unwrap_or_default().to_owned();
                    document.remove("_id");
                    if owner_uid.is_empty() { continue; }
                    match mongodb::bson::from_document::<PaperSession>(document) {
                        Ok(session) => { persisted_paper_sessions.insert(owner_uid, session); }
                        Err(error) => tracing::error!(error=%error, "skipping malformed owner-scoped paper session during recovery"),
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::error!(error=%error, "owner-scoped paper session recovery scan failed");
                    break;
                }
            }
        },
        Err(error) => tracing::error!(error=%error, "owner-scoped paper session recovery query failed"),
    }
    let credential_vault = credential_vault::CredentialVault::from_env()
        .expect("broker credential encryption configuration must be valid");
    let firebase_verifier = env::var("QOIN_FIREBASE_PROJECT_ID").ok()
        .filter(|project_id| !project_id.trim().is_empty())
        .map(firebase_identity::FirebaseIdentityVerifier::new)
        .transpose()
        .expect("configure a valid Firebase project ID for authenticated trading routes");
    let (stream, _) = broadcast::channel(4096);
    let state=AppState{
        stream, database: db.clone(), events,set_ledger_records,states,decisions,strategies,orders,order_lifecycle_events,trading_controls,
        users, broker_accounts, broker_connections, broker_connection_audit, credential_vault,
        paper_sessions, paper_session_decisions, paper_session_trades,
        active_paper_sessions: Arc::new(Mutex::new(persisted_paper_sessions)),
        broker_registry: broker_contract::BrokerRegistry::default(),
        broker_environment: Arc::new(broker_env::BrokerEnvironment::from_env().expect("broker environment configuration must be valid")),
        firebase_verifier,
        memory:Arc::new(RwLock::new(restored_memory)),
        symbol_ingest_locks:Arc::new((0..64).map(|_| tokio::sync::Mutex::new(())).collect()),
        active_strategy:Arc::new(RwLock::new(active)),
        signal_config,
        signal_config_version,
        recovery_replayed_events: replayed_events,
        recovery_skipped_events: skipped_events,
    };
    let app=Router::new().route("/health",get(health)).route("/internal/brokers",get(broker_catalogue::get_brokers)).route("/v1/signal",post(evaluate_signal))
        .route("/internal/users/me",get(get_qoin_me))
        .route("/internal/broker-accounts",get(get_broker_accounts))
        .route("/internal/broker-connections",get(get_broker_connections).post(create_broker_connection))
        .route("/internal/broker-connections/rotate",post(rotate_broker_connection))
        .route("/internal/broker-connections/revoke",post(revoke_broker_connection))
        .route("/internal/admin/broker-accounts",post(provision_broker_account))
        .route("/internal/brokers/configuration",get(get_broker_configuration))
        .route("/internal/trading/readiness",get(get_broker_readiness))
        .route("/internal/paper-session",get(get_paper_session))
        .route("/internal/paper-session/trades",get(get_paper_trade_history))
        .route("/internal/paper-session/decisions",get(get_paper_decision_history))
        .route("/internal/paper-session/start",post(start_paper_session))
        .route("/internal/paper-session/stop",post(stop_paper_session))
        .route("/internal/orders",post(submit_live_order))
        .route("/internal/orders/:client_order_id/reconcile",get(reconcile_live_order))
        .route("/internal/trading/kill-switch",post(set_trading_enabled))
        .route("/internal/stream",get(stream_ws)).route("/internal/events",get(get_events).post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/vector/history",get(get_vector_history))
        .route("/internal/set-ledger",get(get_set_ledger))
        .route("/internal/set-ledger/reconstruct",get(reconstruct_set_ledger))
        .route("/internal/set-ledger/backfill",post(backfill_set_ledger))
         .route("/internal/set-ledger/corrections",get(list_set_ledger_corrections).post(propose_set_ledger_correction))
        .route("/internal/set-ledger/integrity",get(audit_set_ledger))
        .route("/internal/backtest",get(run_backtest)).route("/internal/instruments",get(get_instruments))
        .route("/internal/news",get(get_news)).route("/internal/calendar",get(get_calendar))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .route("/internal/algorithms",get(algorithm_catalogue::get_algorithms))
        .layer(DefaultBodyLimit::max(1_048_576)).with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    // Bind address may be 0.0.0.0 inside a container; provider callbacks must
    // connect through a concrete local interface instead of the wildcard address.
    let internal_addr=env::var("QOIN_AXUM_INTERNAL_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let binance_enabled=env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true);
    if binance_enabled {
        let symbols=env::var("QOIN_BINANCE_STREAM_SYMBOLS").unwrap_or_else(|_|"BTCUSDT,ETHUSDT,SOLUSDT,BNBUSDT,XRPUSDT,DOGEUSDT".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(|v|v.to_ascii_uppercase()).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        let open_interest_addr=internal_addr.clone();
        let open_interest_symbols=symbols.clone();
        tokio::spawn(async move{spawn_binance_stream(adapter_addr,symbols).await;});
        tokio::spawn(async move{spawn_binance_open_interest(open_interest_addr,open_interest_symbols).await;});
    } else { tracing::info!("Binance public market stream disabled by configuration"); }
    let kraken_enabled=env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
    if kraken_enabled {
        let symbols=env::var("QOIN_KRAKEN_STREAM_SYMBOLS").unwrap_or_else(|_|"BTC/USD,ETH/USD,SOL/USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_kraken_stream(adapter_addr,symbols).await; });
    } else {
        tracing::info!("Kraken Spot public market stream disabled by configuration");
    }
    let coinbase_enabled=env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
    if coinbase_enabled {
        let products=env::var("QOIN_COINBASE_STREAM_PRODUCTS").unwrap_or_else(|_|"BTC-USD,ETH-USD,SOL-USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_coinbase_stream(adapter_addr,products).await; });
    } else {
        tracing::info!("Coinbase public market stream disabled by configuration");
    }
    if let Ok(token) = env::var("FINNHUB_API_KEY") {
      if !token.trim().is_empty() {
        let symbols=env::var("QOIN_FINNHUB_STREAM_SYMBOLS").unwrap_or_else(|_|"AAPL,MSFT,NVDA,AMZN,GOOGL,META,TSLA,OANDA:EUR_USD,OANDA:GBP_USD,OANDA:USD_JPY,OANDA:AUD_USD,OANDA:SPX500_USD,OANDA:NAS100_USD,OANDA:US30_USD,OANDA:DE30_EUR,OANDA:XAU_USD,OANDA:XAG_USD,OANDA:WTICO_USD,OANDA:BCO_USD,OANDA:NATGAS_USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_finnhub_stream(adapter_addr,token,symbols).await; });
      } else { tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is empty"); }
    } else {
        tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is not configured");
    }
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).with_graceful_shutdown(shutdown_signal()).await.expect("Axum server failed");
}

#[cfg(test)]
mod market_stream_tests {
    use super::*;

    #[test]
    fn kraken_trade_messages_normalize_into_canonical_events() {
        let payload=serde_json::json!({
            "channel":"trade","type":"update","data":[{
                "symbol":"BTC/USD","side":"buy","price":65000.25,"qty":0.01,
                "trade_id":123456,"timestamp":"2026-11-09T12:00:00.123Z"
            }]
        });
        let events=parse_kraken_trade_message(&payload);
        assert_eq!(events.len(),1);
        assert_eq!(events[0].venue,"kraken_spot");
        assert_eq!(events[0].symbol,"BTCUSD_KRAKEN");
        assert_eq!(events[0].event_id,"kraken_trade:123456");
        assert_eq!(events[0].side.as_deref(),Some("buy"));
        assert_eq!(events[0].ts_ms,1_794_225_600_123);
    }

    #[test]
    fn kraken_parser_rejects_snapshots_and_malformed_trades() {
        assert!(parse_kraken_trade_message(&serde_json::json!({"channel":"trade","type":"snapshot","data":[]})).is_empty());
        let payload=serde_json::json!({"channel":"trade","type":"update","data":[{
            "symbol":"BTC/USD","side":"unknown","price":"1","qty":"1",
            "trade_id":1,"timestamp":"not-a-time"
        }]});
        assert!(parse_kraken_trade_message(&payload).is_empty());
    }

    #[test]
    fn coinbase_market_trade_messages_normalize_into_canonical_events() {
        let payload=serde_json::json!({
            "channel":"market_trades",
            "events":[{"type":"update","trades":[{
                "trade_id":"12345","product_id":"BTC-USD","price":"65000.25",
                "size":"0.012","side":"BUY","time":"2026-11-09T12:00:00.123Z"
            }]}]
        });
        let events=parse_coinbase_trade_message(&payload);
        assert_eq!(events.len(),1);
        assert_eq!(events[0].venue,"coinbase_advanced_trade");
        assert_eq!(events[0].symbol,"BTCUSD_COINBASE");
        assert_eq!(events[0].event_id,"coinbase_trade:12345");
        assert_eq!(events[0].side.as_deref(),Some("buy"));
        assert_eq!(events[0].ts_ms,1_794_225_600_123);
    }

    #[test]
    fn coinbase_parser_rejects_wrong_channels_and_malformed_trades() {
        assert!(parse_coinbase_trade_message(&serde_json::json!({"channel":"ticker","events":[]})).is_empty());
        let payload=serde_json::json!({
            "channel":"market_trades",
            "events":[{"trades":[{"trade_id":"bad","product_id":"BTC-USD","side":"BUY",
                "time":"not-a-time","price":"NaN","size":"1"}]}]
        });
        assert!(parse_coinbase_trade_message(&payload).is_empty());
    }

    #[test]
    fn websocket_symbol_subscriptions_are_trimmed_uppercase_and_deduplicated() {
        let symbols = subscription_set(None, Some(" btcusdt, ETHUSDT,btcusdt, ,solusdt ".into()));
        assert_eq!(symbols.len(), 3);
        assert!(symbols.contains("BTCUSDT"));
        assert!(symbols.contains("ETHUSDT"));
        assert!(symbols.contains("SOLUSDT"));
    }

    #[test]
    fn stream_numbers_accept_provider_string_and_numeric_values() {
        assert_eq!(stream_number(Some(&serde_json::json!("123.45"))), Some(123.45));
        assert_eq!(stream_number(Some(&serde_json::json!(0.25))), Some(0.25));
        assert_eq!(stream_number(Some(&serde_json::json!("not-a-number"))), None);
        assert_eq!(stream_number(None), None);
    }

    #[test]
    fn market_event_validation_rejects_unknown_types_and_invalid_sides() {
        let base = MarketEvent {
            event_id: "test-1".into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms: 1_700_000_000_000, price: Some(100.0),
            size: Some(1.0), side: Some("buy".into()), bid: None, ask: None,
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None,
            liquidation_side: None,
        };
        assert!(validate_event(&base).is_ok());
        let mut unknown = base.clone();
        unknown.event_type = "order".into();
        assert!(validate_event(&unknown).is_err());
        let mut invalid_side = base.clone();
        invalid_side.side = Some("BUY".into());
        assert!(validate_event(&invalid_side).is_err());
        let mut invalid_liquidation = base;
        invalid_liquidation.liquidation_side = Some("up".into());
        assert!(validate_event(&invalid_liquidation).is_err());
    }

    #[test]
    fn market_event_validation_rejects_non_finite_and_crossed_book_data() {
        let base = MarketEvent {
            event_id: "test-1".into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms: 1_700_000_000_000, price: Some(100.0),
            size: Some(1.0), side: Some("buy".into()), bid: None, ask: None,
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None,
            liquidation_side: None,
        };
        assert!(validate_event(&base).is_ok());
        let mut non_finite = base.clone();
        non_finite.price = Some(f64::NAN);
        assert!(validate_event(&non_finite).is_err());
        let mut crossed = base;
        crossed.bid = Some(101.0);
        crossed.ask = Some(100.0);
        assert!(validate_event(&crossed).is_err());
    }
}


#[cfg(test)]
mod execution_route_auth_tests {
    use super::constant_time_token_match;

    #[test]
    fn accepts_only_exact_execution_tokens() {
        assert!(constant_time_token_match(b"operator-token-123", b"operator-token-123"));
        assert!(!constant_time_token_match(b"operator-token-123", b"operator-token-124"));
        assert!(!constant_time_token_match(b"operator-token-123", b"operator-token-1234"));
        assert!(!constant_time_token_match(b"operator-token-123", b""));
    }
}


#[cfg(test)]
mod vector_ingestion_tests {
    use super::*;

    fn event(event_id: &str, event_type: &str, ts_ms: i64) -> MarketEvent {
        MarketEvent {
            event_id: event_id.into(),
            venue: "test".into(),
            symbol: "BTCUSDT".into(),
            event_type: event_type.into(),
            ts_ms,
            price: None,
            size: None,
            side: None,
            bid: None,
            ask: None,
            bid_size: None,
            ask_size: None,
            funding_rate: None,
            open_interest: None,
            liquidation_side: None,
        }
    }

    #[test]
    fn live_event_order_matches_deterministic_recovery_order() {
        let mut state = SymbolState::default();
        let current = event("b", "trade", 10_000);
        apply(&mut state, &current);

        let older_timestamp = event("z", "trade", 9_999);
        let earlier_same_timestamp = event("a", "trade", 10_000);
        let later_same_timestamp = event("c", "trade", 10_000);

        assert!(!event_follows_state(&older_timestamp, &state));
        assert!(!event_follows_state(&earlier_same_timestamp, &state));
        assert!(event_follows_state(&later_same_timestamp, &state));
    }

    #[test]
    fn mark_price_does_not_overwrite_last_trade_and_derives_basis() {
        let mut state = SymbolState::default();
        let mut trade = event("trade-1", "trade", 1_000);
        trade.price = Some(100.0);
        trade.size = Some(1.0);
        trade.side = Some("buy".into());
        apply(&mut state, &trade);

        let mut mark = event("mark-1", "mark_price", 2_000);
        mark.price = Some(101.0);
        mark.funding_rate = Some(0.001);
        apply(&mut state, &mark);

        assert_eq!(state.last_price, Some(100.0));
        assert_eq!(state.mark_price, Some(101.0));
        assert_eq!(state.basis_bps, Some(100.0));
        assert_eq!(state.funding_rate, Some(0.001));
    }

    #[test]
    fn book_depth_is_separate_from_decayed_trade_flow() {
        let mut state = SymbolState::default();
        let mut trade = event("trade-1", "trade", 1_000);
        trade.price = Some(100.0);
        trade.size = Some(10.0);
        trade.side = Some("buy".into());
        apply(&mut state, &trade);
        assert_eq!(state.order_flow_imbalance, 1.0);

        let mut book = event("book-1", "book", 61_000);
        book.bid = Some(100.0);
        book.ask = Some(100.02);
        book.bid_size = Some(30.0);
        book.ask_size = Some(10.0);
        apply(&mut state, &book);

        assert!((state.signed_flow - 5.0).abs() < 1e-12);
        assert!((state.total_trade_size - 5.0).abs() < 1e-12);
        assert!((state.order_flow_imbalance - 1.0).abs() < 1e-12);
        assert!((state.liquidity_imbalance - 0.5).abs() < 1e-12);
        let stale = event("stale", "book", 61_001);
        apply(&mut state, &stale);
        assert_eq!(state.order_flow_imbalance, 0.0);
    }

    #[test]
    fn liquidation_pressure_is_signed_and_decays_with_event_time() {
        let mut state = SymbolState::default();
        let mut liquidation = event("liq-1", "liquidation", 1_000);
        liquidation.liquidation_side = Some("long".into());
        apply(&mut state, &liquidation);
        assert_eq!(state.liquidation_pressure, -1.0);

        let next = event("trade-1", "trade", 61_000);
        apply(&mut state, &next);
        assert!((state.liquidation_pressure + 0.5).abs() < 1e-12);
    }

    #[test]
    fn open_interest_force_is_a_relative_change_not_an_absolute_level() {
        let mut state = SymbolState::default();
        let mut first = event("oi-1", "open_interest", 1_000);
        first.open_interest = Some(1_000.0);
        apply(&mut state, &first);
        let mut second = event("oi-2", "open_interest", 2_000);
        second.open_interest = Some(1_100.0);
        apply(&mut state, &second);

        assert_eq!(state.open_interest, Some(1_100.0));
        assert!((state.open_interest_change - 0.1).abs() < 1e-12);
    }
}
