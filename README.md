# Qoin — Unified Multi-Asset Vector Platform

Qoin is a multi-client research and paper-trading platform for forex, crypto and listed equities. React web and React Native share API contracts. Rust/Axum owns event ingestion, market state, deterministic vector decisions and risk gating; C++/Drogon is the HTTP gateway; MongoDB is the durable store. The vector engine is a research baseline, not a validated or proven profitable edge.

## Repository structure

```text
Qoin-full-repo/
├── frontend/
│   ├── apps/web/                 # React + TypeScript + Vite
│   ├── apps/mobile/              # Expo / React Native
│   └── packages/core/            # Shared types, API client, theme and utilities
├── backend/
│   ├── rust-axum/               # Authoritative event and vector engine
│   │   └── crates/qoin-vector-engine/
│   ├── rust-axum-api/            # Compatibility/research API
│   ├── cpp-drogon/               # HTTP gateway; forwards to Axum
│   ├── config/                   # Strategy configuration examples
│   ├── research/                 # Python reference models
│   └── tests/                    # Backend/reference tests
├── shared/contracts/             # OpenAPI and market-event schemas
├── docs/                         # Architecture, runbooks, risk policy and versions
├── scripts/                      # Repository development and paper-build scripts
└── docker-compose.yml             # Local MongoDB
```

## Service boundaries

1. Frontend clients handle account flows, instrument discovery and visualization; they never authorize themselves or implement authoritative trading decisions.
2. C++ Drogon is the public-facing HTTP gateway and forwards to Axum. It does not duplicate vector or risk logic.
3. Rust Axum owns authenticated identity context, entitlement checks, event ingestion, state estimation, vector decisions and risk gates.
4. MongoDB stores profiles, entitlement grants, instrument metadata, preferences, event/state/decision history and audit records.
5. Provider adapters own venue-specific discovery, streaming, normalization and provenance.

## Unified asset universe

The instrument catalogue is provider-backed and searchable. Users select Forex, Crypto or Stocks, then a supported provider/exchange, instrument and timeframe. Coverage is explicit: a listed instrument is not necessarily live-data entitled or trade-enabled. Forex pip conventions and rollover, crypto spot/perpetual/futures contracts and funding, and equity exchange sessions/corporate actions require asset-specific adapters and normalization.

## Architecture and rollout

See [Unified multi-asset platform](docs/UNIFIED_MULTI_ASSET_PLATFORM.md), [functional multi-market confluence](docs/FUNCTIONAL_MULTIMARKET_CONFLUENCE.md), and [risk policy](docs/RISK_POLICY.md).

The retired JEPA model is not part of the active architecture. Qoin uses one deterministic vector mechanism. User permissions are server-owned and checked on every protected API; frontend entitlement data is display-only.

## Requirements and local services

Node.js and pnpm 9; Rust stable/Cargo; MongoDB 7 or compatible; C++17, CMake and Drogon development libraries; Python 3 for reference tests.

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

The default gateway binds to loopback at `127.0.0.1:8080`; Axum binds to `127.0.0.1:8081`. Do not expose Axum directly to the public internet.

## Frontend and validation

```bash
pnpm install
pnpm dev:web
pnpm dev:mobile
pnpm --filter @qoin/react-web build
cargo test --workspace
python3 -m unittest discover -s backend/tests -v
bash scripts/smoke.sh
```

All signals remain research/paper-only. Live order submission is disconnected. Do not enable live trading without independently verified risk controls and explicit written approval.
