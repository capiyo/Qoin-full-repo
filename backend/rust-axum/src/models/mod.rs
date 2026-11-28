#![allow(unused_imports)]
use super::*;

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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub(crate) struct SymbolState {
    #[serde(default)] pub(crate) venue: String,
    pub(crate) symbol: String,
    pub(crate) event_count: usize,
    pub(crate) last_event_ts_ms: Option<i64>,
    #[serde(default)] pub(crate) last_event_id: Option<String>,
    #[serde(default)] pub(crate) last_event_venue: Option<String>,
    pub(crate) last_price: Option<f64>,
    #[serde(default)] pub(crate) mark_price: Option<f64>,
    #[serde(default)] pub(crate) basis_bps: Option<f64>,
    pub(crate) spread_bps: Option<f64>,
    #[serde(default)] pub(crate) last_book_ts_ms: Option<i64>,
    #[serde(default)] pub(crate) liquidity_imbalance: f64,
    #[serde(default)] pub(crate) liquidity_available: bool,
    #[serde(default)] pub(crate) last_trade_ts_ms: Option<i64>,
    #[serde(skip)] pub(crate) last_price_ts_ms: Option<i64>,
    #[serde(skip)] pub(crate) recent_return_observations: VecDeque<(i64, f64)>,
    pub(crate) signed_flow: f64,
    pub(crate) total_trade_size: f64,
    pub(crate) order_flow_imbalance: f64,
    pub(crate) event_intensity: f64,
    pub(crate) ewma_abs_return: f64,
    pub(crate) funding_rate: Option<f64>,
    pub(crate) open_interest: Option<f64>,
    #[serde(default)] pub(crate) open_interest_change: f64,
    #[serde(default)] pub(crate) liquidation_pressure: f64,
    pub(crate) liquidation_events: usize,
    pub(crate) last_liquidation_side: Option<String>,
    pub(crate) state_quality: f64,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) stream: broadcast::Sender<serde_json::Value>,
    pub(crate) database: mongodb::Database,
    pub(crate) events: Collection<Document>,
    pub(crate) dataset_manifests: Collection<Document>,
    pub(crate) market_backfill_jobs: Collection<Document>,
    pub(crate) set_ledger_records: Collection<Document>,
    pub(crate) relationship_ledger: Collection<Document>,
    pub(crate) states: Collection<Document>,
    pub(crate) decisions: Collection<Document>,
    pub(crate) strategies: Collection<Document>,
    pub(crate) orders: Collection<Document>,
    pub(crate) trading_controls: Collection<Document>,
    pub(crate) users: Collection<Document>,
    pub(crate) notifications: Collection<Document>,
    pub(crate) market_vectors: Collection<Document>,
    pub(crate) fcm_tokens: Collection<Document>,
    pub(crate) algorithm_selections: Collection<Document>,
    pub(crate) broker_accounts: Collection<Document>,
    pub(crate) broker_registry: broker_contract::BrokerRegistry,
    pub(crate) broker_environment: Arc<broker_env::BrokerEnvironment>,
    pub(crate) firebase_verifier: Option<firebase_identity::FirebaseIdentityVerifier>,
    pub(crate) memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    pub(crate) symbol_ingest_locks: Arc<Vec<tokio::sync::Mutex<()>>>,
    pub(crate) active_strategy: Arc<RwLock<EdgeStrategy>>,
    pub(crate) signal_config: EngineConfig,
    pub(crate) signal_strategy_version: String,
    pub(crate) recovery_replayed_events: u64,
    pub(crate) recovery_skipped_events: u64,
    pub(crate) operating_cycles: Collection<Document>,
    pub(crate) paper_sessions: Collection<Document>,
    pub(crate) paper_session_runs: Collection<Document>,
    pub(crate) paper_session_decisions: Collection<Document>,
    pub(crate) paper_session_trades: Collection<Document>,
    pub(crate) investment_plans: Collection<Document>,
    pub(crate) active_cycles: Arc<Mutex<HashMap<String, OperatingCycle>>>,
    pub(crate) active_paper_sessions: Arc<Mutex<HashMap<String, PaperSession>>>,
}

#[derive(Deserialize)]
pub(crate) struct MarketHorizonQuery { pub(crate) symbol: Option<String>, pub(crate) venue_id: Option<String>, pub(crate) min_events: Option<usize> }

#[derive(Deserialize)]
pub(crate) struct PaperHistoryQuery { pub(crate) symbol: Option<String>, pub(crate) cycle_id: Option<String>, pub(crate) limit: Option<i64> }

#[derive(Deserialize)]
pub(crate) struct PaperRunsQuery { pub(crate) limit: Option<i64> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InvestmentPlan { pub(crate) goal: String, pub(crate) currency: String, pub(crate) monthly_contribution_minor: u64, pub(crate) target_amount_minor: Option<u64>, pub(crate) horizon_months: u16, pub(crate) updated_at: Option<String> }

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct InvestmentPlanRequest { pub(crate) goal: String, pub(crate) currency: String, pub(crate) monthly_contribution_minor: u64, pub(crate) target_amount_minor: Option<u64>, pub(crate) horizon_months: u16 }

#[derive(Deserialize)]
pub(crate) struct SignalRequest { pub(crate) observation: ForceObservation, pub(crate) has_open_position: bool }

#[derive(Deserialize)]
pub(crate) struct SymbolQuery { pub(crate) symbol: Option<String>, pub(crate) venue: Option<String> }

#[derive(Deserialize)]
pub(crate) struct StreamQuery { pub(crate) symbol: Option<String>, pub(crate) symbols: Option<String> }

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AlgorithmSelectionRequest { pub(crate) symbol: String, pub(crate) algorithm_id: String, pub(crate) version: String }

#[derive(Deserialize)]
pub(crate) struct TradingControlRequest { pub(crate) enabled: bool }

#[derive(Deserialize)]
pub(crate) struct BrokerReadinessQuery { pub(crate) brokers: Option<String> }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BrokerOrderSubmission {
    pub(crate) broker_id: String,
    pub(crate) order: broker_contract::NormalizedOrder,
    pub(crate) preflight: broker_contract::ExecutionPreflight,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StoredAlgorithmSelection { pub(crate) symbol: String, pub(crate) algorithm_id: String, pub(crate) version: String, pub(crate) selected_at_ms: i64 }

#[derive(Deserialize)]
pub(crate) struct HistoryQuery { pub(crate) symbol: Option<String>, pub(crate) venue: Option<String>, pub(crate) limit: Option<i64>, pub(crate) at_ms: Option<i64> }

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProviderHealthQuery {
    pub(crate) venue: Option<String>,
    pub(crate) symbol: Option<String>,
    pub(crate) max_age_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct DatasetManifestRequest { pub(crate) symbol: String, pub(crate) venue: String, pub(crate) from_ts_ms: i64, pub(crate) to_ts_ms: i64, #[serde(default)] pub(crate) max_gap_ms: Option<i64> }

#[derive(Deserialize)]
pub(crate) struct DatasetManifestQuery { pub(crate) symbol: Option<String>, pub(crate) venue: Option<String>, pub(crate) limit: Option<i64> }

#[derive(Deserialize)]
pub(crate) struct BacktestQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) venue: Option<String>,
    pub(crate) from_ts_ms: Option<i64>,
    pub(crate) to_ts_ms: Option<i64>,
    pub(crate) limit: Option<i64>,
    pub(crate) initial_capital: Option<f64>,
    pub(crate) fee_bps: Option<f64>,
    pub(crate) slippage_bps: Option<f64>,
    pub(crate) latency_ms: Option<i64>,
    pub(crate) max_participation_rate: Option<f64>,
    pub(crate) market_impact_bps: Option<f64>,
    pub(crate) algorithm_id: Option<String>,
    pub(crate) algorithm_version: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct LedgerCorrectionRequest {
    pub(crate) symbol: String,
    pub(crate) venue: String,
    pub(crate) event_id: String,
    pub(crate) expected_source_hash: String,
    pub(crate) corrected_event: MarketEvent,
    pub(crate) reason: String,
}

#[derive(Deserialize)]
pub(crate) struct LedgerCorrectionQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct LedgerBackfillQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) after_ts_ms: Option<i64>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) after_venue: Option<String>,
    pub(crate) batch_size: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct LedgerIntegrityQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) limit: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct NewsQuery { pub(crate) category: Option<String> }
