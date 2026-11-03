declare const __QOIN_VITE_API_URL__: string | undefined;
declare const process: { env: { EXPO_PUBLIC_QOIN_API_URL?: string } };

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

export type QoinPortfolio = {
  symbols: string[];
  updated_at: string | null;
};

function getBaseUrl(): string {
  // Vite injects the web URL; Expo statically inlines its public mobile URL.
  const viteConfigured = typeof __QOIN_VITE_API_URL__ !== "undefined" ? __QOIN_VITE_API_URL__ : undefined;
  const expoConfigured = typeof process !== "undefined" ? process.env.EXPO_PUBLIC_QOIN_API_URL : undefined;
  return (viteConfigured || expoConfigured || "http://127.0.0.1:8080").replace(/\/$/, "");
}

async function request<T>(path: string, init?: RequestInit, idToken?: string): Promise<T> {
  const headers = new Headers(init?.headers);
  headers.set("content-type", "application/json");
  if (idToken) headers.set("authorization", `Bearer ${idToken}`);
  const response = await fetch(`${getBaseUrl()}${path}`, {
    ...init,
    headers,
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


/** Reads the authenticated user's server-side portfolio. The UID is derived from the verified token. */
export function getQoinPortfolio(idToken: string): Promise<QoinPortfolio> {
  if (!idToken.trim()) return Promise.reject(new Error("A Firebase ID token is required"));
  return request<QoinPortfolio>("/v1/portfolio", undefined, idToken);
}

/** Replaces the authenticated user's selected instrument basket. */
export function saveQoinPortfolio(symbols: string[], idToken: string): Promise<QoinPortfolio> {
  if (!idToken.trim()) return Promise.reject(new Error("A Firebase ID token is required"));
  return request<QoinPortfolio>("/v1/portfolio", {
    method: "PUT",
    body: JSON.stringify({ symbols }),
  }, idToken);
}
