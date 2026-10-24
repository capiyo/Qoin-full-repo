use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use qoin_vector_engine::{DecisionContext, EdgeDecision, EdgeStrategy, ForceObservation};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    strategy: EdgeStrategy,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    mode: &'static str,
    live_trading: bool,
    strategy_version: String,
}

#[derive(Deserialize)]
struct SignalRequest {
    #[serde(default = "default_symbol")]
    symbol: String,
    #[serde(default)]
    observation: ForceObservation,
    #[serde(default)]
    context: DecisionContext,
}

fn default_symbol() -> String { "BTCUSDT".to_owned() }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // This compatibility endpoint calls the same canonical engine as Axum.
    // It is signal-only and fail-closed when required market/risk context is absent.
    let mut strategy = EdgeStrategy::baseline();
    if let Ok(value) = std::env::var("QOIN_ANGULAR_THRESHOLD_DEGREES") {
        strategy.angular_threshold_degrees = value.parse().expect("QOIN_ANGULAR_THRESHOLD_DEGREES must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_MAGNITUDE_THRESHOLD") {
        strategy.magnitude_threshold = value.parse().expect("QOIN_MAGNITUDE_THRESHOLD must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_MAX_SPREAD_BPS") {
        strategy.max_spread_bps = value.parse().expect("QOIN_MAX_SPREAD_BPS must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_ATTENTION_SCALE") {
        strategy.attention_scale = value.parse().expect("QOIN_ATTENTION_SCALE must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_MIN_PERSISTENCE_TICKS") {
        strategy.min_persistence_ticks = value.parse().expect("QOIN_MIN_PERSISTENCE_TICKS must be an integer");
    }
    if let Ok(value) = std::env::var("QOIN_COST_MULTIPLIER") {
        strategy.cost_multiplier = value.parse().expect("QOIN_COST_MULTIPLIER must be numeric");
    }
    if let Ok(value) = std::env::var("QOIN_CONFIG_VERSION") {
        strategy.version = value;
    }
    strategy.validate().expect("Qoin strategy configuration must be valid");
    let state = Arc::new(AppState { strategy });
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

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind API listener");
    tracing::info!(%addr, "Qoin canonical research signal API listening; live trading is disabled");
    axum::serve(listener, app).await.expect("serve API");
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        mode: "research",
        live_trading: false,
        strategy_version: state.strategy.version.clone(),
    })
}

async fn signal(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SignalRequest>,
) -> Result<Json<EdgeDecision>, (StatusCode, String)> {
    let observation = request.observation;
    let result = state.strategy.decide_with_forces(
        &request.symbol,
        observation.ofi,
        observation.book_imbalance,
        Some(observation.funding),
        observation.spread_bps,
        observation.volatility,
        observation.event_intensity,
        observation.liquidation_pressure,
        observation.open_interest_change,
        Some(observation.basis),
        &request.context,
    ).map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    // A signal is not an order. No exchange adapter is called here.
    Ok(Json(result))
}
