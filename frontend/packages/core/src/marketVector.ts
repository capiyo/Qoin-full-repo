/** Shared Qoin market-state contract. Mirrors qoin-market-space schema v1. */
export const QOIN_MARKET_VECTOR_SCHEMA_VERSION = 1 as const;

export const QOIN_MARKET_FEATURES = [
  "log_return", "price_displacement", "rate_of_change", "gap_magnitude", "swing_position",
  "realized_volatility", "implied_volatility", "volatility_of_volatility", "return_skew", "return_kurtosis", "jump_intensity",
  "momentum", "trend_strength", "trend_persistence", "mean_reversion_strength", "trend_exhaustion",
  "spread_bps", "order_book_depth", "order_flow_imbalance", "volume_delta", "market_impact", "slippage_estimate", "liquidity_concentration",
  "correlation", "covariance", "beta", "correlation_breakdown", "relative_strength",
  "funding_pressure", "open_interest_change", "liquidation_pressure", "gamma_exposure", "delta_exposure", "basis",
  "event_intensity", "macro_surprise", "event_proximity",
  "regime_trend_strength", "regime_volatility_level", "regime_liquidity_stress", "regime_correlation_stress",
  "regime_transition_intensity", "regime_uncertainty", "regime_persistence",
  "portfolio_exposure", "concentration", "drawdown", "tail_risk", "risk_contribution",
  "time_since_structural_break", "feature_persistence", "signal_decay", "timeframe_agreement",
] as const;

export type QoinMarketFeature = (typeof QOIN_MARKET_FEATURES)[number];
export type QoinMarketFeatureFamily =
  | "price" | "volatility" | "trend" | "liquidity" | "cross_asset"
  | "derivatives" | "events" | "regime" | "portfolio" | "temporal";

export type QoinFeatureAvailability = "observed" | "missing" | "not_applicable";

export type QoinFeatureCoordinate = {
  status: QoinFeatureAvailability;
  /** Past-only normalized coordinate in [-1, 1]; null means unavailable, not zero. */
  value: number | null;
  /** Reliability in (0, 1] for observed values; exactly zero when missing. */
  quality: number;
};

export type QoinMarketVector = {
  vector_id: string;
  schema_version: typeof QOIN_MARKET_VECTOR_SCHEMA_VERSION;
  venue: string;
  symbol: string;
  timeframe: string;
  event_time_cutoff_ms: number;
  available_at_ms: number;
  /** All 53 schema dimensions must be present, including explicit missing values. */
  coordinates: Record<QoinMarketFeature, QoinFeatureCoordinate>;
};

export type QoinMarketVectorComparison = {
  cosine_similarity: number;
  weighted_euclidean_distance: number;
  common_dimensions: number;
  coverage: number;
};

export type QoinMarketVectorDecision = {
  signal: {
    decision: "BUY" | "SELL" | "CLOSE" | "WAIT";
    score: number;
    alignment: number;
    angle_radians: number;
    active_force_count: number;
    config_version: string;
    rationale: string;
  };
  regime_risk_score: number;
  effective_signal_threshold: number;
  effective_volatility_dampener: number;
};

export type QoinMarketVectorSignalRequest = {
  vector: QoinMarketVector;
  has_open_position: boolean;
};

export type QoinMarketVectorSignalResponse = {
  algorithm_version: "1";
  vector_id: string;
  vector_schema_version: typeof QOIN_MARKET_VECTOR_SCHEMA_VERSION;
  decision: QoinMarketVectorDecision;
  live_trading: false;
};

export type QoinVectorEvidenceConfig = {
  negative_threshold: number;
  positive_threshold: number;
  minimum_quality: number;
  include_missing: boolean;
  include_not_applicable: boolean;
};

export type QoinDiscoverVectorRelationshipsRequest = {
  query: QoinMarketVector;
  candidates: QoinMarketVector[];
  model_version: string;
  temperature: number;
  recorded_at_ms: number;
  evidence_config?: QoinVectorEvidenceConfig;
};
