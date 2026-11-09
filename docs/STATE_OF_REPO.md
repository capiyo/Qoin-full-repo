# State of repository

This report describes the integration candidate, not a production release. The fixed architecture is Rust/Axum for authoritative vector decisions, risk and state; C++/Drogon as HTTP/WebSocket gateway only; MongoDB persistence; React web and React Native sharing `@qoin/core`; Python research/backtesting; and Tauri 2 desktop with the Rust vector-motion solver.

## Present foundation

- Rust workspace includes the vector engine, vector-motion solver, domain-neutral core and Axum application.
- Axum ingests venue-scoped events, rebuilds state from the immutable event log, stores decisions and strategy versions, exposes research/backtest APIs and streams.
- Drogon proxies HTTP and WebSocket traffic to Axum.
- Binance USD-M, Coinbase Advanced Trade, Kraken Spot and optional Finnhub adapters exist with different data capabilities.
- Historical replay uses persisted events and explicit execution assumptions; it is not a backfill service.
- A curated multi-asset instrument registry and Archive/Explore clients exist.
- Firebase client authentication, profile/Archive client persistence and Firestore/Storage rules exist.
- Algorithm catalogue and entitlement domain primitives exist, but protected selection and paid-access enforcement require verified backend identity and persisted grants.
- A private Binance USD-M order adapter, durable client-order-ID intent ledger and operator-triggered reconciliation endpoint exist. Real-money mode is blocked pending release gates.

## Known incomplete integrations

### Data
- No resumable historical backfill, immutable dataset versions, per-provider gap completeness, or automatic provider failover.
- Coinbase/Kraken are trades-only. Binance supplies the richer derivatives observations. Finnhub is optional and entitlement-dependent.
- The curated instrument registry does not prove complete live and historical coverage per instrument.

### Research
- No complete portfolio accounting across currencies, cash, fees, funding and correlated exposure.
- Walk-forward/out-of-sample evaluation, benchmark comparisons, fixed-seed run manifests and immutable dataset/config references are incomplete.
- The independent multi-algorithm runtime, isolated algorithm state and durable per-user algorithm selections are incomplete.

### User platform
- Axum Firebase ID-token verification with JWKS caching, issuer/audience/expiry checks is incomplete.
- Server-side entitlements derived from verified UID, payment reconciliation and protected algorithm selection are incomplete.
- Durable cross-device Archive and notification synchronization are incomplete; current client-side and in-memory paths must not be represented as synchronized server state.
- Firebase Console values and provider keys must be supplied outside source control for external integrations.

### Execution
- Portfolio-level risk primitives are not wired into every order path.
- Account ownership verification and account/position synchronization are incomplete.
- Durable cancel/amend/reject/partial-fill state transitions are not fully connected to the exchange adapter.
- Reconciliation is operator-triggered, not an automatic worker; current kill switch blocks new entries only.
- Binance testnet end-to-end lifecycle verification is outstanding.
- Real-money mode must remain disabled.

### Operations and release
- The intermittent Drogon `std::length_error` has not been root-caused with a reproducible failing trace.
- Twenty consecutive integration runs have not been demonstrated.
- Full CI, Tauri platform packaging, Compose recovery/restore, security/failure tests and final release review must pass on the exact final commit.
- Production hosting, secret manager, monitoring and backup/restore objectives are not configured.

See `docs/READINESS.md` for per-work-item status and evidence. Do not claim production readiness, complete market coverage, profitability or safe real-money execution.
