# Qoin — Market-Force Research Monorepo

Qoin is a multi-client market research and paper-trading system. The repository is divided into `frontend/` and `backend/`. React web and React Native share TypeScript packages. Rust/Axum owns market-event ingestion, state estimation, strategy-map validation and vector decisions; C++/Drogon is the HTTP gateway; MongoDB is the durable store. The current strategy is a research baseline, not a validated or proven profitable edge.

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

1. Frontend web and mobile call gateway/API contracts and never implement trading decisions.
2. C++ Drogon is the HTTP gateway and forwards market events and queries to Axum. It must not duplicate strategy, vector, or risk logic.
3. Rust Axum owns event ingestion, state estimation, vector decisions and strategy state.
4. MongoDB is the durable persistence layer.

The separate `backend/rust-axum-api` owns compatibility research signals and Firebase-authenticated user portfolio persistence; it is not a second owner of integrated event-flow endpoints.

## Version track

- [Version 1 — Vector baseline](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)
- [Version map](docs/versions/README.md)

The system remains paper/research-only until every gate in [docs/RISK_POLICY.md](docs/RISK_POLICY.md) is independently verified and live trading is explicitly approved in writing.

## Requirements

Node.js and pnpm 9; Rust stable/Cargo; MongoDB 7 or compatible; C++17, CMake and Drogon development libraries; Python 3 for research/reference tests.

## Start locally

```bash
cp .env.example .env
# Edit .env with your Qoin Firebase project and local service settings.
docker compose up -d mongodb
cargo run --manifest-path backend/rust-axum/Cargo.toml
```

In a second terminal, start the isolated research-signal API:

```bash
cargo run --manifest-path backend/rust-axum-api/Cargo.toml
```

In a third terminal, start the gateway:

```bash
cmake -S backend/cpp-drogon -B build/drogon
cmake --build build/drogon
./build/drogon/qoin-gateway
```

Both Rust services load the repository-root `.env` file when started from the repository root (explicit process environment variables take precedence). The default gateway binds to loopback at `127.0.0.1:8080`; the integrated flow-engine Axum service binds to `127.0.0.1:8081`; the isolated research-signal Axum service binds to `127.0.0.1:8082`. The gateway forwards `/v1/signal` and `/v1/portfolio` to the research service and handles browser CORS preflight for those routes; it uses bearer tokens rather than cookies. The research API allows the local development origins by default and accepts additional comma-separated browser origins through `QOIN_WEB_ORIGINS`. The portfolio endpoints require a Firebase ID token in the `Authorization: Bearer` header; the server validates the token signature against Google's rotating Secure Token JWKs and checks the issuer, audience, expiry and subject. Set `QOIN_FIREBASE_PROJECT_ID` to the ID of Qoin's own Firebase project and configure MongoDB before enabling authenticated portfolio persistence. Do not reuse Funspot Firebase credentials or expose either Axum service directly to the public internet. Configure `QOIN_MONGODB_URI`, `QOIN_MONGODB_DATABASE`, `QOIN_FIREBASE_PROJECT_ID`, `QOIN_AXUM_ADDR`, `QOIN_AXUM_URL`, `QOIN_DROGON_PORT`, `QOIN_RESEARCH_API_ADDR` and `QOIN_RESEARCH_API_URL` as required.

## Firebase identity setup

Create a Firebase project owned by Qoin and enable the Email/Password provider in Firebase Authentication. Set `QOIN_FIREBASE_PROJECT_ID` on the research API and set the corresponding `VITE_FIREBASE_*` and `EXPO_PUBLIC_FIREBASE_*` client values for the web and mobile build environments. All three project IDs must match. Vite reads the repository-root `.env` through its config; for local Expo builds, place the `EXPO_PUBLIC_FIREBASE_*` entries in `frontend/apps/mobile/.env` or export them before starting Expo. Client API keys are public Firebase app identifiers, not server credentials; never place service-account private keys in either client. If the project settings are absent, clients remain in research-preview mode and server portfolio routes stay unavailable. The mobile Firebase Auth client uses AsyncStorage-backed persistence.

The web and mobile clients can now sign in/create an account and synchronize the instrument basket through the authenticated `/v1/portfolio` API. A deployment still needs its own Firebase project, enabled Email/Password provider, and reachable MongoDB database before this path is operational.

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
bash scripts/smoke.sh  # includes gateway-to-research /v1/signal forwarding
```

Do not switch from paper/research operation to live execution without satisfying every risk gate and receiving explicit written approval.
