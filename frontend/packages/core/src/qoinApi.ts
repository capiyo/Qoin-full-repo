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


export type QoinVectorDecision = {
  symbol: string;
  strategy_key?: string;
  strategy_version?: string;
  features?: Record<string, number>;
  attention?: number[][];
  attended_features?: number[];
  decision_vector?: [number, number];
  magnitude?: number;
  angle_radians?: number;
  angular_threshold_radians?: number;
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
  const runtime = globalThis as typeof globalThis & { process?: { env?: Record<string, string | undefined> } };
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

export type QoinStrategy = {
  strategy_key: string; version: string; status: string;
  bullish_weights: Record<string, number>; bearish_weights: Record<string, number>;
  angular_threshold_degrees: number; magnitude_threshold: number; max_spread_bps: number;
  attention_scale: number; feature_scales: Record<string, number>; updated_at_ms: number;
};
export function getQoinStrategy(): Promise<QoinStrategy> {
  return request<QoinStrategy>("/api/v1/strategy");
}
