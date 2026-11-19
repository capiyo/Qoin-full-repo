# Qoin architecture

## Ownership
- **Axum (Rust)** is the sole authority for event validation, bounded state estimation, seven-force feature construction, vector/attention decisions, research replay, risk validation, and MongoDB writes.
- **Drogon (C++)** is the HTTP gateway. It forwards event, state, vector, strategy, and order requests to Axum; it contains no independent model, risk, or order logic.
- **MongoDB** is the durable source of truth for raw market events, recovered state snapshots, vector decisions, the V2 append-only set/relation ledger, and legacy edge-map settings.

## Authoritative seven-force decision engine

The crate `backend/rust-axum/crates/qoin-vector-engine` is the single decision implementation used by both live market-event ingestion and historical backtesting. The legacy `edge_strategy` module and `edge_strategy` collection remain available through the old strategy inspection/update endpoints for compatibility, but they do **not** calculate current live decisions or the current backtest decisions.

The engine consumes seven normalized forces in a stable order:

1. `order_flow_imbalance` — signed trade flow divided by observed trade size.
2. `liquidity_imbalance` — signed best-bid versus ask-depth imbalance; spread is handled by a separate freshness/quality gate.
3. `volatility` — EWMA absolute log return normalized by 1%.
4. `funding_pressure` — funding rate multiplied by 100.
5. `open_interest_change` — relative change between consecutive open-interest observations, multiplied by 100; the absolute open-interest level is not treated as directional.
6. `liquidation_pressure` — exponentially decayed signed liquidation count (60-second half-life), with long liquidations negative and short liquidations positive.
7. `basis` — mark-to-last-trade basis in bps normalized by 25; this is available only where a provider supplies mark price and a trade reference.

Trade-flow imbalance and signed liquidation pressure decay with a 60-second half-life; OFI is forced to zero after 60 seconds without a trade. Relative OI-change impulses decay with a five-minute half-life. Book depth imbalance is kept separate from trade flow. The engine clips inputs, constructs a seven-by-seven deterministic self-attention matrix, applies deterministic top-k force selection, calculates the directional score and alignment, and records a full feature/contribution trace. The OFI × absolute OI-change interaction is reported separately so the score calculation is inspectable. This is a hand-specified research baseline, not a trained model.

A decision is forced to WAIT if the spread is absent, exceeds the configured maximum, or the quote used to calculate it is older than the configured maximum age. Defaults are 25 bps and 30 seconds. These are configurable at service startup with `QOIN_MAX_SPREAD_BPS` and `QOIN_MAX_SPREAD_AGE_MS`. Other baseline parameters include `QOIN_SIGNAL_THRESHOLD`, `QOIN_INPUT_CLIP`, `QOIN_TOP_K`, `QOIN_MIN_ALIGNMENT`, `QOIN_MAX_ANGLE_RADIANS`, and `QOIN_CONFIG_VERSION`.

The engine returns a research signal only. It does not size a position or submit an order. Portfolio/risk policy and verified broker adapters remain separate authorities.

## Live event and decision flow
1. A market-data adapter submits an event to `POST /api/v1/events` on the local Drogon gateway.
2. Axum validates and deduplicates the event by venue + symbol + event_id.
3. The immutable source event is persisted to `market_events`; the per-symbol rolling state is updated.
4. Trade prices update the trade reference and EWMA volatility. Mark-price events update mark price, funding and mark-to-trade basis without overwriting the last trade. Consecutive open-interest observations produce a relative OI change. Book events update spread and quote timestamp.
5. Axum maps that causal state into the seven-force `ForceObservation`, calls `qoin_vector_engine::decide`, persists the exact engine output in `trade_decisions`, and broadcasts the same decision to web/mobile clients.
6. Consumers query `/api/v1/state?symbol=BTCUSDT`, `/api/v1/vector?symbol=BTCUSDT`, and `/api/v1/vector/history?symbol=BTCUSDT`.
7. Historical replay calls the same vector-engine crate with event-time state reconstruction and the same quote freshness gate. It does not call the legacy edge strategy.

On startup, Axum rebuilds rolling symbol state by replaying the immutable market-event log in deterministic event-time order. The recovery counters are exposed by the health endpoint.

## Explicit research limitations
- This is a deterministic, hand-specified research baseline, not evidence of an exploitable or profitable edge.
- Feature transforms are fixed scale mappings, not robust past-only rolling normalization or walk-forward calibration.
- OI change is zero until two valid OI observations are available.
- Basis is zero and marked unavailable unless both a trade reference and a mark-price observation are available. The current Binance mark-price stream provides this; other venues may not.
- A signal is not an order. Live order submission remains independently gated and the broker registry must contain a verified provider adapter before any account can execute.
- Historical replay uses only persisted events and cannot reconstruct missing provider history. Execution fills remain a simulation with explicit latency, liquidity, fee, slippage and impact assumptions.

## Version 2 direction: sets and relation ledger

Version 2 has a durable append-only observation/relation ledger backed by MongoDB. It derives typed event sets (trades, buy/sell flow, books, funding, open interest and liquidations), counting/observed trade-size measures, and explicit temporal-precedence edges from persisted raw events. Relations use a fixed 60-second window and are not causal claims. The V2 ledger is an analytical projection; it does not currently replace the seven-force decision engine or grant execution authority.

## Client platforms

Qoin ships three clients over one domain model and one backend decision authority:

- **Web:** React + TypeScript + Vite.
- **Mobile:** Expo / React Native.
- **Desktop:** Tauri 2 native shell that packages the shared React/Vite client and switches to a wide workstation layout when running in the native webview.

All clients use the shared `@qoin/core` API, authentication, profile and archive contracts. No client computes trading decisions. The mechanics views render the exact persisted seven-force decision, seven-by-seven attention matrix, feature trace, quote-quality gates and historical decision trajectory. Desktop does not contain provider credentials, MongoDB access, exchange keys, or native order execution.

Desktop installers are built on their native operating systems through `.github/workflows/desktop.yml` (Windows, macOS and Linux). Build locally with `pnpm dev:desktop` or `pnpm build:desktop`. The Tauri shell starts the same web app and talks to the existing local Drogon gateway; start the Rust engine and gateway separately. Packaging a desktop binary does not change the execution policy.
