use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use qoin_vector_engine::{decide, CostContext, EngineConfig, ForceObservation, Signal};
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
    #[serde(default)]
    costs: Option<CostContext>,
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
    if let Ok(value) = std::env::var("QOIN_MAX_ANGLE_RADIANS") {
        config.max_angle_radians = value.parse().expect("QOIN_MAX_ANGLE_RADIANS must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_EXIT_THRESHOLD") {
        config.exit_threshold = value.parse().expect("QOIN_EXIT_THRESHOLD must be numeric");
    }
    let config_version = std::env::var("QOIN_CONFIG_VERSION")
        .unwrap_or_else(|_| "baseline-v2".to_owned());
    // Validate configuration before opening the HTTP listener.
    qoin_vector_engine::decide(
        ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 },
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
        .layer(cors)
        .with_state(state);

    // Loopback binding keeps this research API local by default.
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
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
        request.costs.as_ref(),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // A signal is not an order. No exchange adapter is called here.
    Ok(Json(result))
}
