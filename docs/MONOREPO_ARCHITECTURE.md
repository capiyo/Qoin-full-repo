# Qoin monorepo architecture

## Applications and services

- `apps/web`: React web dashboard (currently Next.js-based in this repository).
- `apps/mobile`: Expo / React Native mobile dashboard.
- `packages/core`: shared UI tokens, types and data contracts.
- `crates/qoin-vector-engine`: deterministic Rust research-stage vector signal engine shared by backend workflows.
- `services/axum-api`: Rust/Axum API exposing health and research-only signal evaluation.
- Existing C++/Drogon gateway and research documentation remain separate from decision logic.

## Contract

The web and mobile clients should consume the same API contracts and must not duplicate signal mathematics. The Axum service is authoritative for signal calculation. The gateway may proxy requests but must not calculate signals.

## Current engine status

The vector engine is a deliberately small deterministic baseline with explicit coefficients and a config version. It is not trained, calibrated, or evidence of a profitable strategy. Inputs must be causal and normalized with past-only data by the ingestion/state-estimation layer. The current endpoint accepts normalized observations for research and returns a signal only.

## Required before paper/live trading

1. Add versioned runtime configuration and persist every observation, signal, config version, and outcome.
2. Implement causal normalization, robust filtering, Pareto/top-k selection, angle/magnitude/stability checks, and regime-aware validation.
3. Build an event-driven replay/backtester that invokes the exact production signal crate; include fees, spread, slippage, funding, delayed fills, and stressed costs.
4. Add an independent risk gate, position/order lifecycle, idempotency, reconciliation, kill switch, exchange-side protective stops, and crash recovery.
5. Run unit, property, leakage, walk-forward, purged/embargoed CV, paper and shadow-mode tests.
6. Do not enable real-money execution without explicit human approval. `LIVE_TRADING=false` is the required default; no real orders are sent by this service.

## Run locally

From repository root:

```sh
cargo test --workspace
cargo run -p qoin-axum-api
```

Health check: `GET http://127.0.0.1:8080/health`

Signal endpoint: `POST http://127.0.0.1:8080/v1/signal` with JSON fields `observation` and `has_open_position`. All seven observation fields are required.
