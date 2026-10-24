# Qoin — Market-Force Trading Research Monorepo

A multi-client workspace for researching a present-centered crypto market-force algorithm. React web and React Native share API contracts; Rust/Axum is authoritative for signal computation. The current engine is a research baseline, not a validated trading strategy.

## Monorepo layout

- `apps/react-web` — React + TypeScript + Vite research dashboard.
- `apps/web` — existing Next.js React dashboard.
- `apps/mobile` — Expo / React Native app.
- `packages/core` — shared UI tokens, types, mock market data, and Qoin API client.
- `crates/qoin-vector-engine` — deterministic Rust signal engine.
- `services/axum-api` — Axum health and research-signal API.
- `services/` — existing market-data / gateway service foundation.
- `docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.md` and PDF — mathematical and philosophical specification.
- `docs/MONOREPO_ARCHITECTURE.md` — component responsibilities and next engineering gates.

## Requirements

- Node.js and pnpm 9
- Rust stable toolchain (Cargo)

## Start the React web dashboard

```sh
pnpm install
pnpm dev:react-web
```

Vite serves the UI at `http://localhost:5173`. By default, the API client expects the research API at `http://127.0.0.1:8080`. Set `NEXT_PUBLIC_QOIN_API_URL` for the web client if the API is hosted elsewhere.

## Start the React Native app

```sh
pnpm dev:mobile
```

For a physical device or emulator, configure `EXPO_PUBLIC_QOIN_API_URL` to a reachable API host. Android emulator localhost normally maps to `10.0.2.2`; a physical device needs the development machine's LAN address. The API currently binds to loopback for safety, so adjust its bind address deliberately for device testing.

## Start and test the Rust research API

```sh
cargo test --workspace
pnpm dev:api
```

- `GET /health` — service health and mode.
- `POST /v1/signal` — evaluates seven normalized market-force inputs and returns a versioned research signal.

Example request:

```json
{
  "observation": {
    "ofi": 1.2,
    "liquidity": 0.4,
    "volatility": 0.3,
    "funding": -0.2,
    "open_interest_change": 0.6,
    "liquidation_pressure": 0.1,
    "basis": 0.0
  },
  "has_open_position": false
}
```

## Important status and safety limits

- The current Rust coefficients are explicit baseline values, not trained weights or evidence of edge.
- Inputs to the signal API are expected to be causally normalized upstream; production ingestion, past-only rolling normalization, persistence, Pareto selection, and event replay remain work to complete.
- A signal is not an order. The API does not call an exchange adapter or submit orders.
- `LIVE_TRADING=false` is the required default. No real orders are sent by this API; do not enable real-money execution without explicit human approval.
- Backtesting, paper/shadow validation, testnet, realistic costs, and profitability remain unverified. See `docs/PROGRESS.md`, `docs/RISK_POLICY.md`, and `docs/MONOREPO_ARCHITECTURE.md`.

## Algorithm paper

- [Detailed PDF: observations, mathematical specification, and philosophy](docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.pdf)
- [Editable Markdown source](docs/QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.md)
