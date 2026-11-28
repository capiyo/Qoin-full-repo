use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Query, State},
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
use std::{collections::{BTreeMap, HashMap, HashSet, VecDeque}, env, sync::{Arc, RwLock}, time::Duration};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::sync::{broadcast, Mutex};
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
mod broker_env;
mod broker_env_api;
mod broker_allocation;
mod broker_allocation_policy;
mod broker_fanout;
mod market_data_quality;
mod market_data_backfill;
mod market_data_health;
mod v2_relationships;
mod v2_evaluation;
mod paper_session;
mod operating_cycle;
mod notifications;
use operating_cycle::{CycleConfig, OperatingCycle};
use paper_session::PaperSession;
mod market_vector;
mod market_vector_api;
use edge_strategy::{EdgeStrategy, STRATEGY_KEY};
use qoin_vector_engine::{aggregate_horizon, decide as decide_signal, evaluate_multi_horizon, EngineConfig, ForceObservation, Horizon, HorizonInput, MarketEvent as HorizonMarketEvent, Signal};

mod models;
mod handlers;
mod routes;
use models::*;

#[tokio::main]
async fn main() {
    routes::run().await;
}
