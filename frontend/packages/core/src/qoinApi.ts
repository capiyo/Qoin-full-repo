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
  return request<QoinSignal>("/v1/signal", {
    method: "POST",
    body: JSON.stringify(input),
  });
}
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

