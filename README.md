# Qoin — Market-Force Trading Research Monorepo

Qoin is a multi-client research and paper-trading system. The repository is explicitly divided into `frontend/` and `backend/`. React web and React Native share TypeScript packages. Rust/Axum owns event ingestion, market state, strategy-map and vector decisions; C++/Drogon is the HTTP gateway; MongoDB is the durable store. The strategy is a research baseline, not a validated or proven profitable edge.

## Repository structure

```text
Qoin-full-repo/
├── frontend/
│   ├── apps/
│   │   ├── web/                 # React + TypeScript + Vite web client
│   │   └── mobile/              # Expo / React Native mobile client
│   ├── packages/core/           # Shared types, API client, theme and utilities
│   └── archive/next-web/        # Preserved legacy Next.js app; not active
├── backend/
│   ├── rust-axum/               # Authoritative market-flow and vector engine
│   │   └── crates/qoin-vector-engine/
│   ├── rust-axum-api/           # Separate compatibility/research API
│   ├── cpp-drogon/              # HTTP gateway; forwards to Axum
│   ├── jepa-world-model/        # JEPA-inspired research service
│   ├── config/                  # Backend strategy configuration examples
│   ├── research/                # Python research/reference models
│   └── tests/                   # Backend/reference-model tests
├── shared/contracts/            # OpenAPI and market-event schemas
├── docs/                        # Architecture, runbooks, risk policy and versions
├── scripts/                     # Repository-level development and paper-build scripts
├── .github/workflows/           # CI for frontend and backend services
├── Cargo.toml                   # Root Rust workspace manifest
├── package.json                 # Root pnpm workspace scripts
└── docker-compose.yml            # Local MongoDB
```

## Service boundaries

1. Frontend web and mobile call the gateway/API contracts and never implement trading decisions.
2. C++ Drogon is the HTTP gateway and forwards market events and queries to Axum. It must not duplicate strategy, vector, or risk logic.
3. Rust Axum owns event ingestion, state estimation, vector/attention decisions and strategy state.
4. MongoDB is the durable persistence layer.
5. The JEPA world model is research-only; it cannot override Axum decision authority or bypass risk controls.

The separate `backend/rust-axum-api` remains isolated for compatibility/research and is not a second owner of the integrated event-flow endpoints.

## Version tracks

- [Version 1 — Vector baseline](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)
- [Version 2 — JEPA world model](https://github.com/capiyo/Qoin-full-repo/tree/version-2-jepa-world-model)
- [Version map](docs/versions/README.md)

Both versions remain paper-only until every gate in [docs/RISK_POLICY.md](docs/RISK_POLICY.md) is independently verified and live trading is explicitly approved in writing.

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

The default gateway binds to loopback at `127.0.0.1:8080`; Axum binds to `127.0.0.1:8081`. Do not expose Axum directly to the public internet. Configure `QOIN_MONGODB_URI`, `QOIN_MONGODB_DATABASE`, `QOIN_AXUM_ADDR`, `QOIN_AXUM_URL` and `QOIN_DROGON_PORT` as required.

## Frontend commands

```bash
pnpm install
pnpm dev:web
pnpm dev:mobile
pnpm --filter @qoin/react-web build
```

## Validation

```bash
cargo test --workspace
python3 -m unittest discover -s backend/tests -v
bash scripts/smoke.sh
```

Do not switch from paper/research operation to live execution without satisfying every risk gate and receiving explicit written approval.
