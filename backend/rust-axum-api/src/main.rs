mod auth;

use auth::FirebaseVerifier;
use axum::{
    extract::State,
    http::{header::{AUTHORIZATION, CONTENT_TYPE}, HeaderMap, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use qoin_vector_engine::{decide, EngineConfig, ForceObservation, Signal};
use mongodb::{
    bson::{doc, Bson, Document},
    options::{IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, env, net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    config: EngineConfig,
    config_version: String,
    portfolios: Option<Collection<Document>>,
    firebase: Option<Arc<FirebaseVerifier>>,
}

#[derive(Serialize, Deserialize)]
struct Portfolio {
    symbols: Vec<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct PortfolioRequest {
    symbols: Vec<String>,
}

fn validate_symbols(symbols: Vec<String>) -> Result<Vec<String>, String> {
    if symbols.len() > 500 {
        return Err("a portfolio may contain at most 500 instruments".to_owned());
    }
    let mut seen = HashSet::new();
    let mut accepted = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let normalized = symbol.trim().to_ascii_uppercase();
        if normalized.is_empty() || normalized.len() > 32
            || !normalized.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()
                || matches!(c, '/' | '-' | '_' | '.'))
            || !normalized.chars().any(|c| c.is_ascii_alphabetic()) {
            return Err("portfolio contains an invalid instrument symbol".to_owned());
        }
        if seen.insert(normalized.clone()) {
            accepted.push(normalized);
        }
    }
    Ok(accepted)
}

async fn authenticated_uid(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, (StatusCode, String)> {
    let verifier = state.firebase.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Firebase authentication is not configured".to_owned()))?;
    let value = headers.get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "bearer token required".to_owned()))?;
    let token = value.strip_prefix("Bearer ")
        .filter(|token| !token.trim().is_empty())
        .ok_or((StatusCode::UNAUTHORIZED, "bearer token required".to_owned()))?;
    verifier.verify(token).await
        .map(|user| user.uid)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "bearer token is invalid or expired".to_owned()))
}

async fn get_portfolio(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Portfolio>, (StatusCode, String)> {
    let uid = authenticated_uid(&state, &headers).await?;
    let collection = state.portfolios.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "portfolio storage is not configured".to_owned()))?;
    let stored = collection.find_one(doc! { "uid": &uid }, None).await
        .map_err(|error| {
            tracing::error!(%error, "portfolio read failed");
            (StatusCode::SERVICE_UNAVAILABLE, "portfolio storage unavailable".to_owned())
        })?;
    let symbols = stored.as_ref()
        .and_then(|document| document.get_array("symbols").ok())
        .map(|items| items.iter().filter_map(|item| match item {
            Bson::String(value) => Some(value.clone()),
            _ => None,
        }).collect())
        .unwrap_or_default();
    let updated_at = stored.as_ref()
        .and_then(|document| document.get_datetime("updated_at").ok())
        .map(|value| value.to_string());
    Ok(Json(Portfolio { symbols, updated_at }))
}

async fn put_portfolio(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<PortfolioRequest>,
) -> Result<Json<Portfolio>, (StatusCode, String)> {
    let uid = authenticated_uid(&state, &headers).await?;
    let collection = state.portfolios.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "portfolio storage is not configured".to_owned()))?;
    let symbols = validate_symbols(request.symbols)
        .map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    let updated = mongodb::bson::DateTime::now();
    let symbols_bson = mongodb::bson::to_bson(&symbols)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "portfolio serialization failed".to_owned()))?;
    let update = doc! { "$set": { "symbols": symbols_bson, "updated_at": updated } };
    collection.update_one(
        doc! { "uid": &uid },
        update,
        Some(UpdateOptions::builder().upsert(true).build()),
    ).await.map_err(|error| {
        tracing::error!(%error, "portfolio write failed");
        (StatusCode::SERVICE_UNAVAILABLE, "portfolio storage unavailable".to_owned())
    })?;
    Ok(Json(Portfolio { symbols, updated_at: Some(updated.to_string()) }))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    mode: &'static str,
    live_trading: bool,
}

#[derive(Deserialize)]
struct SignalRequest {
    observation: ForceObservation,
    has_open_position: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Every runtime parameter is explicit and tied to a config version.
    // Invalid values fail startup rather than silently changing model behavior.
    let mut config = EngineConfig::default();
    if let Ok(value) = std::env::var("QOIN_SIGNAL_THRESHOLD") {
        config.signal_threshold = value.parse().expect("QOIN_SIGNAL_THRESHOLD must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_INPUT_CLIP") {
        config.input_clip = value.parse().expect("QOIN_INPUT_CLIP must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_TOP_K") {
        config.top_k = value.parse().expect("QOIN_TOP_K must be an integer");
    }
    if let Ok(value) = std::env::var("QOIN_MIN_ALIGNMENT") {
        config.min_alignment = value.parse().expect("QOIN_MIN_ALIGNMENT must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_MAX_ANGLE_RADIANS") {
        config.max_angle_radians = value.parse().expect("QOIN_MAX_ANGLE_RADIANS must be numeric");
    }
    let config_version = std::env::var("QOIN_CONFIG_VERSION")
        .unwrap_or_else(|_| "baseline-v2".to_owned());
    // Validate configuration before opening the HTTP listener.
    qoin_vector_engine::decide(
        ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 },
        &config, &config_version, false,
    ).expect("Qoin engine configuration must be valid");
    let firebase = match env::var("QOIN_FIREBASE_PROJECT_ID") {
        Ok(project_id) if !project_id.trim().is_empty() => Some(Arc::new(
            FirebaseVerifier::new(project_id).expect("create Firebase HTTP client"),
        )),
        _ => {
            tracing::warn!("QOIN_FIREBASE_PROJECT_ID is unset; authenticated portfolio endpoints will return 503");
            None
        }
    };
    let portfolios = match env::var("QOIN_MONGODB_URI") {
        Ok(uri) if !uri.trim().is_empty() => {
            let client = Client::with_uri_str(&uri).await.expect("connect portfolio MongoDB client");
            let database = client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_| "qoin".to_owned()));
            let collection: Collection<Document> = database.collection("user_portfolios");
            collection.create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! { "uid": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
                None,
            ).await.expect("create unique user portfolio index");
            Some(collection)
        }
        _ => {
            tracing::warn!("QOIN_MONGODB_URI is unset; authenticated portfolio endpoints will return 503");
            None
        }
    };
    let state = Arc::new(AppState { config, config_version, portfolios, firebase });
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/signal", post(signal))
        .route("/v1/portfolio", get(get_portfolio).put(put_portfolio))
        .layer(cors)
        .with_state(state);

    // Loopback binding keeps this research API local by default.
    let addr: SocketAddr = std::env::var("QOIN_RESEARCH_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8082".to_owned())
        .parse()
        .expect("QOIN_RESEARCH_API_ADDR must be a valid socket address");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind API listener");
    tracing::info!(%addr, "Qoin research API listening; live trading is disabled");
    axum::serve(listener, app).await.expect("serve API");
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok", mode: "research", live_trading: false })
}

async fn signal(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SignalRequest>,
) -> Result<Json<Signal>, (StatusCode, String)> {
    let result = decide(
        request.observation,
        &state.config,
        &state.config_version,
        request.has_open_position,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // A signal is not an order. No exchange adapter is called here.
    Ok(Json(result))
}
