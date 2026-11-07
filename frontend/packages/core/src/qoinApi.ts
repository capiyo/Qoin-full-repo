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


export type PaperAccountConfig = {
  symbol: string;
  venue: string;
  quote_currency: string;
  initial_cash: number;
  notional_per_trade: number;
  contract_multiplier: number;
  quantity_step: number;
  reinvest_profits: boolean;
  fee_bps: number;
  slippage_bps: number;
};

export type OperatingCycleConfig = {
  observation_minutes: number;
  weight_evaluation_minutes: number;
  session_minutes: number;
  mode: "research" | "paper";
  paper_accounts: PaperAccountConfig[];
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
  mean_absolute_score: number | null;
  mean_alignment: number | null;
  mean_feature_values: Record<string, number>;
  mean_weight_contributions: Record<string, [number, number]>;
  mean_attention: number[][];
  buy_signals: number;
  sell_signals: number;
  close_signals: number;
  wait_signals: number;
  live_execution_enabled: false;
  weight_policy: string;
  note: string;
};

export function getOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return flowRequest<OperatingCycleSnapshot>("/internal/operating-cycle");
}

export function configureOperatingCycle(config: OperatingCycleConfig): Promise<OperatingCycleSnapshot> {
  return flowRequest<OperatingCycleSnapshot>("/internal/operating-cycle", {
    method: "PUT",
    body: JSON.stringify(config),
  });
}

export function startOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return flowRequest<OperatingCycleSnapshot>("/internal/operating-cycle/start", { method: "POST" });
}

export function stopOperatingCycle(): Promise<OperatingCycleSnapshot> {
  return flowRequest<OperatingCycleSnapshot>("/internal/operating-cycle/stop", { method: "POST" });
}

export type PaperAccountSnapshot = {
  symbol: string; venue: string; quote_currency: string; initial_cash: number; cash: number; equity: number;
  contract_multiplier: number; quantity_step: number;
  net_pnl: number; realized_pnl: number; unrealized_pnl: number; fees_paid: number; max_drawdown_pct: number; events: number; decisions: number;
  buy_signals: number; sell_signals: number; wait_signals: number; entries: number; exits: number;
  wins: number; losses: number; win_rate_pct: number; net_winning_pnl: number; net_losing_pnl: number; profit_factor: number | null;
  skipped_entries: number; has_open_position: boolean;
  open_position: { entry_ts_ms: number; entry_event_id: string; strategy_version: string; quantity: number; contract_multiplier: number; entry_price: number; entry_fee: number; unrealized_pnl: number } | null;
  pending_action: string | null;
};

export type PaperSessionSnapshot = {
  cycle_id: string | null; active: boolean; accounts: PaperAccountSnapshot[];
  live_execution_enabled: false; note: string;
};

export function getPaperSession(): Promise<PaperSessionSnapshot> {
  return flowRequest<PaperSessionSnapshot>("/internal/paper-session");
}

export type PaperMirrorEvent = {
  cycle_id: string; event_id: string; venue: string; symbol: string; quote_currency: string;
  contract_multiplier: number; quantity_step: number; notional_per_trade: number; reinvest_profits: boolean;
  fee_bps: number; slippage_bps: number; ts_ms: number; strategy_version: string;
  bullish_weights: Record<string, number>; bearish_weights: Record<string, number>; action: "buy" | "sell" | "wait"; reason: string; features: Record<string, number>;
  attention: number[][]; attended_features: number[]; decision_vector: [number, number]; magnitude: number;
  angle_radians: number; spread_bps: number | null; execution: string; equity: number; cash: number;
  realized_pnl: number; fees_paid: number; open_position: boolean;
};

export type PaperTradeHistoryItem = {
  cycle_id: string; symbol: string; venue: string; quote_currency: string; strategy_version: string;
  entry_event_id: string; exit_event_id: string; entry_ts_ms: number; exit_ts_ms: number;
  quantity: number; contract_multiplier: number; quantity_step: number; fee_bps: number; slippage_bps: number; reinvest_profits: boolean; entry_price: number; exit_price: number; gross_pnl: number; fees: number; net_pnl: number;
  return_pct: number; exit_reason: string;
};

export type PaperSessionRun = PaperSessionSnapshot & { archived_at?: unknown };

export function getPaperSessionRuns(limit = 20): Promise<{ runs: PaperSessionRun[] }> {
  return flowRequest<{ runs: PaperSessionRun[] }>(`/internal/paper-session/runs?limit=${limit}`);
}

export function getPaperSessionHistory(symbol?: string, limit = 50): Promise<{ decisions: PaperMirrorEvent[]; trades: PaperTradeHistoryItem[] }> {
  const query = new URLSearchParams({ limit: String(limit) });
  if (symbol) query.set("symbol", symbol);
  return flowRequest<{ decisions: PaperMirrorEvent[]; trades: PaperTradeHistoryItem[] }>(`/internal/paper-session/history?${query.toString()}`);
}

export type PaperBacktestRequest = {
  symbol: string;
  venue: string;
  quote_currency: string;
  start_ts_ms: number;
  end_ts_ms: number;
  warmup_hours?: number;
  initial_cash: number;
  notional_per_trade: number;
  reinvest_profits: boolean;
  fee_bps: number;
  slippage_bps: number;
  max_events?: number;
};

export type PaperBacktestTrade = {
  entry_ts_ms: number;
  exit_ts_ms: number | null;
  quantity: number;
  entry_price: number;
  exit_price: number | null;
  gross_pnl: number | null;
  fees: number;
  net_pnl: number | null;
  return_pct: number | null;
  exit_reason: string | null;
};

export type MirrorDecision = {
  sequence: number;
  event_id: string;
  venue: string;
  ts_ms: number;
  strategy_version: string;
  action: "buy" | "sell" | "wait";
  reason: string;
  features: Record<string, number>;
  attention: number[][];
  attended_features: number[];
  decision_vector: [number, number];
  magnitude: number;
  angle_radians: number;
  spread_bps: number | null;
  execution: string;
};

export type PaperBacktestReport = {
  run_id: string;
  symbol: string;
  venue: string;
  quote_currency: string;
  strategy_version: string;
  start_ts_ms: number;
  end_ts_ms: number;
  initial_cash: number;
  reinvest_profits: boolean;
  final_equity: number;
  realized_pnl: number;
  unrealized_pnl: number;
  net_pnl: number;
  return_pct: number;
  max_drawdown_pct: number;
  event_count: number;
  warmup_event_count: number;
  signal_count: number;
  buy_signals: number;
  sell_signals: number;
  wait_signals: number;
  executed_entries: number;
  executed_exits: number;
  skipped_entries: number;
  pending_order: string | null;
  pending_signal_ts_ms: number | null;
  win_rate_pct: number | null;
  profit_factor: number | null;
  fees_paid: number;
  assumptions: string;
  open_position: PaperBacktestTrade | null;
  trades: PaperBacktestTrade[];
  decisions: MirrorDecision[];
};

function getFlowBaseUrl(): string {
  const runtime = globalThis as typeof globalThis & {
    process?: { env?: Record<string, string | undefined> };
  };
  const env = runtime.process?.env;
  const configured = env?.EXPO_PUBLIC_QOIN_FLOW_API_URL ?? env?.VITE_QOIN_FLOW_API_URL;
  return (configured || "http://127.0.0.1:8081").replace(/\/$/, "");
}

async function flowRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${getFlowBaseUrl()}${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    throw new Error(`Qoin flow API ${response.status}: ${detail || response.statusText}`);
  }
  return response.json() as Promise<T>;
}

/** Replays persisted market events chronologically. It only simulates long-only fills. */
export function runPaperBacktest(input: PaperBacktestRequest): Promise<PaperBacktestReport> {
  return flowRequest<PaperBacktestReport>("/internal/backtest", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export type PaperBacktestHistoryItem = Pick<PaperBacktestReport,
  "run_id" | "symbol" | "venue" | "quote_currency" | "strategy_version" | "initial_cash" | "reinvest_profits" | "final_equity" |
  "net_pnl" | "return_pct" | "max_drawdown_pct" | "event_count" | "warmup_event_count" |
  "executed_entries" | "executed_exits" | "fees_paid" | "pending_order" | "pending_signal_ts_ms"> & {
  trades: PaperBacktestTrade[];
  open_position: PaperBacktestTrade | null;
};

export function getPaperBacktestHistory(symbol?: string): Promise<{ runs: PaperBacktestHistoryItem[] }> {
  const query = symbol ? `?symbol=${encodeURIComponent(symbol)}` : "";
  return flowRequest<{ runs: PaperBacktestHistoryItem[] }>(`/internal/backtests${query}`);
}

export function getPaperBacktest(runId: string): Promise<{ summary: PaperBacktestHistoryItem; decisions: MirrorDecision[] }> {
  return flowRequest<{ summary: PaperBacktestHistoryItem; decisions: MirrorDecision[] }>(
    `/internal/backtests/${encodeURIComponent(runId)}`,
  );
}
