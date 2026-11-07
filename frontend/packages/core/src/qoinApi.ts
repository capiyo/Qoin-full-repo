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


export type OperatingCycleConfig = {
  observation_minutes: number;
  weight_evaluation_minutes: number;
  session_minutes: number;
  mode: "research" | "paper";
};

export type OperatingCycleSnapshot = {
  config: OperatingCycleConfig;
  phase: "idle" | "stopped" | "observation" | "weight_evaluation" | "paper_session" | "completed" | string;
  active: boolean;
  started_at_ms: number | null;
  elapsed_seconds: number;
  phase_seconds_remaining: number;
  observations: number;
  signal_evaluations: number;
  live_execution_enabled: false;
  weight_policy: string;
  note: string;
};

export function getOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return request<OperatingCycleSnapshot>("/v1/operating-cycle");
}

export function configureOperatingCycle(config: OperatingCycleConfig): Promise<OperatingCycleSnapshot> {
  return request<OperatingCycleSnapshot>("/v1/operating-cycle", {
    method: "PUT",
    body: JSON.stringify(config),
  });
}

export function startOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return request<OperatingCycleSnapshot>("/v1/operating-cycle/start", { method: "POST" });
}

export function stopOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return request<OperatingCycleSnapshot>("/v1/operating-cycle/stop", { method: "POST" });
}
