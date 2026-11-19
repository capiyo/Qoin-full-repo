# State of repository

This branch contains the Qoin multi-asset research platform and an operator-controlled private Binance USD-M execution route. The fixed architecture is Rust/Axum for authoritative vector and decision processing, C++/Drogon as the public HTTP/WebSocket gateway, MongoDB for persistence, React web and React Native clients sharing `@qoin/core`, and a Tauri 2 desktop client.

## Implemented capabilities

- Axum event ingestion, state snapshots, authoritative seven-force vector decisions and full calculation traces, a durable append-only V2 finite-set/temporal-relation ledger (`GET /api/v1/set-ledger`), algorithm catalogue, strategy configuration, WebSocket streaming, read-only APIs, the shared research-signal endpoint (`POST /v1/signal`), and a token-protected private live order route (`POST /internal/orders`).
- Drogon proxies the research-signal endpoint to Axum; market ingestion and strategy mutation remain private to Axum.
- A domain-neutral Rust core contract crate for entity identity, source-attributed observations, feature vectors and algorithm decisions. Finance remains the first implemented vertical; other verticals still require their own adapters and algorithms.
- MongoDB persistence for raw events, state snapshots, seven-force decisions, legacy edge-map settings and append-only V2 ledger records. Axum rebuilds rolling per-symbol state from the immutable raw event log on startup in deterministic `(symbol, event time, event ID, venue)` order, so events persisted before a crash but not followed by a snapshot are recovered. Malformed legacy rows are logged and skipped. In-process ingestion is serialized through 64 bounded lock stripes across persistence and decision creation to prevent concurrent same-symbol events from writing stale snapshots out of order without allowing lock-map memory growth from arbitrary symbols. Recovery replayed/skipped counts are exposed by `/health`; any skipped persisted event marks service health degraded (`503`) instead of reporting a clean startup.
- Historical event replay with venue filtering, deterministic event ordering, execution-cost assumptions, partial-fill simulation and explicit limitations. Historical replay calls the same versioned seven-force engine as live ingestion; the legacy edge-map replay helper is retained only for unit tests. The V2 set ledger stores append-only observation and relation records derived from the canonical raw event log. It remains an analytical projection and does not grant execution authority. It is a research simulator, not a production portfolio accounting engine.
- Market-data adapters:
  - Binance USD-M: trades, best bid/ask, mark-price/funding-rate observations, liquidation events and open-interest polling. Mark-price observations no longer overwrite the last trade; mark-to-trade basis and relative OI change feed the seven-force engine.
  - Coinbase Advanced Trade: public spot trade feed.
  - Kraken Spot WebSocket v2: public spot trade feed.
  - Finnhub: authenticated trade feed, news and economic calendar; a valid key, subscription and symbol entitlement are required.
- Provider-specific symbols and venue IDs keep observations from different venues separate. Provider adapters preserve source timestamps and reject malformed observations.
- Firebase client authentication, Firestore user-profile and Archive persistence, and Firebase Storage avatar rules for web and mobile.
- Web, React Native and Tauri desktop clients, shared TypeScript contracts, algorithm catalogue preview and transparent vector inspection.
- GitHub Actions workflows for Rust, Drogon, web, mobile, desktop and Axum–Drogon–MongoDB integration.
- Dockerfiles for Axum and Drogon plus a Docker Compose topology with private Axum/MongoDB services, authenticated MongoDB, a least-privilege application user and a published gateway only. Live execution settings are passed only to the private Axum container and default to disabled.

## Verification status

The branch has prior green CI snapshots documented in the PR history. The new provider adapters and container deployment changes are not considered verified until the corresponding checks pass on the exact current commit. A container build and full-stack smoke-test workflow has been added; its result must be checked before using the Compose stack.

## Remaining production blockers

### Data infrastructure
- Coinbase and Kraken adapters currently ingest trades only; they do not yet maintain complete level-2 books.
- Binance is the only source currently supplying funding, liquidation and open-interest observations.
- There is no resumable historical backfill, immutable dataset versioning, complete symbol master, automatic provider arbitration/failover, or measured provider freshness/uptime SLO.
- Finnhub coverage is plan- and symbol-entitlement-dependent. The instrument registry is curated and does not guarantee live data or redistribution rights.

### Research and algorithms
- The seven-force deterministic baseline uses seven dimensions (60-second decayed/stale-expiring OFI, bid/ask depth imbalance, volatility, funding, five-minute decayed relative OI change, 60-second decayed signed liquidation pressure and mark-to-trade basis), a seven-by-seven attention matrix, top-k force selection, directional alignment and quote freshness/spread gates. The baseline is not independently validated as profitable.
- Portfolio accounting across instruments/currencies, robust position sizing, correlated exposure controls, walk-forward/out-of-sample evaluation, regime analysis and reproducible dataset versioning remain incomplete.
- The algorithm catalogue and market-access tiers are preview-only. Independent strategy runtimes and verified entitlement enforcement are not complete.

### Identity and application security
- Firebase Auth and Firestore rules protect client-owned profile/Archive documents. Axum does not yet verify Firebase ID tokens for future personalized server-side routes.
- Public market-data routes are read-only, but ingress-level rate limits, request-size limits, WebSocket connection/origin controls, DDoS protection and audit telemetry must be configured before broad public exposure.
- Payment reconciliation, refund/chargeback handling and paid entitlement activation are not complete.

### Trading and risk
- The private `POST /internal/orders` route is token-protected and not proxied through Drogon. It supports explicit Binance Futures testnet/live modes, requires the matching confirmation string, Binance API credentials and a unique durable client order ID in MongoDB. Testnet mode enforces the testnet hostname; live mode enforces the production hostname.
- A persistent operator-controlled new-order gate is available through `POST /internal/trading/kill-switch`; it defaults closed and is reported in `/health`. It blocks new entries but does not cancel existing orders or flatten positions.
- The Binance USD-M adapter enforces configured per-order quantity/notional caps, checks stop direction against mark price, submits a reduce-only stop-market order for the filled quantity, and attempts a reduce-only emergency close if stop placement fails. It is the only current execution adapter. Coinbase Advanced, Kraken Spot and Finnhub (including OANDA-namespaced FX/CFD reference symbols) are market-data integrations, not order destinations or direct OANDA account connections. The broker-neutral `BrokerAdapter` registry has no concrete registered implementations yet.
- Durable client-order-ID deduplication prevents a retry from blindly submitting the same order twice. Uncertain exchange outcomes are marked `reconciliation_required`; authenticated `GET /internal/orders/{client_order_id}/reconcile` looks up the entry and stop orders and persists their exchange status. Reconciliation is operator-triggered, not an automatic worker.
- Capital-aware broker routing now has Decimal equal-notional allocation, a consolidation-first cost policy (including per-child minimum commissions, split-size slippage, FX cost and operational overhead), aggregate portfolio risk validation, and a parent/child lifecycle with deterministic idempotency IDs, fill deduplication, unknown-outcome handling and Mongo intent persistence helpers with revision guards. The aggregate gate reserves the proposed stop loss against the remaining daily-loss budget. These are testable foundations only: the fan-out persistence helpers are not wired to AppState or an API, no concurrent multi-account dispatcher or automatic reconciliation worker exists, and the generic adapter registry has no concrete adapters.
- The frontend broker collection is a status catalogue and local preview, not account linking. Secure per-user broker credential storage, authenticated account discovery, server-side Firebase ID-token verification for personalized Axum routes, verified owner-scope injection, per-account preflight across all selected accounts and concurrent dispatch remain incomplete. The current private order route remains Binance-only; market-data-only and pending integrations cannot receive orders.
- Portfolio-level pre-trade risk checks are not yet wired into the existing private order route. Account ownership verification, account/position reconciliation, cancellation/amendment lifecycle, testnet end-to-end validation and independent operational review remain incomplete.
- The private execution route is operator-invoked and configuration-gated; it is not connected to autonomous strategy signals and is not an unattended production trading system.

### Operations and releases
- The Docker Compose deployment is a baseline, not a hosted production environment. No hosting provider, production DNS/TLS, managed secret store, external monitoring, MongoDB backup/restore drill or disaster-recovery objective has been configured.
- The current branch does not include a committed Rust `Cargo.lock`; reproducible dependency resolution and pinned image digests/SBOM scanning remain release requirements.
- Desktop packaging must pass the final Windows, macOS and Linux matrix before release.

## Release policy

Qoin is currently a research/paper-trading system. Do not claim production readiness, investment performance, complete market coverage or safe real-money execution until the relevant gates above are implemented, tested on the exact release commit, and operationally configured.
