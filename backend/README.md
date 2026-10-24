# Qoin Backend Services

The monorepo keeps UI clients under `frontend/` and Rust/C++ services under `backend/`. This backend remains research-only; live order submission is disconnected.

## Service ownership

- `backend/drogon` — C++ Drogon public HTTP gateway, request forwarding, API boundary, and response/error handling.
- `backend/axum` — Rust/Axum authoritative market-event ingestion, state estimation, strategy evaluation, vector/attention decisions, and MongoDB persistence.
- `backend/axum-api` — compatibility research signal API.
- `backend/crates/qoin-vector-engine` — reusable deterministic Rust signal crate.
- `contracts/` — shared HTTP and event schemas.
- `config/`, `research/`, `tests/` — strategy configuration, research models, and verification.

Drogon must not duplicate Axum's business logic. MongoDB is the durable source of truth for the active versioned edge strategy map and recorded market state/decisions.

## Run locally

Requirements: Rust stable, C++17, CMake, Drogon development libraries, Docker, and MongoDB 7.

```sh
docker compose -f backend/docker-compose.yml up -d mongodb
cargo fmt --all --manifest-path backend/Cargo.toml -- --check
cargo test --workspace --all-features --manifest-path backend/Cargo.toml
cargo run --manifest-path backend/axum/Cargo.toml
```

In another terminal:

```sh
cmake -S backend/drogon -B build/drogon
cmake --build build/drogon
./build/drogon/qoin-gateway
```

Set `QOIN_MONGODB_URI`, `QOIN_MONGODB_DATABASE`, `QOIN_AXUM_ADDR`, `QOIN_AXUM_URL`, `QOIN_DROGON_PORT`, and a strong `QOIN_ADMIN_TOKEN`. Event ingestion and strategy mutation are denied if the token is missing and are checked in both Drogon and Axum. Keep Axum bound to loopback/private networking.

## Strategy persistence and safety

Axum seeds a validated versioned baseline in MongoDB's `edge_strategy` collection if no active strategy exists. Legacy maps are migrated to the expanded nine-force schema. Entries require sequence continuity, fresh state/spread, persistence, explicit regime approval, and expected edge above the configured cost multiple. Missing gates fail closed. The independent risk gate also fails closed until all owner-approved limits and account/exchange context exist. The `live-execution` feature only compiles the adapter; no signal invokes it. Live order submission remains disconnected and disabled.

## Branch policy

- `backend`: Rust/C++ services, MongoDB configuration, contracts, research, backend CI and tests.
- `frontend`: React web, React Native, and shared TypeScript package.
- `main`: integration snapshot containing both sides.
