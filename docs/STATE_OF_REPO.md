# State of repository

This branch contains the Qoin multi-asset research platform. The fixed architecture is Rust/Axum for authoritative V1 vector and decision processing, C++/Drogon as the public HTTP/WebSocket gateway, MongoDB for persistence, React web and React Native clients sharing `@qoin/core`, and a Tauri 2 desktop client.

## Implemented capabilities

- Axum event ingestion, state snapshots, deterministic V1 vector decisions and calculation traces, strategy configuration, WebSocket streaming, read-only APIs, the shared research-signal endpoint (`POST /v1/signal`), and a separate token-protected private Binance USD-M execution route (`POST /internal/orders`).
- Drogon proxies the research-signal endpoint and selected read-only APIs; market ingestion and strategy mutation remain private to Axum.
- A domain-neutral Rust core contract crate for entity identity, source-attributed observations, feature vectors and algorithm decisions. Finance is the first implemented vertical; other verticals still require their own adapters and algorithms.
- MongoDB persistence for raw events, state snapshots, decisions and strategy configuration. Axum rebuilds rolling per-symbol state from the immutable raw event log on startup in deterministic order. Recovery replayed/skipped counts are exposed by `/health`; skipped malformed persisted events degrade service health rather than being silently treated as a clean recovery.
- Historical event replay with venue filtering, deterministic event ordering, causal fill timing, partial-fill simulation, configurable latency, bounded participation and market-impact assumptions, funding cashflows, cost stress scenarios and explicit limitations. Replay is research-only and never submits orders.
- **V2 set / measure / relation ledger foundation:** typed finite sets, counting and observed trade-size measures, explicit missing-size counts, bounded temporal-precedence relations, append-only durable observation and relation records, source-event SHA-256 fingerprints, deterministic record IDs, bounded resumable backfill, reconstruction with hash checks, bounded integrity audits and operator-only correction proposals. Temporal ordering is not causation. V2 is descriptive/research-only and has no authority over V1 decisions or order execution.
- Market-data adapters:
  - Binance USD-M: trades, best bid/ask, mark/funding, liquidation events and open-interest polling.
  - Coinbase Advanced Trade: public spot trade feed.
  - Kraken Spot WebSocket v2: public spot trade feed.
  - Finnhub: authenticated trade feed, news and economic calendar; a valid key, subscription and symbol entitlement are required.
- Provider-specific symbols and venue IDs keep observations from different venues separate. Provider adapters preserve source timestamps and reject malformed observations.
- Firebase client authentication, Firestore user-profile and Archive persistence, and Firebase Storage avatar rules for web and mobile. Server-side Firebase ID-token verification for personalized Axum routes is not yet implemented.
- Web, React Native and Tauri desktop clients, shared TypeScript contracts, algorithm catalogue preview and transparent vector inspection. The latest PR changes include Archive/Explore, live market state, instrument details, V1 calculation traces and V2 ledger panels.
- A versioned algorithm catalogue and fail-closed entitlement domain rules. The catalogue and market-access tiers are preview-only; verified user grants and independent strategy runtimes are not yet integrated.
- Equal-notional broker allocation planning for up to 15 user-selected broker-account targets. It converts portfolio-currency target shares into broker-native quantities, rounds down to quantity steps and leaves residual unallocated. This is a planning primitive, not multi-broker execution.
- GitHub Actions workflows for Rust, Drogon, web, mobile, desktop, container build and Axum–Drogon–MongoDB integration.
- Dockerfiles for Axum and Drogon plus a Docker Compose topology with private Axum/MongoDB services, authenticated MongoDB, a least-privilege application user and a published gateway only. Live execution settings default to disabled.

## Current branch and verification

- Main feature PR: [#15 — Qoin live market streams, vector space and multi-asset Explore](https://github.com/capiyo/Qoin-full-repo/pull/15), currently draft and not merged.
- Audited PR head at 2026-11-17: `118f3404770259ae65d3b98770dbe035fd1475a7`.
- Eight PR-triggered workflows were queued at the time of the audit: live workspace, engine, Rust workspace, container build, desktop, React Native, React web, and Axum–Drogon–MongoDB integration. Queued is not passed. Recheck all runs on the exact current head before release or merge.
- JEPA removal is recorded as complete on the active PR branch: Python sidecar/tests, Rust sidecar module, JEPA workflow, API/proxy/Compose references, client health field and obsolete JEPA specification were removed. Final acceptance must repeat a repository-wide search.
- No merge to `main`, production deployment, or live-execution activation is authorized by this status report.

## Remaining production blockers

### Data infrastructure
- Coinbase and Kraken currently ingest trades only; they do not maintain complete level-2 books.
- Binance is the only source currently supplying funding, liquidation and open-interest observations.
- There is no complete resumable historical market-data backfill, immutable dataset versioning, complete symbol master, automatic provider arbitration/failover, or measured provider freshness/uptime SLO.
- Finnhub coverage is plan- and symbol-entitlement-dependent. Instrument catalogue membership does not guarantee live data, complete history or redistribution rights.

### Research and algorithms
- The deterministic baseline is not independently validated as profitable.
- Portfolio accounting across instruments/currencies, robust position sizing, correlated exposure controls, walk-forward/out-of-sample evaluation, regime analysis and reproducible dataset versioning remain incomplete.
- Independent strategy runtimes, isolated algorithm state, authenticated durable user selection and verified entitlement enforcement are not complete.
- V2 still needs pagination/continuation for large ledger views, stronger property-based tests and an operational correction/backfill runbook. It is not a generalized measure algebra or independent autonomous decision engine.

### Identity, notifications and application security
- Firebase Auth and Firestore rules protect client-owned profile/Archive documents. Axum does not yet verify Firebase ID tokens for personalized server-side routes.
- Archive selection is not yet fully synchronized as durable user-owned backend state across devices.
- Notification history is not yet a durable per-user queue with cross-device read state, retry semantics and background push delivery.
- Public market-data WebSockets need ingress-level rate limits, request-size limits, connection/origin controls, DDoS protection and audit telemetry before broad public exposure.
- Payment reconciliation, refunds/chargebacks and paid entitlement activation are not complete.

### Broker execution and risk
- The Decimal equal-allocation primitive supports up to 15 targets but is not wired into the private order route.
- The provider-neutral broker contract and risk/lifecycle modules are domain foundations; there is only one existing Binance USD-M execution adapter. Broker directory entries and market-data providers are not executable broker accounts.
- User broker-account linking, a server-side credential vault, real multi-broker adapters, durable parent/child fan-out, concurrent dispatch, atomic aggregate risk reservation, per-account position reconciliation and automatic recovery remain unfinished.
- The private `POST /internal/orders` route is token-protected and not proxied through Drogon. It has explicit Binance testnet/live confirmation and credential gates. It must remain disabled by default and is not an unattended production trading system.
- The operator kill switch blocks new entries; it does not cancel existing exchange orders or flatten positions. Existing reconciliation is operator-triggered rather than an automatic worker.
- Portfolio-level pre-trade risk checks are not fully integrated with multi-account routing. Live execution must not be enabled based on the presence of isolated risk primitives.

### Operations and releases
- Docker Compose is a deployment baseline, not a hosted production environment. No production host, DNS/TLS, managed secret store, external monitoring, MongoDB backup/restore drill or disaster-recovery objective has been configured.
- Reproducible dependency resolution, pinned image digests, SBOM/vulnerability scanning and a complete secret review remain release requirements.
- Desktop packaging must pass Windows, macOS and Linux checks on the exact release commit.
- The complete Rust, C++, web, mobile, desktop, Python, Firebase emulator-security and integration matrix must pass on one exact commit before release.

## Release policy

Qoin is currently a research/paper-trading platform. Do not claim production readiness, investment performance, complete market coverage or safe real-money execution until the relevant gates above are implemented, tested on the exact release commit and operationally configured.
