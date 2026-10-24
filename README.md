# Qoin Backend Services

This branch contains the separated Rust/C++ backend and its research and persistence layer. The React and React Native clients live on the `frontend` branch.

## Service ownership

- `services/drogon` — C++ Drogon public HTTP gateway, request forwarding, API boundary, and response/error handling.
- `services/axum` — Rust/Axum authoritative market-event ingestion, state estimation, strategy evaluation, vector/attention decisions, and MongoDB persistence.
- `services/axum-api` — compatibility research signal API.
- `crates/qoin-vector-engine` — reusable deterministic Rust signal crate.
- `contracts/` — shared HTTP and event schemas.
- `config/`, `research/`, `tests/` — strategy configuration, research models, and verification.

Drogon must not duplicate Axum's business logic. MongoDB is the durable source of truth for the active versioned edge strategy map and recorded market state/decisions.

## Run locally

Requirements: Rust stable, C++17, CMake, Drogon development libraries, Docker, and MongoDB 7.

```sh
docker compose up -d mongodb
cargo test --workspace
cargo run --manifest-path services/axum/Cargo.toml
```

In another terminal:

```sh
cmake -S services/drogon -B build/drogon
cmake --build build/drogon
./build/drogon/qoin-gateway
```

Set `QOIN_MONGODB_URI`, `QOIN_MONGODB_DATABASE`, `QOIN_AXUM_ADDR`, `QOIN_AXUM_URL`, and `QOIN_DROGON_PORT` as required. Keep Axum private behind Drogon in deployment.

## Strategy persistence and safety

Axum seeds a validated versioned baseline in MongoDB's `edge_strategy` collection if no active strategy exists. Strategy changes are validated and persisted before activation, and decisions carry the strategy version. Coefficients remain a research baseline—not proof of profitability. Live order submission is disconnected and trading remains disabled by default.

## Branch policy

- `backend`: Rust/C++ services, MongoDB configuration, contracts, research, backend CI and tests.
- `frontend`: React web, React Native, and shared TypeScript package.
- `main`: integration snapshot containing both sides.
