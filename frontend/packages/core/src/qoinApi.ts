declare const process: { env?: { EXPO_PUBLIC_QOIN_API_URL?: string; EXPO_PUBLIC_QOIN_WS_URL?: string } };

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


export type QoinFeatureTrace = {
  name: string;
  raw_value: number;
  transform: string;
  scale: number;
  normalized_value: number;
  clamped_value: number;
  bullish_weight: number;
  bearish_weight: number;
  bullish_weighted_term: number;
  bearish_weighted_term: number;
};
export type QoinDecisionDiagnostics = {
  spread_present: boolean;
  spread_within_limit: boolean;
  event_intensity_positive: boolean;
  magnitude_passed: boolean;
  buy_angle_passed: boolean;
  sell_angle_passed: boolean;
  bullish_exceeds_bearish: boolean;
  bearish_exceeds_bullish: boolean;
  spread_limit_bps: number;
  magnitude_limit: number;
  angle_limit_degrees: number;
};
export type QoinStrategySnapshot = {
  strategy_key: string;
  version: string;
  status: string;
  bullish_weights: Record<string, number>;
  bearish_weights: Record<string, number>;
  angular_threshold_degrees: number;
  magnitude_threshold: number;
  max_spread_bps: number;
  attention_scale: number;
  feature_scales: Record<string, number>;
  updated_at_ms: number;
};
export type QoinDecisionInputEvent = {
  event_id: string;
  venue: string;
  event_type: string;
  event_ts_ms: number;
  state_event_count: number;
  state_last_event_ts_ms: number | null;
};
export type QoinVectorDecision = {
  symbol: string;
  input_event?: QoinDecisionInputEvent | null;
  liquidation_side?: string | null;
  strategy_key?: string;
  strategy_version?: string;
  raw_inputs?: Record<string, number | null>;
  feature_trace?: QoinFeatureTrace[];
  strategy_parameters?: QoinStrategySnapshot;
  features?: Record<string, number>;
  attention?: number[][];
  attended_features?: number[];
  decision_vector?: [number, number];
  magnitude?: number;
  angle_radians?: number;
  angle_degrees?: number;
  angular_threshold_radians?: number;
  diagnostics?: QoinDecisionDiagnostics;
  action?: "buy" | "sell" | "wait" | string;
  reason?: string;
  research_only?: boolean;
};

export const QOIN_FEATURE_NAMES = [
  "order_flow_imbalance",
  "funding_pressure",
  "liquidity_stress",
  "volatility_shock",
  "event_intensity",
  "liquidation_pressure",
] as const;

/** Fetches the latest persisted engine vector for a symbol; never places orders. */
export function getInstrumentVector(symbol: string): Promise<QoinVectorDecision> {
  return request<QoinVectorDecision>(`/api/v1/vector?symbol=${encodeURIComponent(symbol)}`);
}

export type QoinSignalRequest = {
  observation: MarketForceObservation;
  has_open_position: boolean;
};

export type QoinHealth = {
  service?: string;
  status: "ok" | "degraded" | string;
  mongodb?: boolean;
  strategy_version?: string;
  jepa_research_inference?: string;
  market_streams?: { binance_usdm_enabled?: boolean; finnhub_key_configured?: boolean };
  live_order_submission?: string;
  live_trading?: boolean | string;
};

function getBaseUrl(): string {
  // Android emulator and physical devices must use a gateway host reachable from that device.
  // A blank Expo URL falls back to loopback for local simulators only.
  const runtime = globalThis as typeof globalThis & {
    process?: { env?: Record<string, string | undefined> };
    __QOIN_API_URL?: string;
    __QOIN_WS_URL?: string;
  };
  const env = runtime.process?.env;
  const location = (globalThis as typeof globalThis & { location?: { protocol?: string; host?: string } }).location;
  const expoConfigured = typeof process !== "undefined" ? process.env?.EXPO_PUBLIC_QOIN_API_URL : undefined;
  const configured = expoConfigured || runtime.__QOIN_API_URL || env?.EXPO_PUBLIC_QOIN_API_URL || env?.VITE_QOIN_API_URL || env?.NEXT_PUBLIC_QOIN_API_URL;
  if (configured) return configured.replace(/\/$/, "");
  if (location?.host) return (location.protocol === "https:" ? "https://" : "http://") + location.host;
  return "http://127.0.0.1:8080";
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${getBaseUrl()}${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    try {
      const payload = JSON.parse(detail) as { status?: string; items?: unknown[] };
      if (typeof payload.status === "string" && Array.isArray(payload.items)) return payload as T;
    } catch { /* preserve the HTTP error below */ }
    throw new Error(`Qoin API ${response.status}: ${detail || response.statusText}`);
  }
  return response.json() as Promise<T>;
}

export function getQoinHealth(): Promise<QoinHealth> {
  return request<QoinHealth>("/health");
}

/** Returns a research signal only. This client never submits exchange orders. */
export function evaluateQoinSignal(input: QoinSignalRequest): Promise<QoinSignal> {
  return request<QoinSignal>("/v1/signal", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export type QoinMarketState = {
  symbol: string; event_count: number; last_event_ts_ms?: number | null; last_price?: number | null;
  spread_bps?: number | null; signed_flow: number; order_flow_imbalance: number; total_trade_size: number;
  event_intensity: number; ewma_abs_return: number; funding_rate?: number | null;
  open_interest?: number | null; liquidation_events: number; last_liquidation_side?: string | null; state_quality: number;
};
export type QoinStreamUpdate = {
  type: string; symbol?: string;
  event?: { event_id?: string; venue?: string; symbol?: string; ts_ms?: number; event_type?: string; price?: number | null; size?: number | null; side?: string | null;
    bid?: number | null; ask?: number | null; bid_size?: number | null; ask_size?: number | null; funding_rate?: number | null;
    open_interest?: number | null; liquidation_side?: string | null };
  state?: QoinMarketState; decision?: QoinVectorDecision; server_ts_ms?: number;
  missed_updates?: number; note?: string; subscribed_symbols?: string[];
};
export type QoinNewsItem = {
  id: string | number; headline: string; summary: string; source: string; url: string;
  image?: string; published_at_ms: number; related?: string; category?: string;
};
export type QoinNewsResponse = { source: string; status: string; items: QoinNewsItem[]; message?: string };
export type QoinInstrument = {
  symbol: string; engine_symbol: string; name: string;
  asset_class: "Crypto" | "Equities" | "FX" | "Indices" | "Commodities";
  venue: string; provider: string; provider_status?: string; description: string; quote_currency?: string;
};
export type QoinVectorHistory = {
  symbol: string; strategy_version?: string; decision_vector: [number, number];
  magnitude: number; angle_radians: number; action: string; stored_at?: string;
};
function getWebSocketBaseUrl(): string {
  const runtime = globalThis as typeof globalThis & { process?: { env?: Record<string, string | undefined> }; __QOIN_WS_URL?: string };
  const env = runtime.process?.env;
  const expoConfigured = typeof process !== "undefined" ? process.env?.EXPO_PUBLIC_QOIN_WS_URL : undefined;
  const configured = expoConfigured || runtime.__QOIN_WS_URL || env?.EXPO_PUBLIC_QOIN_WS_URL || env?.VITE_QOIN_WS_URL;
  if (configured) return configured.replace(/\/$/, "");
  return getBaseUrl().replace(/^https:/, "wss:").replace(/^http:/, "ws:");
}
/** Reconnecting read-only stream through the Drogon gateway. Returns an unsubscribe function. */
export function subscribeQoinStream(
  symbols: string[],
  onUpdate: (update: QoinStreamUpdate) => void,
  onStatus?: (status: "connecting" | "connected" | "reconnecting" | "disconnected" | "error") => void,
): () => void {
  let stopped = false;
  let socket: WebSocket | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let retryMs = 1000;
  const normalized = [...new Set(symbols.map((value) => value.trim().toUpperCase()).filter(Boolean))];
  const scheduleReconnect = () => {
    if (stopped || retryTimer) return;
    retryTimer = setTimeout(() => { retryTimer = null; connect(); }, retryMs);
    retryMs = Math.min(30000, retryMs * 2);
  };
  const connect = () => {
    if (stopped) return;
    onStatus?.(retryMs === 1000 ? "connecting" : "reconnecting");
    const base = getWebSocketBaseUrl() + "/api/v1/stream";
    const url = base + (normalized.length ? "?symbols=" + encodeURIComponent(normalized.join(",")) : "");
    try { socket = new WebSocket(url); }
    catch { onStatus?.("error"); scheduleReconnect(); return; }
    socket.onopen = () => {
      retryMs = 1000;
      onStatus?.("connected");
      if (normalized.length) socket?.send(JSON.stringify({ type: "subscribe", symbols: normalized }));
    };
    socket.onmessage = (message) => {
      try { onUpdate(JSON.parse(String(message.data)) as QoinStreamUpdate); }
      catch { onStatus?.("error"); }
    };
    socket.onerror = () => onStatus?.("error");
    socket.onclose = () => {
      if (!stopped) { onStatus?.("reconnecting"); scheduleReconnect(); }
    };
  };
  connect();
  return () => {
    stopped = true;
    if (retryTimer) clearTimeout(retryTimer);
    socket?.close();
    socket = null;
    onStatus?.("disconnected");
  };
}
export function getQoinNews(category = "general"): Promise<QoinNewsResponse> {
  return request<QoinNewsResponse>("/api/v1/news?category=" + encodeURIComponent(category));
}
export function getQoinInstruments(): Promise<{ items: QoinInstrument[]; source: string }> {
  return request<{ items: QoinInstrument[]; source: string }>("/api/v1/instruments");
}
export function getInstrumentVectorHistory(symbol: string, limit = 120): Promise<{ symbol: string; items: QoinVectorHistory[] }> {
  return request<{ symbol: string; items: QoinVectorHistory[] }>("/api/v1/vector/history?symbol=" + encodeURIComponent(symbol) + "&limit=" + limit);
}

export type QoinMacroEvent = {
  country: string; event: string; impact: string; actual: number | string | null;
  previous: number | string | null; estimate: number | string | null; time: string; unit: string;
};
export type QoinCalendarResponse = { source: string; status: string; from?: string; to?: string; items: QoinMacroEvent[]; message?: string };
export function getQoinCalendar(): Promise<QoinCalendarResponse> {
  return request<QoinCalendarResponse>("/api/v1/calendar");
}

export type QoinMarketEvent = {
  event_id: string; venue: string; symbol: string; event_type: string; ts_ms: number;
  price?: number | null; size?: number | null; side?: string | null;
  bid?: number | null; ask?: number | null; bid_size?: number | null; ask_size?: number | null;
  funding_rate?: number | null; open_interest?: number | null; liquidation_side?: string | null;
};
export function getQoinEvents(symbol: string, limit = 200): Promise<{ symbol: string; items: QoinMarketEvent[]; count: number }> {
  return request<{ symbol: string; items: QoinMarketEvent[]; count: number }>("/api/v1/events?symbol=" + encodeURIComponent(symbol) + "&limit=" + limit);
}


/** Public, read-only metadata for an independently versioned research algorithm.
 * This is a data contract only; it does not imply that paid access or switching
 * endpoints are available until server-side identity and grants are integrated.
 */
export type QoinAlgorithmStatus = "active" | "preview" | "deprecated" | "disabled";
export type QoinAlgorithmManifest = {
  algorithm_id: string;
  display_name: string;
  description: string;
  version: string;
  status: QoinAlgorithmStatus;
  supported_asset_classes: string[];
  supported_market_data_providers: string[];
  supported_venues: string[];
  supported_instrument_groups: string[];
  supported_timeframes: string[];
  required_features: string[];
  optional_features: string[];
  risk_profile: string;
  execution_mode: "research_only";
  evaluation_status: "not_independently_validated" | "in_sample" | "out_of_sample" | "walk_forward" | "independently_validated";
};

export type QoinMarketAccessTier = {
  tier_id: string;
  display_name: string;
  status: "preview" | "active" | "disabled";
  market_data_providers: string[];
  venues: string[];
  asset_classes: string[];
  instrument_groups: string[];
  max_active_instruments: number;
  historical_depth_days: number;
  supported_timeframes: string[];
  max_stream_subscriptions: number;
  execution_brokers: string[];
  notes: string;
};

export type QoinAlgorithmCatalogue = {
  catalogue_version: number;
  market_access_tiers: QoinMarketAccessTier[];
  market_access_tiers_enabled: boolean;
  algorithms: QoinAlgorithmManifest[];
  selection_enabled: boolean;
  entitlements_enabled: boolean;
  note?: string;
};
export type QoinAlgorithmEntitlement = {
  algorithm_id: string;
  allowed: boolean;
  grant_status: "pending" | "active" | "expired" | "refunded" | "revoked" | "chargeback" | "none";
  allowed_capabilities: string[];
  missing_capabilities: string[];
  effective_at_ms?: number;
  expires_at_ms?: number | null;
};
export type QoinAlgorithmSelection = {
  algorithm_id: string;
  version: string;
  selected_at_ms: number;
  config_fingerprint: string;
};

export type QoinStrategy = {
  strategy_key: string; version: string; status: string;
  bullish_weights: Record<string, number>; bearish_weights: Record<string, number>;
  angular_threshold_degrees: number; magnitude_threshold: number; max_spread_bps: number;
  attention_scale: number; feature_scales: Record<string, number>; updated_at_ms: number;
};
export function getQoinStrategy(): Promise<QoinStrategy> {
  return request<QoinStrategy>("/api/v1/strategy");
}


export type QoinBacktestFill = {
  ts_ms: number; event_id: string; action: string; price: number; quantity: number;
  fee: number; slippage_cost: number; realized_pnl: number; equity_after: number; reason: string;
};
export type QoinBacktestResult = {
  symbol: string; venue: string; strategy_key: string; strategy_version: string; from_ts_ms: number; to_ts_ms: number;
  events_replayed: number; decisions: number; buy_decisions: number; sell_decisions: number; wait_decisions: number;
  initial_capital: number; final_equity: number; net_pnl: number; net_return_pct: number; max_drawdown_pct: number;
  fees_paid: number; slippage_paid: number; fee_bps: number; slippage_bps: number;
  fills: QoinBacktestFill[]; last_decision?: QoinVectorDecision | null; assumptions: string[];
};
export type QoinBacktestRequest = {
  symbol: string; venue?: string; from_ts_ms?: number; to_ts_ms?: number; limit?: number;
  initial_capital?: number; fee_bps?: number; slippage_bps?: number;
};
/** Replays only persisted, time-ordered raw observations. No orders are submitted. */
export function runQoinBacktest(input: QoinBacktestRequest): Promise<QoinBacktestResult> {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(input)) if (value !== undefined) query.set(key, String(value));
  return request<QoinBacktestResult>("/api/v1/backtest?" + query.toString());
}

/** Read-only algorithm and preview market coverage catalogue. */
export function getQoinAlgorithmCatalogue(): Promise<QoinAlgorithmCatalogue> {
  return request<QoinAlgorithmCatalogue>("/api/v1/algorithms");
}
