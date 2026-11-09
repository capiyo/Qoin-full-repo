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

declare const process: { env: { EXPO_PUBLIC_QOIN_API_URL?: string } } | undefined;

function getBaseUrl(): string {
  // Android emulator: http://10.0.2.2:8080; iOS simulator: http://127.0.0.1:8080.
  // Physical devices need a reachable host configured for the development environment.
  const runtime = globalThis as typeof globalThis & {
    location?: { origin: string };
    document?: unknown;
  };
  const configured = typeof process !== "undefined" ? process.env.EXPO_PUBLIC_QOIN_API_URL : undefined;
  const sameOrigin = typeof runtime.document !== "undefined" ? runtime.location?.origin : undefined;
  return (configured || sameOrigin || "http://127.0.0.1:8080").replace(/\/$/, "");
}

export async function qoinRequest<T>(path: string, init?: RequestInit & { token?: string }): Promise<T> {
  const { token, ...requestInit } = init ?? {};
  const response = await fetch(`${getBaseUrl()}${path}`, {
    ...requestInit,
    headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}), ...init?.headers },
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    throw new Error(`Qoin API ${response.status}: ${detail || response.statusText}`);
  }
  return response.json() as Promise<T>;
}

export function getQoinHealth(): Promise<QoinHealth> {
  return qoinRequest<QoinHealth>("/health");
}

/** Returns a research signal only. This client never submits exchange orders. */
export function evaluateQoinSignal(input: QoinSignalRequest): Promise<QoinSignal> {
  return qoinRequest<QoinSignal>("/v1/signal", {
    method: "POST",
    body: JSON.stringify(input),
  });
}
