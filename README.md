# Qoin — Multi-Asset Quantitative Research Platform

Qoin is a multi-client research and paper-trading platform for forex, crypto and listed equities. React web and React Native share TypeScript contracts. Rust/Axum owns market-event ingestion, state estimation, deterministic vector decisions and risk gating; C++/Drogon is the HTTP gateway; MongoDB is the durable store. The current strategy is a research baseline, not a validated or proven profitable edge.

## Repository structure

```text
Qoin-full-repo/
├── frontend/
│   ├── apps/web/                 # React + TypeScript + Vite
│   ├── apps/mobile/              # Expo / React Native
│   └── packages/core/            # Shared types, API client, theme and utilities
├── backend/
│   ├── rust-axum/                # Authoritative market-flow and vector engine
│   │   └── crates/qoin-vector-engine/
│   ├── rust-axum-api/            # Compatibility/research API
│   ├── cpp-drogon/               # HTTP gateway; forwards to Axum
│   ├── config/                   # Backend strategy configuration examples
│   ├── research/                 # Python research/reference models
│   └── tests/                    # Backend/reference-model tests
├── shared/contracts/             # OpenAPI and market-event schemas
├── docs/                         # Architecture, runbooks, risk policy and versions
├── scripts/                      # Repository development and paper-build scripts
├── .github/workflows/            # CI workflows
├── Cargo.toml                    # Root Rust workspace manifest
├── package.json                  # Root pnpm workspace scripts
└── docker-compose.yml            # Local MongoDB
```

## Service boundaries

1. Frontend clients handle account flows, instrument discovery and visualization; they never implement authoritative trading decisions.
2. C++ Drogon is the HTTP gateway and forwards market events and queries to Axum. It does not duplicate vector or risk logic.
3. Rust Axum owns event ingestion, state estimation, deterministic vector decisions and risk gates.
4. MongoDB is the durable persistence layer.
5. Research and model experiments cannot override the authoritative decision engine or bypass risk controls.

## Algorithm versions

- [Version 1 — Deterministic vector baseline](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)
- [Version 2 — Set-theoretic attention and relational ledger research](https://github.com/capiyo/Qoin-full-repo/tree/version-2-set-theoretic-attention)
- [Version map](docs/versions/README.md)

Version 1 is the comparison baseline. Version 2 explores explicit set/relationship representations and ledger-based provenance while preserving deterministic decision boundaries. Version 2 must be evaluated against Version 1 using identical chronological data splits, costs and risk budgets; it must not be promoted based on in-sample metrics alone.

## Unified asset universe

The instrument catalogue is provider-backed and searchable. Users select Forex, Crypto or Stocks, then a supported provider/exchange, instrument and timeframe. Coverage is explicit: a listed instrument is not necessarily live-data entitled or trade-enabled. Forex pip conventions and rollover, crypto contract type and funding, and equity exchange sessions/corporate actions require asset-specific adapters and normalization.

## Requirements

Node.js and pnpm 9; Rust stable/Cargo; MongoDB 7 or compatible; C++17, CMake and Drogon development libraries; Python 3 for research/reference tests.

## Start locally

```bash
cp .env.example .env
docker compose up -d mongodb
cargo run --manifest-path backend/rust-axum/Cargo.toml
```

In another terminal:

```bash
cmake -S backend/cpp-drogon -B build/drogon
cmake --build build/drogon
./build/drogon/qoin-gateway
```

The default gateway binds to loopback at `127.0.0.1:8080`; Axum binds to loopback at `127.0.0.1:8081`.

## Safety and research status

Qoin is research/paper-trading software. A signal is not an order, allocation, or promise of returns. Live order submission remains disabled until durable execution, independent risk controls, reconciliation, recovery, kill-switch behaviour and all policy gates are implemented and independently verified, followed by explicit approval.
