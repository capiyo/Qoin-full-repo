# Qoin — Domain-Extensible Intelligence Platform

Qoin is a multi-client, domain-extensible intelligence and decision platform. Finance is its first application vertical, not the definition of the core. A domain-neutral Rust crate defines entities, source-attributed observations, feature vectors and algorithm decisions. The current complete application remains finance-focused research and paper trading; the baseline is not a validated or proven profitable edge.

## Repository structure

```text
Qoin-full-repo/
├── frontend/
│   ├── apps/
│   │   ├── web/                  # React + TypeScript + Vite
│   │   ├── mobile/               # Expo / React Native
│   │   └── desktop/              # Tauri 2 native shell
│   ├── packages/core/            # Shared TypeScript contracts and API client
│   └── archive/next-web/         # Preserved legacy app; not active
├── backend/
│   ├── rust-axum/
│   │   ├── src/                  # Finance application and provider adapters
│   │   └── crates/
│   │       ├── qoin-vector-engine/
│   │       ├── qoin-vector-motion/
│   │       └── qoin-domain-core/ # Domain-neutral contracts
│   ├── rust-axum-api/            # Separate compatibility/research API
│   ├── cpp-drogon/               # HTTP/WebSocket gateway
│   ├── jepa-world-model/         # Research-only model service
│   ├── config/
│   ├── research/
│   └── tests/
├── docker/                       # MongoDB initialization
├── firebase/                     # Firestore and Storage rules/tests
├── shared/contracts/             # OpenAPI and event schemas
├── docs/
├── scripts/
├── .github/workflows/
├── Cargo.toml
├── package.json
└── docker-compose.yml             # MongoDB + private Axum + Drogon gateway
```

## Service boundaries

1. Web, mobile and desktop use shared TypeScript contracts and never implement authoritative trading decisions.
2. Rust/Axum owns event ingestion, state estimation, vector decisions, strategy state and the domain-neutral core contracts.
3. C++/Drogon is the public HTTP/WebSocket gateway. Provider ingestion and strategy mutation remain internal and are not exposed as public gateway writes.
4. MongoDB stores raw observations, state snapshots, decisions and strategy configuration.
5. Provider adapters normalize source observations into venue-scoped events. Current feeds include Binance USD-M, Coinbase Advanced Trade, Kraken Spot and optional Finnhub. See [docs/MARKET_DATA_PROVIDERS.md](docs/MARKET_DATA_PROVIDERS.md) for coverage and limitations.
6. Domain-specific algorithms and safety policies sit above the domain-neutral contracts. The core does not assume financial instruments, trades, portfolios or orders.
7. JEPA is research-only and cannot override Axum decision authority or bypass risk controls.

The separate `backend/rust-axum-api` remains isolated for compatibility/research and is not a second owner of the integrated event-flow endpoints.

## Domain expansion

The generic contracts can support finance, agriculture, logistics, energy, manufacturing, healthcare operations, cybersecurity and robotics. Those are architectural target verticals, not claims that complete products already exist. Each requires its own licensed data adapters, algorithms, evaluation and safety policies. See [docs/DOMAIN_NEUTRAL_CORE.md](docs/DOMAIN_NEUTRAL_CORE.md).

## Requirements

Docker Engine and Docker Compose; Node.js and pnpm 9; Rust stable/Cargo; C++17, CMake and Drogon development libraries; Python 3 for research/reference tests.

## Start locally with Docker Compose

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```
2. Generate two different 64-character hexadecimal passwords:
   ```bash
   openssl rand -hex 32
   ```
   Put one in `MONGO_INITDB_ROOT_PASSWORD` and the other in `QOIN_MONGODB_APP_PASSWORD`. Replace the example placeholders before starting.
3. Build and start the full stack:
   ```bash
   docker compose build
   docker compose up -d --wait
   docker compose ps
   ```
4. The gateway is available at `http://127.0.0.1:8080`. Axum and MongoDB are private to the Compose network. Follow logs with `docker compose logs -f axum gateway`.

MongoDB authentication and a least-privilege application account are configured by the Compose stack. The initialization script runs only when the MongoDB data volume is first created. See [docs/DEPLOYMENT_DOCKER.md](docs/DEPLOYMENT_DOCKER.md) before deploying to a hosted environment.

For manual development outside Compose, provide a reachable MongoDB URI and set `QOIN_AXUM_ADDR`, `QOIN_AXUM_INTERNAL_ADDR`, `QOIN_AXUM_URL`, `QOIN_AXUM_WS_URL` and `QOIN_DROGON_HOST` consistently. The default non-container Axum and Drogon listeners bind to loopback. Never expose Axum directly to the public internet.

## Firebase account setup

Web, mobile and desktop use shared Firebase Authentication, private Firestore profiles and Firebase Storage avatars. See [docs/FIREBASE_SETUP.md](docs/FIREBASE_SETUP.md) for Console configuration, environment variables and security-rule deployment. Firestore rules restrict each user to their own supported profile and Archive fields.

## Frontend commands

```bash
pnpm install
pnpm dev:web
pnpm dev:mobile
pnpm dev:desktop
pnpm --filter @qoin/react-web build
pnpm build:desktop
```

## Validation

```bash
cargo test --workspace
python3 -m unittest discover -s backend/tests -v
bash scripts/smoke.sh
```

The Compose build, Firebase rules emulator, Rust, Drogon, web, mobile, desktop and Axum–Drogon–MongoDB integration checks must all pass on the exact release commit.

Qoin remains research/paper-only. Live order submission is not wired into the API and must remain disabled until the controls in [docs/RISK_POLICY.md](docs/RISK_POLICY.md) are implemented and independently verified.
