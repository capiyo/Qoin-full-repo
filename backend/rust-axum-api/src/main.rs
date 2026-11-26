use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use qoin_vector_engine::{decide, EngineConfig, ForceObservation, Signal};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    config: EngineConfig,
    config_version: String,
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
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0,
            spread_bps: None, spread_age_ms: None },
        &config, &config_version, false,
    ).expect("Qoin engine configuration must be valid");
    let state = Arc::new(AppState { config, config_version });
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE]);
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/signal", post(signal))
        .layer(DefaultBodyLimit::max(65_536))
        .layer(cors)
        .with_state(state);

    // Loopback binding keeps this research API local by default.
    let research_port = std::env::var("QOIN_RESEARCH_API_PORT")
        .ok()
        .map(|value| value.parse::<u16>().expect("QOIN_RESEARCH_API_PORT must be a valid port"))
        .unwrap_or(8082);
    let addr = SocketAddr::from(([127, 0, 0, 1], research_port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind API listener");
    tracing::info!(%addr, "Qoin research API listening; live trading is disabled");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.expect("serve API");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install research API Ctrl-C handler");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => { signal.recv().await; }
            Err(error) => {
                tracing::error!(%error, "failed to install research API SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => tracing::info!("research API graceful shutdown requested"),
        _ = terminate => tracing::info!("research API graceful shutdown requested"),
    }
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
