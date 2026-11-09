# Qoin — Market-Force Trading Research Monorepo

Qoin is a multi-client, domain-extensible intelligence and decision platform. Finance is its first application vertical; a domain-neutral Rust core defines entities, source-attributed observations, feature vectors and algorithm decisions. React web, React Native and the native desktop shell share TypeScript packages. Rust/Axum owns event ingestion, market state, strategy-map and vector decisions; C++/Drogon is the HTTP gateway; MongoDB is the durable store. The current complete application is finance-focused research/paper trading, not a validated or proven profitable edge.

## Repository structure

```text
Qoin-full-repo/
├── frontend/
│   ├── apps/
│   │   ├── web/                 # React + TypeScript + Vite web client
│   │   ├── mobile/              # Expo / React Native mobile client
│   │   └── desktop/             # Tauri 2 native desktop shell around shared React client
│   ├── packages/core/           # Shared types, API client, theme and utilities
│   └── archive/next-web/        # Preserved legacy Next.js app; not active
├── backend/
│   ├── rust-axum/               # Authoritative market-flow and vector engine
│   │   ├── crates/qoin-vector-engine/\n│   ├── crates/qoin-vector-motion/\n│   └── crates/qoin-domain-core/       # Domain-neutral entities, observations, vectors and decisions
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

1. Frontend web, mobile and desktop call the same gateway/API contracts and never implement trading decisions.
2. Desktop is a Tauri 2 native shell around the shared React/Vite client. It adds a desktop workstation layout and native installers without creating a separate trading engine or a forked strategy implementation.
3. C++ Drogon is the public HTTP/WebSocket gateway for read-only queries and market updates. Provider ingestion and strategy mutation use Axum's loopback-only internal routes; they are not exposed as public gateway writes. Drogon must not duplicate strategy, vector, or risk logic.
4. Rust Axum owns event ingestion, state estimation, vector/attention decisions and strategy state.
5. MongoDB is the durable persistence layer.\n6. Provider adapters normalize exchange/vendor observations into venue-scoped engine events. Current feeds include Binance USD-M, Coinbase Advanced Trade, Kraken Spot and optional Finnhub; coverage and limitations are listed in [docs/MARKET_DATA_PROVIDERS.md](docs/MARKET_DATA_PROVIDERS.md).\n7. Domain-specific algorithms and safety policies sit above the domain-neutral contracts; the core itself does not assume financial instruments or trades.
6. The JEPA world model is research-only; it cannot override Axum decision authority or bypass risk controls.

The separate `backend/rust-axum-api` remains isolated for compatibility/research and is not a second owner of the integrated event-flow endpoints.

## Version tracks

- [Version 1 — Vector baseline](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)
- [Version 2 — JEPA world model](https://github.com/capiyo/Qoin-full-repo/tree/version-2-jepa-world-model)
- [Version map](docs/versions/README.md)

Both versions remain paper-only until every gate in [docs/RISK_POLICY.md](docs/RISK_POLICY.md) is independently verified and live trading is explicitly approved in writing.

## Requirements

Docker Engine with Docker Compose; Node.js and pnpm 9; Rust stable/Cargo; MongoDB 7 or compatible; C++17, CMake and Drogon development libraries; Python 3 for research/reference tests.

## Start locally with Docker Compose

1. Copy the example environment file and generate distinct hexadecimal MongoDB passwords:
   ```bash
   cp .env.example .env
   openssl rand -hex 32
   ```
   Put one generated value in `MONGO_INITDB_ROOT_PASSWORD` and a different value in `QOIN_MONGODB_APP_PASSWORD`. Replace the example placeholders before starting.
2. Build and start the stack:
   ```bash
   docker compose build
   docker compose up -d --wait
   docker compose ps
   ```
3. The public gateway is available at `http://127.0.0.1:8080`. Axum and MongoDB are private to the Compose network. Follow logs with `docker compose logs -f axum gateway`.

The Compose topology uses MongoDB authentication and a least-privilege Qoin application account. The initialization script runs only when the MongoDB data volume is first created. For managed hosting, use a managed MongoDB deployment, secret manager, TLS ingress, backups and monitoring; see [docs/DEPLOYMENT_DOCKER.md](docs/DEPLOYMENT_DOCKER.md).

For manual development outside Compose, provide a reachable MongoDB URI and set `QOIN_AXUM_ADDR`, `QOIN_AXUM_INTERNAL_ADDR`, `QOIN_AXUM_URL`, `QOIN_AXUM_WS_URL` and `QOIN_DROGON_HOST` consistently. The default non-container Axum and Drogon listeners bind to loopback. Never expose Axum directly to the public internet.

## Firebase account setup

Web, mobile and desktop use shared Firebase Authentication, private Firestore profiles and Firebase Storage avatars. See [docs/FIREBASE_SETUP.md](docs/FIREBASE_SETUP.md) for Console configuration, environment variables, and security-rule deployment. Firestore rules restrict each user to their own supported profile and Archive fields. Archive selections are saved per Firebase user and shared across clients.

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

The container stack is a deployment baseline, not evidence that Qoin is production-ready. Historical backfill, provider failover and measured data SLOs, portfolio-grade evaluation, complete authenticated server operations, operational backups/monitoring and release verification remain open. Live order submission is not wired into the API and must remain disabled until every risk gate in [docs/RISK_POLICY.md](docs/RISK_POLICY.md) is implemented and independently verified.
