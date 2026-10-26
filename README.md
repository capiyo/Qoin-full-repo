# Qoin — Market-Force Trading Research Monorepo

A multi-client workspace for researching a present-centered crypto market-force algorithm. React web and React Native share API contracts. **Axum (Rust) is the authoritative ingestion, state, strategy-map, and vector-decision engine; Drogon (C++) is the HTTP gateway; MongoDB is the durable source of truth.** The current strategy is a research baseline, not a validated or proven profitable edge.

## Versioned architecture tracks

- **[Version 1 — Vector baseline](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)** — preserves the existing architecture as the comparison baseline.
- **[Version 2 — JEPA world model](https://github.com/capiyo/Qoin-full-repo/tree/version-2-jepa-world-model)** — retains Version 1 and adds a research-only JEPA-inspired latent-state predictor.
- **[Version map](https://github.com/capiyo/Qoin-full-repo/blob/main/docs/versions/README.md)** — architecture boundaries and evaluation rules.

Version 2 predictions do not replace Axum's decision authority or bypass risk controls. Both versions remain paper-only until all risk-policy gates are independently verified and live trading is explicitly approved in writing.


## Branch layout

The repository is now explicitly shelved into focused branches so frontend and backend work are not mixed together:

- **[`frontend`](https://github.com/capiyo/Qoin-full-repo/tree/frontend)** — React/Vite web, existing Next.js React web, React Native/Expo mobile, shared TypeScript core, and API contracts.
- **[`backend`](https://github.com/capiyo/Qoin-full-repo/tree/backend)** — Axum/Rust engine, Drogon/C++ gateway, MongoDB configuration and persistence, vector engine, research, backend tests, and service CI.
- **[`main`](https://github.com/capiyo/Qoin-full-repo/tree/main)** — integration snapshot containing both sides together. Keep feature work on the focused branches and bring changes into the integration branch deliberately.

The service boundary remains: clients call Drogon; Drogon forwards to Axum; Axum owns strategy/state/vector decisions and MongoDB persistence. Neither frontend clients nor Drogon should duplicate strategy logic.

## Monorepo layout

- apps/react-web — React + TypeScript + Vite research dashboard.
- apps/web — existing Next.js React dashboard.
- apps/mobile — Expo / React Native app.
- packages/core — shared UI tokens, types, mock market data, and Qoin API client.
- crates/qoin-vector-engine — deterministic Rust baseline signal engine.
- services/axum — authoritative event ingestion, state estimation, vector/attention decisions, MongoDB persistence, and versioned edge strategy map.
- services/drogon — C++ HTTP gateway that forwards to Axum; it must not duplicate model logic.
- services/axum-api — separate research-signal API retained for compatibility; the integrated market-event flow uses services/axum.
- docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.md and PDF — mathematical and philosophical specification.
- docs/ARCHITECTURE.md and docs/MONOREPO_ARCHITECTURE.md — service responsibilities and engineering gates.

## Requirements

- Node.js and pnpm 9
- Rust stable toolchain (Cargo)
- MongoDB 7 (or compatible)
- C++17, CMake, and the Drogon development package to build the gateway

## Start services locally

    cp .env.example .env
    docker compose up -d mongodb
    cargo run --manifest-path services/axum/Cargo.toml

In a second terminal, build the C++ gateway:

    cmake -S services/drogon -B build/drogon
    cmake --build build/drogon
    ./build/drogon/qoin-gateway

Defaults bind both HTTP services to loopback: Drogon at 127.0.0.1:8080, Axum at 127.0.0.1:8081. Do not expose Axum directly to the public internet. Set QOIN_MONGODB_URI, QOIN_MONGODB_DATABASE, QOIN_AXUM_ADDR, QOIN_AXUM_URL, and QOIN_DROGON_PORT as needed.

## API flow

- GET /health — Drogon forwards Axum/MongoDB health and the active strategy version.
- POST /api/v1/events — ingest a normalized market event through Drogon.
- GET /api/v1/state?symbol=BTCUSDT — latest state.
- GET /api/v1/vector?symbol=BTCUSDT — latest stored decision.
- GET /api/v1/strategy — inspect the active edge strategy map.
- PUT /api/v1/strategy — validate, persist, and activate a complete strategy map through Axum.

On first startup, Axum seeds the MongoDB edge_strategy collection with an explicit versioned baseline if no active map exists. The map contains named bullish/bearish feature weights, feature scales, attention scale, angular threshold, magnitude threshold, and the spread gate. Strategy updates are persisted before activation, and each decision records the strategy version. Promote new maps only after reviewed chronological replay; backtest outcomes never automatically rewrite the active strategy.

## Tests

    cargo test --workspace

The integrated flow-engine CI also runs cargo test --manifest-path services/axum/Cargo.toml. Drogon compilation requires its native system package and is verified separately from the Rust tests.

## Important status and safety limits

- The current coefficients are explicit hand-specified baseline values, not trained weights or evidence of edge.
- Input scaling and current state estimation remain early-stage; robust past-only rolling normalization, complete event replay/recovery, and validated market-data adapters remain to be completed.
- A signal is not an order. Live order submission is not connected to the market-event flow. QOIN_TRADING_MODE=disabled is the default.
- Backtesting, paper/shadow validation, testnet, realistic costs, and profitability remain unverified. No real-money trading should be enabled without explicit human approval and independent risk controls.

## Algorithm paper

- Detailed PDF: docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.pdf
- Editable Markdown source: docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.md
