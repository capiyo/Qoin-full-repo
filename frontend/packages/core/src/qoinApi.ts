export type MarketForceObservation = {
  ofi: number;
  liquidity: number;
  volatility: number;
  funding: number;
  open_interest_change: number;
  liquidation_pressure: number;
  basis: number;
};

export type QoinDecision = "BUY" | "SELL" | "CLOSE" | "WAIT";

export type QoinSignal = {
  decision: QoinDecision;
  score: number;
  alignment: number;
  angle_radians: number;
  active_force_count: number;
  config_version: string;
  rationale: string;
};

export type QoinSignalRequest = {
  observation: MarketForceObservation;
  has_open_position: boolean;
};

export type QoinHealth = {
  status: "ok" | string;
  mode: "research" | string;
  live_trading: false;
};

function getBaseUrl(): string {
  // Android emulator: http://10.0.2.2:8080; iOS simulator: http://127.0.0.1:8080.
  // Physical devices need a reachable host configured for the development environment.
  const runtime = globalThis as typeof globalThis & {
    process?: { env?: Record<string, string | undefined> };
  };
  const env = runtime.process?.env;
  const configured = env?.EXPO_PUBLIC_QOIN_API_URL ?? env?.NEXT_PUBLIC_QOIN_API_URL;
  return (configured || "http://127.0.0.1:8080").replace(/\/$/, "");
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${getBaseUrl()}${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    throw new Error(`Qoin API ${response.status}: ${detail || response.statusText}`);
  }
  return response.json() as Promise<T>;
}

export function getQoinHealth(): Promise<QoinHealth> {
  return request<QoinHealth>("/health");
}

/** Returns a research signal only. This client never submits exchange orders. */
export function evaluateQoinSignal(input: QoinSignalRequest): Promise<QoinSignal> {
  return request<QoinSignal>("/api/v1/signal", {
    method: "POST",
    body: JSON.stringify(input),
  });
}
export type QoinAssetClass = "forex" | "crypto" | "stock";

export type QoinHorizon =
  | "one_minute"
  | "five_minutes"
  | "fifteen_minutes"
  | "thirty_minutes"
  | "one_hour"
  | "four_hours"
  | "one_day"
  | "one_week";

export type QoinEngineConfig = {
  signal_threshold: number;
  input_clip: number;
  score_clip: number;
  top_k: number;
  min_alignment: number;
  max_angle_radians: number;
  volatility_dampener: number;
  interaction_coefficient: number;
};

export type QoinHorizonInput = {
  horizon: QoinHorizon;
  observation: MarketForceObservation;
  config: QoinEngineConfig;
  config_version: string;
  has_open_position: boolean;
};

export type QoinHorizonSignal = {
  horizon: QoinHorizon;
  duration_seconds: number;
  signal: QoinSignal;
};

export type QoinHorizonConsensus = {
  relationship: "aligned_buy" | "aligned_sell" | "divergent" | "no_directional_consensus";
  buy_count: number;
  sell_count: number;
  wait_count: number;
  close_count: number;
  directional_count: number;
  unanimous_direction: QoinDecision | null;
};

export type QoinMultiHorizonSignal = {
  horizons: QoinHorizonSignal[];
  consensus: QoinHorizonConsensus;
};

/**
 * Evaluates separately constructed, causal observations for each selected
 * horizon. The result is descriptive research output, never an order.
 */
export function evaluateQoinMultiHorizon(
  inputs: QoinHorizonInput[],
): Promise<QoinMultiHorizonSignal> {
  return request<QoinMultiHorizonSignal>("/api/v1/multi-horizon", {
    method: "POST",
    body: JSON.stringify(inputs),
  });
}

export type QoinHorizonObservation = {
  horizon: QoinHorizon;
  observation: MarketForceObservation;
  event_count: number;
  trade_count: number;
  book_count: number;
  first_event_ts_ms: number | null;
  last_event_ts_ms: number | null;
  data_quality: number;
  ready: boolean;
  reasons: string[];
};

export type QoinMarketHorizonsResponse = {
  symbol: string;
  asset_class: QoinAssetClass;
  decision_ts_ms: number;
  events_loaded: number;
  event_limit_reached: boolean;
  mode: "research_only";
  observations: QoinHorizonObservation[];
  signals: QoinMultiHorizonSignal | null;
  note: string;
};

/** Builds causal horizon-specific observations from events already ingested by Qoin. */
export function getQoinMarketHorizons(
  symbol: string,
  assetClass: QoinAssetClass,
  minEvents = 20,
): Promise<QoinMarketHorizonsResponse> {
  if (!symbol.trim() || symbol.length > 64) {
    return Promise.reject(new Error("symbol must contain 1 to 64 characters"));
  }
  if (!Number.isInteger(minEvents) || minEvents < 5 || minEvents > 10_000) {
    return Promise.reject(new Error("minEvents must be an integer between 5 and 10000"));
  }
  const params = `symbol=${encodeURIComponent(symbol)}&asset_class=${assetClass}&min_events=${minEvents}`;
  return request<QoinMarketHorizonsResponse>(`/api/v1/market-horizons?${params}`);
}

export type QoinInstrumentCatalogue = {
  asset_class: QoinAssetClass;
  count: number;
  symbols: string[];
  source: "ingested_market_events";
};

/** Lists only symbols for which this deployment has already ingested market events. */
export function getQoinInstruments(
  assetClass: QoinAssetClass,
): Promise<QoinInstrumentCatalogue> {
  return request<QoinInstrumentCatalogue>(
    `/api/v1/instruments?asset_class=${assetClass}`,
  );
}

export type QoinMarketEventType =
  | "trade"
  | "book"
  | "funding"
  | "open_interest"
  | "liquidation"
  | "basis";

export type QoinMarketEvent = {
  event_id: string;
  venue: string;
  symbol: string;
  asset_class: QoinAssetClass;
  event_type: QoinMarketEventType;
  ts_ms: number;
  price?: number | null;
  size?: number | null;
  side?: "buy" | "sell" | "unknown" | null;
  bid?: number | null;
  ask?: number | null;
  bid_size?: number | null;
  ask_size?: number | null;
  funding_rate?: number | null;
  open_interest?: number | null;
  liquidation_side?: "long" | "short" | null;
  basis?: number | null;
};

export type QoinIngestResult = {
  status: "accepted" | "duplicate_ignored";
  event_id: string;
};

/** Submit one normalized, asset-class-tagged market event to the research ingestion path. */
export function ingestQoinMarketEvent(
  event: QoinMarketEvent,
): Promise<QoinIngestResult> {
  return request<QoinIngestResult>("/api/v1/events", {
    method: "POST",
    body: JSON.stringify(event),
  });
}

