# Qoin backend completion audit

Audit date: 2026-11-26  
Repository: `capiyo/Qoin-full-repo`  
Active branch: `qoin/backend-completion-auth`  
Pull request: [#32](https://github.com/capiyo/Qoin-full-repo/pull/32) — draft, based on `main`  
Base SHA: `0d9cba0ce8c88ca874c29c891a0785934d05aa17`  
Audited candidate SHA before the owner-isolated paper-session change: `bb16843ed2aedad4396bd387ecb1f36d705ad0c3`  
Current source head at this update: `f4b41c7778fc4bc28b93e5db8797d32ab1de9f8c`  
Verification: Rust workspace, Axum/Drogon/Mongo integration, Drogon gateway and engine checks passed on the earlier code candidate `843f99a24671416d21fbaeb875f1a05774567520`; newer risk/order lifecycle/rate-limit/shutdown changes and the explicit HTTP 429 integration assertion on `fb558542...` were queued for CI at this audit update.

> This audit records source findings and verified CI evidence. It does not describe the repository as production-ready. Do not interpret provider catalogues, adapter traits, test fixtures or paper simulations as live broker connectivity.

## Repository and architecture findings

- Rust workspace: `backend/rust-axum` contains `qoin-vector-engine`, `qoin-flow-engine` and `qoin-vector-motion`. The vector engine remains the quantitative signal authority; Axum owns orchestration and persistence; vector-motion is a visualization crate.
- Drogon: `backend/cpp-drogon` is the HTTP gateway and proxies typed internal Axum routes. It does not contain the quantitative engine.
- MongoDB: Axum persists raw market events, derived snapshots, decisions, orders, paper ledgers, users and broker connection records. Recovery replays the immutable market event log.
- Python: `backend/research` contains replay/backtest research tooling. Production decisions remain Rust-owned.
- JEPA: no active JEPA production or research pathway is part of this candidate. Do not reintroduce it.
- Frontend: web, React Native, shared core and Tauri applications remain outside this backend implementation scope. The current full-workspace checks also include unrelated frontend checks and must be reported separately.

## Component audit

| Component | Existing implementation verified in candidate | Missing / incomplete work | Required action | Verification |
|---|---|---|---|---|
| Vector engine | Rust `qoin-vector-engine` computes the existing seven-force signal from normalized `ForceObservation`; config and numerical validation are tested; Axum ingests events and persists versioned decisions | The full prompt's explicit Pulse/Attend variable contract, pressure-blade provenance model and immutable per-contribution audit schema are not yet evidenced as complete; mathematical naming must not be inferred from seven-force fields | Preserve the current algorithm; document exact existing mathematics and add provenance/replay only where it is mathematically faithful | Rust workspace and engine workflows |
| Axum API | Typed Axum handlers, Firebase token verification, Mongo persistence, health/readiness, paper session and private operator order routes | Firebase revocation lookup, uniform typed error envelope, comprehensive endpoint contract tests, stricter config validation and shutdown/recovery coverage remain | Complete API contract/security integration; do not bypass ownership checks | Rust workspace + Axum integration |
| Drogon | Gateway proxies health, market stream, broker connection and paper-session endpoints; backend CI compiles and exercises gateway | End-to-end WebSocket reconnection/backpressure and account-private event subscriptions need more tests; no private portfolio event fanout should be assumed | Keep gateway thin; add versioned event envelope and authorization tests if private event routes are introduced | Drogon and Axum/Drogon workflows |
| MongoDB | Raw event, state, decision, order, paper and broker collections; indexes; event-log replay; owner-scoped broker records | Multi-document transaction boundaries, immutable execution collection, full ledger reconciliation, retention policy, bounded retries and paper-session write recovery remain incomplete | Add idempotent repositories and reconciliation records; test crash windows and duplicate delivery | Axum/Mongo integration workflow |
| Authentication | Firebase JWT signature/JWKS, issuer, audience and time claims; Firebase UID ownership filters on broker and order handlers | Token revocation / disabled-user enforcement is not implemented by JWKS verification alone; production identity integration not exercised | Add Admin SDK/service-account revocation checks with bounded caching and fail-closed policy | Unit tests exist; production Firebase not exercised |
| Broker credentials | AES-256-GCM versioned envelope vault, key ID, key rotation support, owner-scoped create/list/rotate/revoke, ciphertext omitted from responses | Per-user credential decryption is not connected to provider adapters; provider-side credential validation and account discovery are absent; deployment key not supplied in CI | Keep all new connections pending validation; implement a real provider adapter before changing status | Vault unit tests; adapter integration unavailable |
| Broker adapters | Broker-neutral trait/contracts and explicit 20-provider directory; provider adapter types intentionally fail closed and are not registered | All twenty are scaffolds, not working provider APIs. No real authenticated account, quote, order, cancellation or execution adapter is ready | Implement providers individually from official API contracts and sandbox conformance tests; do not mark catalogue entries as ready | Scaffold fail-closed tests only |
| Market data | Multiple public market-data ingestion paths, normalized events, timestamps, provenance, deduplication and historical replay | Provider health/failover policy, instrument/session freshness matrix, full backfill scheduler and market-data quality alerting remain | Finish freshness policies and recovery; keep stale observations from driving decisions | Integration tests and Rust tests |
| Risk | `broker_contract::submit_risk_checked` fetches account/instrument/quote/capabilities and applies account-currency portfolio risk and margin preflight before adapter submission | The legacy `order_safety.rs` primitive from commit `3bef7b0c` is not the active broker submission gate; broader account kill switch and live adapter registration remain intentionally closed | Maintain one authoritative risk gate in the broker-neutral contract; reconcile/remove redundant legacy checks only after review | Rust unit tests; no adapter registered |
| Paper trading | Forward event-driven paper simulator uses the Rust `Signal`, observed bid/ask, fees and slippage; persistent snapshots and decision/trade mirrors | Previous implementation used one global `active` session, violating per-user isolation. Current changes use owner-scoped v2 collections and a UID-keyed session map; session pause/resume preserves unsettled positions, and owner-scoped trade/decision history is exposed through authenticated Axum/Drogon routes. Partial fills, liquidity participation, transactionally safe event replay and multi-currency portfolio aggregation remain | Verify new owner-scoped flow under integration tests; implement crash-safe event idempotency and realistic partial fills | Rust workspace passed after owner-scope change; Mongo integration pending |
| Order lifecycle | Broker-neutral normalized order, durable intent reservation, client-order idempotency, unknown-outcome reconciliation and execution lifecycle domain types | Full immutable execution history and complete state-transition event store are not yet connected end-to-end; live dispatch is unavailable because registry is empty | Persist each state transition and reconcile provider state before any retry | Rust tests; no provider dispatch |
| Backtesting | Rust replay uses the production vector engine; chronological event ordering; later-event fills; partial fills; participation caps; latency; fees; slippage; market impact; funding; stress scenarios; annualized return, volatility, Sharpe, Sortino, Calmar, drawdown, completed-trade win rate, profit factor, exposure, costs and buy-and-hold comparison. Python signal/backtest clients delegate to Rust | Walk-forward orchestration, out-of-sample leakage checks, parameter sensitivity, multi-currency portfolio valuation and externally selected benchmark attribution remain incomplete | Add reproducible experiment manifests and validation splits without creating a Python decision/fill engine | Rust/Python exact-head CI pending; previous Rust replay tests passed |
| Events / WebSockets | Broadcast market updates and authenticated market WebSocket path; gateway proxies Axum stream | User-private account/order event topics, durable event replay cursor, explicit backpressure contract and cross-service schema version tests remain | Add private topics only with UID/account ownership checks and durable sequence cursors | Gateway CI; private event tests pending |
| Infrastructure | Docker Compose, Axum and Drogon images, Mongo initialization and container workflow | Production TLS termination, secret-manager wiring, observability/alerting, backup restore drills and deployed environment validation remain | Complete environment-specific hardening and recovery drills | Container workflow on current candidate |
| Security | Server-side secrets, encrypted credential persistence, owner filters, operator-only order entry, default-off kill switch | Firebase revocation, CSRF/CORS deployment review, request body limits, rate-limit coverage, dependency audit and threat-model verification remain | Complete security test matrix before production use | Partial automated coverage |

## Paper-session ownership correction in current candidate

The previous paper-session implementation stored one global `session_key=active` document and used one in-memory `PaperSession`. Although the GET route required Firebase authentication, every authenticated user could receive that same snapshot. That was an ownership defect.

Commit `19f7a2a...` changes the active model:
- one in-memory session per verified Firebase UID;
- session reads, starts and stops resolve the Firebase UID server-side;
- MongoDB persists each session under `owner_uid` and UID-derived `session_key`;
- separate v2 paper-session, decision and trade collections prevent collisions with legacy global unique indexes;
- decision and trade upserts include owner UID in their idempotency filters;
- startup recovery reconstructs the UID-keyed session map.

The legacy unscoped collections are not reused for new user data. This is an additive storage migration; existing global legacy records are retained but are not exposed as any user's account. Integration CI must validate the current implementation before this correction is considered verified.

## Broker provider scope

The twenty-provider directory is a selection catalogue only: ten crypto venues and ten FX/CFD brokers. Adapter types are deliberately unregistered and return unsupported errors. No provider is execution-ready. See `docs/BROKER_PROVIDER_SCOPE.md` for the provider list and adapter acceptance gate.

## Safety posture

- Live-money execution remains disabled by default.
- The operator order route requires independent operator authorization, Firebase identity, an active owner-mapped broker account, environment authorization, a registered adapter, and broker-neutral risk preflight.
- The current registry is empty, so no provider can be dispatched through the active path.
- New broker connections remain `pending_validation`; credential encryption is not equivalent to provider authentication.
- Paper fills are simulations and are not broker executions.
- No production deployment, real broker sandbox, production Firebase revocation check, or live-money trade was performed in this audit.

## CI evidence

- Rust workspace on `19f7a2a080993be15b172ae811dd09f74eccbfb7`: **passed**.
- Axum/Drogon/Mongo integration on the same SHA: **running at time of update**.
- Qoin engine checks, Drogon gateway and container build on the same SHA: **running at time of update**.
- Previous candidate `bb16843...`: Rust workspace, Axum/Drogon/Mongo integration, Drogon gateway, engine checks and container build passed. React Native, desktop and live-workspace composite checks failed; the mobile failure was missing React type declarations and desktop failures were frontend build failures, not evidence of a backend failure.
- Exact current-head workflow status must be rechecked before merging.

## Release gate

Do not mark the backend production-ready until all of these pass:
1. owner-isolated paper-session integration tests with two distinct Firebase UIDs;
2. Mongo crash/restart and duplicate-event recovery tests for paper ledger writes;
3. Firebase revocation and disabled-user enforcement;
4. at least one fully implemented, sandbox-tested provider adapter end to end;
5. durable order state transitions, execution immutability and broker reconciliation;
6. risk tests connected to the exact dispatch path;
7. market-data stale/failover and event replay tests;
8. WebSocket ownership, reconnect and backpressure tests;
9. backup/restore, secrets rotation, TLS and deployment verification;
10. current-head CI green for all backend workflows.

## Additional hardening after the paper-session ownership correction

- The legacy `order_safety::validate_order` from commit `3bef7b0c` is now called by `broker_contract::submit_risk_checked` for opening live orders after quantity and price are normalized into account-currency exposure. The Decimal portfolio risk gate remains authoritative for FX conversion, broker margin and instrument semantics. All five independent live-approval environment flags default to false.
- Broker submission errors are separated into pre-dispatch rejection versus unknown submission outcome. Only the latter is marked reconciliation-required; raw provider error strings are not returned to clients.
- Added an append-only `order_lifecycle_events` collection with owner/order/state idempotency and timeline indexes. Submission reservation, acceptance, preflight rejection and reconciliation outcomes are persisted as lifecycle events. Broker acknowledgement is validated through the existing `execution_lifecycle::OrderLifecycle` state machine before it is marked accepted.
- Added bounded per-peer Drogon HTTP fixed-window limits: 120 reads and 30 writes per 60 seconds, with a bounded peer table and 429/Retry-After response. WebSocket connection-attempt rate limiting remains outstanding; stream backpressure already emits an explicit gap message.
- Added Axum graceful shutdown handling for Ctrl-C and SIGTERM.
- The authenticated paper session can be resumed using the same cycle ID. A new cycle cannot replace an active session, and cannot discard a paused session with an open position or pending action.

## Latest verification boundary

The earlier candidate `843f99a...` passed Rust workspace, Axum/Drogon/Mongo integration and Drogon gateway workflows. The newer changes to dual risk gating, order lifecycle persistence, rate limiting, session resume and graceful shutdown are not yet covered by a completed CI run at the time of this audit. Current branch workflows for `0fe1913...` were queued. Do not promote these newest changes until those exact-head workflows finish successfully.

The integration workflow now also sends enough write requests to verify the gateway returns HTTP 429 after the configured per-peer write threshold. This assertion is awaiting execution on the current head.

## V1 mathematics and Python parity update

- The Rust V1 seven-force algorithm was preserved; no coefficients, clipping, top-k selection, volatility damping, spread gate or BUY/SELL/CLOSE/WAIT rule was changed.
- `Signal.attend` now exposes the existing row-softmax Attend matrix while retaining `attention` as a backward-compatible alias.
- `Signal.pulse` exposes the existing signed pressure-blade decomposition as bullish and bearish magnitudes. Each feature trace now records Attend weights, positive/negative Pulse, and its signed pressure-blade contribution. The interaction term remains separately attributed and is included once in the net Pulse.
- `docs/VECTOR_ENGINE_MATHEMATICS.md` records the actual equations, validated domains and relationship to the existing score. Tests assert Attend row consistency, unique feature provenance and Pulse/pressure-blade reconciliation.
- Python `research.vector_model.evaluate_signal` is now a thin bridge to Axum `POST /v1/signal`; the independent Python decision calculation was removed. Python tests verify request mapping/delegation. Rust remains the only decision implementation.
- OpenAPI now describes the Rust signal request/response and the explicit Attend/Pulse/pressure-blade diagnostics.

Exact-head Rust and Python CI for these latest schema/diagnostic changes is pending.

## Research parity and standalone API update

- Python `research.vector_model.evaluate_signal` now calls the production Rust `/v1/signal` endpoint and contains no duplicate decision coefficients or rules.
- Python `research.backtest_client.run_rust_backtest` delegates chronological event replay and execution assumptions to Rust `/internal/backtest`; Python is a client/research layer, not a second simulator.
- Python tests cover the bridge request contract, validation and response handling.
- The separate `qoin-axum-api` binary is documented and hardened as a loopback-only research server on port 8082 by default, with a 64 KiB body cap and graceful shutdown. The production Axum/Mongo service remains `qoin-flow-engine` on private port 8081 behind Drogon.
- The production Axum router has a one MiB request body cap and graceful shutdown. Drogon rate limits HTTP proxy requests and WebSocket connection attempts.

## Remaining quantitative definition boundary

The V1 engine now explicitly exposes Attend, Pulse and pressure-blade contributions using the existing V1 mathematics. The current seven-force implementation has an angular alignment/angle diagnostic, but no separately specified vector-spacing metric or independent evolving vector-state dynamics. Those must not be invented or silently substituted; the exact V1 angular measure is documented in `VECTOR_ENGINE_MATHEMATICS.md`. If vector spacing means a distinct quantity, it requires the user's established formula before implementation.

## Dead-code and dependency cleanup

- Removed unreferenced `backend/rust-axum/src/trading.rs`, the uncompiled Binance-only legacy trading adapter.
- Removed unreferenced `backend/rust-axum/src/vector_engine.rs`, the uncompiled six-force Python-era duplicate; the shared `qoin-vector-engine` crate remains the only Rust signal implementation.
- Removed unreferenced `backend/rust-axum/src/operating_cycle.rs`, a disconnected cycle prototype that was not declared as a module or wired to the active paper-session service.
- Removed the now-unused direct `hmac` dependency from `backend/rust-axum/Cargo.toml`; HMAC functionality remains available transitively where `jsonwebtoken` requires it.
- Rechecked the active `main.rs` module declarations against `backend/rust-axum/src/*.rs`; remaining Rust source modules are declared and used by the production service.

## Quantitative research metrics update

The Rust replay now calculates annualized return, annualized dailyized volatility, Sharpe and Sortino ratios (zero risk-free rate), Calmar ratio, max drawdown, completed exit-order win rate, net winning/losing trade P&L, profit factor, maximum gross exposure, transaction costs and a same-instrument buy-and-hold comparison. Unsupported/degenerate ratios serialize as `null`, not infinity. The backtest client remains a thin Python transport wrapper.

Backtest metric definitions are now embedded in each Rust `ReplayResult.assumptions`: calendar-day annualization (365.25), zero risk-free rate for Sharpe, dailyized calendar returns, completed SELL-order aggregation for win rate/profit factor, net realized P&L after entry/exit fees, and same-instrument first-to-last mark buy-and-hold comparison. Unsupported ratios are represented as null. Exact-head verification remains pending.
