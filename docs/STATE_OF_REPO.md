# State of repository

This branch contains the Qoin research and paper-trading platform foundation. The fixed architecture is Rust/Axum for authoritative vector and decision processing, C++/Drogon as the public HTTP/WebSocket gateway, MongoDB for persistence, React web and React Native clients sharing `@qoin/core`, and a Tauri 2 desktop client.

## Implemented capabilities

- Axum event ingestion, state snapshots, deterministic vector decisions, decision traces, algorithm catalogue, strategy configuration, WebSocket streaming, read-only APIs and the shared research-signal endpoint (`POST /v1/signal`).
- Drogon proxies the research-signal endpoint to Axum and exposes the optional JEPA research prediction endpoint; market ingestion and strategy mutation remain private to Axum.
- A domain-neutral Rust core contract crate for entity identity, source-attributed observations, feature vectors and algorithm decisions. Finance remains the first implemented vertical; other verticals still require their own adapters and algorithms.
- MongoDB persistence for raw events, state snapshots, decisions, strategy configuration and JEPA research predictions. Axum rebuilds rolling per-symbol state from the immutable raw event log on startup in deterministic `(symbol, event time, event ID, venue)` order, so events persisted before a crash but not followed by a snapshot are recovered. Malformed legacy rows are logged and skipped. In-process ingestion is serialized per canonical symbol across persistence and decision creation to prevent concurrent same-symbol events from writing stale snapshots out of order.
- Historical event replay with venue filtering, deterministic event ordering, execution-cost assumptions, partial-fill simulation and explicit limitations. It is a research simulator, not a production portfolio accounting engine.
- Market-data adapters:
  - Binance USD-M: trades, best bid/ask, mark/funding, liquidation events and open-interest polling.
  - Coinbase Advanced Trade: public spot trade feed.
  - Kraken Spot WebSocket v2: public spot trade feed.
  - Finnhub: authenticated trade feed, news and economic calendar; a valid key, subscription and symbol entitlement are required.
- Provider-specific symbols and venue IDs keep observations from different venues separate. Provider adapters preserve source timestamps and reject malformed observations.
- Firebase client authentication, Firestore user-profile and Archive persistence, and Firebase Storage avatar rules for web and mobile.
- Web, React Native and Tauri desktop clients, shared TypeScript contracts, algorithm catalogue preview and transparent vector inspection.
- GitHub Actions workflows for Rust, Drogon, web, mobile, desktop and Axum–Drogon–MongoDB integration.
- Dockerfiles for Axum and Drogon plus a Docker Compose topology with private Axum/MongoDB services, authenticated MongoDB, a least-privilege application user and a published gateway only.

## Verification status

The branch has prior green CI snapshots documented in the PR history. The new provider adapters and container deployment changes are not considered verified until the corresponding checks pass on the exact current commit. A container build and full-stack smoke-test workflow has been added; its result must be checked before using the Compose stack.

## Remaining production blockers

### Data infrastructure
- Coinbase and Kraken adapters currently ingest trades only; they do not yet maintain complete level-2 books.
- Binance is the only source currently supplying funding, liquidation and open-interest observations.
- There is no resumable historical backfill, immutable dataset versioning, complete symbol master, automatic provider arbitration/failover, or measured provider freshness/uptime SLO.
- Finnhub coverage is plan- and symbol-entitlement-dependent. The instrument registry is curated and does not guarantee live data or redistribution rights.

### Research and algorithms
- The deterministic baseline is not independently validated as profitable.
- Portfolio accounting across instruments/currencies, robust position sizing, correlated exposure controls, walk-forward/out-of-sample evaluation, regime analysis and reproducible dataset versioning remain incomplete.
- The algorithm catalogue and market-access tiers are preview-only. Independent strategy runtimes and verified entitlement enforcement are not complete.

### Identity and application security
- Firebase Auth and Firestore rules protect client-owned profile/Archive documents. Axum does not yet verify Firebase ID tokens for future personalized server-side routes.
- Public market-data routes are read-only, but ingress-level rate limits, request-size limits, WebSocket connection/origin controls, DDoS protection and audit telemetry must be configured before broad public exposure.
- Payment reconciliation, refund/chargeback handling and paid entitlement activation are not complete.

### Trading and risk
- No live order-submission route is registered. The isolated Binance executor is not connected to the decision path.
- Deterministic pre-trade risk validation and a provider-neutral order lifecycle state machine exist as tested primitives, but neither is connected to a durable order store or broker adapter. Execution-report deduplication is only in-memory.
- Exchange-side protective orders, durable idempotency, authenticated account ownership, order/position reconciliation, operational kill switch, testnet lifecycle verification and independent operational review are incomplete.
- Keep real-money trading disabled. A configuration flag or user confirmation is not a substitute for those controls.

### Operations and releases
- The Docker Compose deployment is a baseline, not a hosted production environment. No hosting provider, production DNS/TLS, managed secret store, external monitoring, MongoDB backup/restore drill or disaster-recovery objective has been configured.
- The current branch does not include a committed Rust `Cargo.lock`; reproducible dependency resolution and pinned image digests/SBOM scanning remain release requirements.
- Desktop packaging must pass the final Windows, macOS and Linux matrix before release.

## Release policy

Qoin is currently a research/paper-trading system. Do not claim production readiness, investment performance, complete market coverage or safe real-money execution until the relevant gates above are implemented, tested on the exact release commit, and operationally configured.
