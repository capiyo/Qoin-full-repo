# Qoin

Qoin is a multi-client research and paper-trading platform. Finance is its first application vertical. It is not a validated or proven profitable trading system.

## Fixed architecture

- **Rust/Axum:** authoritative market-data ingestion, state, vector decisions, risk domain and execution controls.
- **C++/Drogon:** public HTTP/WebSocket gateway and proxy only; it does not calculate decisions.
- **MongoDB:** durable event, state, decision, strategy, paper ledger and application persistence.
- **React web + React Native:** shared TypeScript contracts and API client in `@qoin/core`.
- **Python:** research, reference models, reproducible evaluation and analysis.
- **Tauri 2:** desktop shell; the vector-motion solver is implemented in Rust.

## Repository layout

```text
frontend/
  apps/web/             React + TypeScript + Vite
  apps/mobile/          Expo / React Native
  apps/desktop/         Tauri 2
  packages/core/        Shared API and domain contracts
backend/
  rust-axum/            Authoritative engine and application services
    crates/qoin-vector-engine/
    crates/qoin-vector-motion/
    crates/qoin-domain-core/
  cpp-drogon/           HTTP/WebSocket gateway
  research/             Python research code
  tests/                Python reference tests
shared/contracts/       OpenAPI and event schemas
firebase/               Client security rules and emulator tests
docker/                 MongoDB initialization
docs/                   Architecture, risk, readiness and operations
.github/workflows/      CI matrix
Cargo.toml              Rust workspace
package.json            pnpm workspace
docker-compose.yml      Private MongoDB/Axum + published gateway
```

## Current product boundaries

- The deterministic Rust vector baseline is the active decision mechanism.
- Provider capability differs by venue. Binance USD-M supplies trades, best bid/ask, funding/mark, liquidations and open-interest polling. Coinbase Advanced Trade and Kraken Spot currently supply trades only. Finnhub is optional and depends on a valid key, plan and symbol entitlement.
- The instrument registry is curated. A listed instrument is not proof of complete live or historical coverage.
- Historical backtests only replay events already stored in MongoDB. They do not provide a historical backfill service or complete dataset guarantee.
- Archive and notification persistence must be evaluated separately from client-local UI state; see `docs/READINESS.md`.
- The private Binance order adapter is operator-invoked. Real-money mode is disabled by code policy until the release gates in `docs/RISK_POLICY.md` are independently verified. Never enable it by setting environment variables alone.

## Local development

Requirements: Docker Engine/Compose, Node.js, pnpm 9, Rust stable, C++17/CMake/Drogon development libraries and Python 3.

```bash
cp .env.example .env
# Replace both MongoDB password placeholders with different random 64-character hex values.
openssl rand -hex 32
pnpm install
cargo test --workspace
python3 -m unittest discover -s backend/tests -v
cmake -S backend/cpp-drogon -B build/drogon
cmake --build build/drogon --parallel 2
```

For the container topology, configure the required secrets in `.env`, then run `docker compose config --quiet`, `docker compose build` and `docker compose up -d --wait`. Only the Drogon gateway is published; MongoDB and Axum remain on the private Compose network.

Web/mobile/desktop use Firebase client configuration supplied through their respective environment files. Do not commit credentials. See `docs/FIREBASE_SETUP.md` and `docs/DEPLOYMENT_DOCKER.md`.

## Verification

The exact release commit must pass the Rust workspace, Clippy, formatting, Drogon build/tests, web typecheck/build/tests, React Native typecheck/tests, Python tests, Tauri checks, Firebase rules emulator, Compose smoke tests and the Axum–Drogon–MongoDB end-to-end suite. CI results from older commits are not evidence for a newer commit.

Qoin remains research/paper-only. No profitability or production-readiness claim is made.
