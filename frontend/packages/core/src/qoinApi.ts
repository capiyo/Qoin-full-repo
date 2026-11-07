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
  return request<QoinSignal>("/v1/signal", {
    method: "POST",
    body: JSON.stringify(input),
  });
}
