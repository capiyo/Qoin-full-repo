# Qoin implementation progress

Updated: 2026-11-17 (repository audit)

## Current implementation snapshot

The primary feature branch is PR #15, `qoin-archive-explore-vector-ui`, currently a draft against `main`. Audited head: `118f3404770259ae65d3b98770dbe035fd1475a7`. The latest observed PR-triggered runs were queued, not passed. No merge or deployment is recorded.

### V1 vector engine
- Deterministic Rust vector decision engine remains authoritative.
- Per-decision transparency includes source event identity/time, raw inputs, feature transformation and normalization, weighted bullish/bearish contributions, parameter snapshot and gate diagnostics.
- Web and React Native instrument detail/card views expose the V1 engine state and calculation trace.
- No independent profitability validation is established.

### V2 set / measure / relation ledger
- Typed finite sets, counting measures, observed trade-size measures and missing-size counts are implemented.
- Bounded temporal-precedence relations expose edge counts, lag statistics, sample truncation and interpretation warnings; temporal precedence is not causation.
- Durable append-only observation/relation records include deterministic identities and source-event SHA-256 fingerprints.
- Operator-only resumable backfill, reconstruction/hash checks, bounded integrity audit and correction proposals exist.
- V2 remains descriptive and non-authoritative. It must not modify V1 decisions or submit orders.
- Remaining: pagination for large ledger views, property-based invariants, and maintenance/recovery runbooks.

### Market data and research
- Binance USD-M streams trades, best bid/ask, mark/funding, liquidation events and open-interest polling.
- Coinbase Advanced Trade and Kraken Spot currently provide public spot trades.
- Finnhub provides optional authenticated trade/news/calendar data subject to key, plan and symbol entitlements.
- Event replay includes strict causal fill timing, venue isolation, partial fills, latency, liquidity participation, impact and cost-stress reporting.
- Remaining: complete historical backfill, immutable dataset versions, full symbol master, provider failover/SLOs, and complete order-book depth where supported/licensed.
- Portfolio accounting, cross-currency exposure, correlated risk, benchmark-relative evaluation, walk-forward and out-of-sample validation remain incomplete.

### Brokers and execution safety
- Decimal equal-notional allocation supports up to 15 selected account targets, per-account native quantity conversion, quantity-step round-down and unallocated residual.
- Provider-neutral broker contracts and deterministic risk/lifecycle primitives exist.
- Multi-broker account linking, credential vault, real adapters, parent/child persistence, concurrent fan-out, aggregate risk reservation and automatic reconciliation are not implemented end-to-end.
- The private Binance USD-M execution path is separate, operator-gated and not exposed through the public Drogon gateway. Keep live mode disabled.
- A routing plan or allocation result is not a submitted order.

### Application and clients
- Web and React Native Firebase client authentication and UID-scoped Firestore profile/Archive rules exist; Firebase project configuration is still required for real account validation.
- Axum server-side Firebase ID-token verification for future personalized endpoints remains outstanding.
- Archive cross-device synchronization and durable per-user notification queue/push delivery remain outstanding.
- Tauri desktop client and cross-platform packaging workflows exist; final-head matrix must be verified.
- Algorithm catalogue and entitlement domain rules are preview/domain foundations; independent runtimes, protected selection APIs and payment reconciliation are outstanding.
- JEPA code and workflows are recorded as removed from the active feature branch. Repeat repository-wide search during final acceptance.

## Current CI verification

Eight PR-triggered workflows were queued for the audited head:
- Qoin live workspace checks
- Qoin engine checks
- Qoin Rust workspace
- Qoin container build
- Desktop
- Qoin React Native
- Qoin React web
- Qoin Axum–Drogon–MongoDB integration

Queued runs do not establish successful builds. Record each final conclusion against the exact commit before closing any release gate. The GitHub connector did not expose local checkout access in this session, so no local shell test result is claimed.

## Active backlog

See root `BACKLOG.md` for the dependency-ordered work plan. Highest priorities are:
1. Stabilize Axum–Drogon–MongoDB integration and diagnose intermittent gateway failures.
2. Complete secure multi-broker account linking and equal-share parent/child execution foundations.
3. Finish V2 ledger pagination, property tests and maintenance runbooks.
4. Build historical market-data completeness and portfolio-grade evaluation.
5. Complete backend identity verification, durable Archive/notifications and cross-client release verification.

## Release boundary

Qoin remains research/paper-trading by default. Do not claim profitability, complete market coverage, production readiness or safe unattended execution. Do not merge, deploy, activate real-money trading or enable live broker credentials without explicit approval and independently verified release gates.
