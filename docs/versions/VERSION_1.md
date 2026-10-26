# Version 1 — Explicit vector architecture

**Branch:** `version-1-vector-baseline`  
**Status:** Existing research baseline preserved; not evidence of a profitable edge.

## Scope

Version 1 is the current architecture in this repository. It remains the baseline against which any learned model must be measured.

- **Frontend:** existing web and React Native clients, shared contracts in `packages/core`.
- **Drogon (C++):** HTTP gateway; forwards requests and does not own strategy/model logic.
- **Axum (Rust):** authoritative event validation, state estimation, feature construction, explicit vector/attention decision logic, strategy-map validation, and persistence coordination.
- **Vector engine:** deterministic, explicit-coefficient research baseline.
- **MongoDB:** durable store for market events, state snapshots, decisions, and versioned strategy maps.
- **Research workflow:** chronological replay and evaluation must call the same decision implementation used by the application path.

## Existing flow

1. A normalized market event enters through the local Drogon gateway.
2. Axum validates, deduplicates, and records the event.
3. Axum updates the market-state representation and persists the state snapshot.
4. The active versioned vector strategy evaluates the state and records its decision and strategy version.
5. Clients inspect state and decisions through the API; clients and Drogon never duplicate signal mathematics.

## Decision philosophy

Use explicit, interpretable market dimensions: order-flow imbalance, liquidity/spread, volatility, funding, open-interest change, liquidation pressure, event intensity, and basis where reliable data is available. Evaluate direction, magnitude, persistence, regime, data quality, and estimated execution costs. WAIT is a valid outcome.

The existing weights are hand-specified research values. They are not trained weights, a validated trading edge, or proof of informed/insider activity.

## Known limitations

The repository's root README and `docs/ARCHITECTURE.md` describe the current implementation gaps, including robust past-only normalization, complete event replay/recovery, validated adapters, realistic backtesting, and independent execution risk controls. Do not infer those controls are complete from this version label.

## Safety invariants

- Live order submission stays disabled and disconnected.
- Axum is not exposed directly to the public internet.
- Do not commit exchange credentials.
- All research uses chronological splits and prevents future-data leakage.
- No real-money trading until all `docs/RISK_POLICY.md` gates are implemented and independently verified and the user approves live trading in writing.
