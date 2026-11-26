# Qoin Backend Completion Audit

Audit date: 2026-11-26  
Repository: `capiyo/Qoin-full-repo`  
Audited ref: `main` at `0d9cba0ce8c88ca874c29c891a0785934d05aa17`  
Working branch: `qoin/backend-completion` (created from `main`)  
Scope: backend source, build configuration, deployment, research, and CI. This is a source inspection; no local build or test execution is claimed.

## Executive finding

The default branch is a research/paper foundation, not a production-ready multi-broker backend. The repository contains useful deterministic signal code, Axum services, MongoDB event persistence, a small Drogon HTTP proxy, and Python reference tests. It does not yet demonstrate the complete authenticated, user-isolated, durable, broker-neutral paper-execution system described in the completion protocol.

Several other branches contain substantial candidate implementations. They must be reviewed and selectively reconciled rather than assumed to be present on `main` or merged wholesale. The most relevant inspected branch snapshots include:
- `qoin/full-integration-rework-paper` at `dfad6131ef6136e662d52d78db2a67e9635b8d15`: adds paper sessions, order-safety primitives, execution lifecycle, operating cycle, backtest code, expanded market-data/vector crates, and removes JEPA code. Its changes also include frontend and deployment changes; it is not a backend-only drop-in.
- `qoin-equal-broker-routing-policy` at `c9ee82b98655ddee7b8c4b887ed7ad19b421b9c3`: adds broker-neutral instrument/account/order contracts, allocation policy, and a set-ledger projection. It is divergent from `main` and must be reconciled carefully.
- `feat/dynamic-broker-registry` at `33003c80660853e5b5cd125abf9fa2901a1b8cc9`: contains broker-registry work and paper/backtest additions in its history; adapter completeness and provider claims still require source-level verification.

## Repository and branch state

| Item | Verified finding | Required action |
|---|---|---|
| Default branch | `main` | Keep as the integration target unless explicitly changed |
| Main HEAD | `0d9cba0ce8c88ca874c29c891a0785934d05aa17` | Preserve as the base |
| Main latest commit | 2026-11-23, event-time historical vector lookup and causal ordering fix | Preserve this change when reconciling older feature branches |
| Working branch | `qoin/backend-completion`, created from `main` | Use for isolated changes; do not force-push or rewrite history |
| Working tree | Not observable through the GitHub repository API | Local uncommitted changes cannot be assessed remotely |

## Component audit

| Component | Existing implementation on audited `main` | Missing or unverified work | Required action | Verification available |
|---|---|---|---|---|
| Workspace | Root Cargo workspace includes `qoin-vector-engine`, `qoin-axum-api`, and `qoin-flow-engine` | No lockfile was found in the inspected root tree; full pinned reproducibility not established | Add/maintain lockfile and pinned CI toolchain | Source/manifests inspected; no local cargo run |
| Vector engine | Deterministic `ForceObservation` to `Signal` function, configurable clipping/top-k/alignment and finite-input/config validation | No complete persisted vector-state abstraction, full Pulse/Attend definitions, or pressure-blade provenance pipeline demonstrated in this crate | Reconcile the expanded market-data/multi-horizon work from feature branches; add explicit state, provenance, versioning and replay tests | Existing source inspected; tests not run |
| Flow engine | Axum service, MongoDB client/collections, event validation, in-memory symbol state, strategy/vector endpoints, event-time historical lookup | Authentication and user context are not demonstrated; durable repository abstraction, complete restart reconstruction, order/risk orchestration and production readiness are not established | Split typed services/repositories, add authentication/authorization, replay and state recovery, readiness and graceful shutdown | Source/docs inspected; no runtime test |
| Research API | Separate `qoin-axum-api` exposing `/health` and `/v1/signal`; loopback bind; research mode and live trading false | No auth or portfolio/execution integration; this is a signal API, not the authoritative production orchestration service | Clarify service boundary; avoid duplicate production decision pathways | Source inspected; no runtime test |
| Drogon | Minimal HTTP proxy for health, state, vector, events, and strategy | No WebSocket implementation, Firebase token verification, ownership propagation, subscription authorization, rate limiting or backpressure demonstrated | Implement gateway middleware and authenticated WebSocket/event delivery; keep quantitative/risk logic in Rust | Source/CMake inspected; native build not run |
| MongoDB | MongoDB driver in flow engine; collections for events, states, decisions and strategies; local Mongo Compose service | No demonstrated typed repository boundary, full schema/index/retention policy, tenant ownership model, transactional order/execution ledger, or verified restart recovery | Implement typed persistence, unique/idempotency indexes, ownership-scoped queries, atomic lifecycle updates and replay recovery | Source/Compose/docs inspected; no Mongo integration run |
| Authentication | Firebase is referenced by frontend/repository documentation | Main backend route code does not demonstrate verified Firebase ID tokens, issuer/audience/expiry/revocation validation or user context enforcement | Add server-side Firebase verification and enforce identity on every private API and stream | Not implemented/verified on audited main |
| Broker registry/adapters | No broker-neutral adapter module or provider registry was found in the audited main backend tree | Real adapters, credential lifecycle, account discovery, capabilities, market data and order APIs not demonstrated | Review broker-registry and broker-contract branches; implement provider-backed adapters and truthful capability reporting | Branch candidate identified; no live credentials/sandbox |
| Credential security | Environment-based configuration exists | No verified AES-256-GCM broker credential vault, key rotation, user-scoped credential access or redaction policy | Implement server-side envelope encryption and ownership-bound access; secrets via deployment secret manager | Not verified |
| Market data | Event ingestion and normalized event fields exist; main commit adds source event time and historical vector lookup | No complete provider failover, historical backfill, instrument master, data freshness policy or provider health management demonstrated | Add normalized provider adapters, event dedupe, causal ordering and freshness gates | Source/docs inspected; provider tests not run |
| Risk | No complete risk gate is wired into an order submission path on main; live order route is not wired | Position/exposure/leverage/drawdown/loss limits, kill switch, protective exits and authorization chain are incomplete | Reconcile `order_safety.rs` candidate, connect it to paper order creation and preserve live-disabled behavior | Candidate source inspected on another branch; no tests run |
| Order lifecycle | No durable broker order state machine demonstrated on main | Idempotency, unknown-submit recovery, immutable execution evidence and broker reconciliation incomplete | Reconcile lifecycle candidate; persist state transitions and execution IDs before any external submission | Candidate source inspected on another branch |
| Paper trading | No persistent paper session engine on main | Full fill simulation, partial fills, costs, P&L, cancellation, reconciliation and restart recovery absent from main | Reconcile paper-session branch; verify forward-only fills against observed quotes/trades and realistic liquidity/cost assumptions | Candidate source inspected on another branch; no tests run |
| Backtesting | Python reference model and vector-dynamics tests exist | Production Rust engine reuse, event-driven execution, walk-forward/OOS, leakage controls and costs are not established on main | Reconcile Rust replay/backtest candidate and call the same Rust decision code from Python through a stable binding/service | Python tests present; not executed |
| Real-time events | Main event ingestion and internal HTTP reads exist | Authenticated WebSockets, event schema versioning, durable offsets, replay, backpressure and user/account filtering absent or unverified | Define versioned event envelopes and authorized subscription/recovery protocol | Not verified |
| Vector motion | No separate `qoin-vector-motion` crate on main | Visual-only component boundary not present on main | Bring over only if needed; it must never authorize decisions | Candidate exists on feature branch |
| JEPA retirement | Main still contains `backend/jepa-world-model`, `backend/rust-axum/src/jepa.rs`, JEPA state fields and a JEPA CI workflow | Retired code remains in the default branch | Remove all active JEPA code, modules, dependencies, workflow and stale docs after confirming no supported consumer remains | Direct tree/source inspection |
| Infrastructure | MongoDB 7 local Compose service with persistent volume and loopback port binding | No complete production deployment topology, service secrets, authenticated Mongo app user, service health/readiness policy or verified backup/restore | Reconcile hardened Compose/Docker assets and document secure production operations | Compose inspected; no container run |
| CI | Rust, Drogon and integration workflow files exist | CI status and actual current-main run outcomes were not verified; native Drogon build and full Mongo integration are not established | Run all workflows on the working branch and repair failures | Workflow source inspected; no workflow execution |
| Security | Some loopback defaults and no live-order route on main | AuthN/AuthZ, CORS policy, rate limits, secrets, WebSocket security, audit trail and dependency audit incomplete | Complete threat model and automated security tests | No security scan run |

## High-priority source-level defects and risks

1. **JEPA remains active on main.** The flow engine imports the JEPA module, carries JEPA client/history/prediction state, and exposes JEPA status. The repository also contains the JEPA research package and CI workflow. This conflicts with the retired-architecture requirement.
2. **Drogon is only a small HTTP proxy.** Its current source forwards selected HTTP methods to Axum; it does not implement the required authenticated WebSocket gateway.
3. **No verified backend Firebase authentication boundary exists on main.** Frontend Firebase support is not a substitute for server-side token verification.
4. **MongoDB is not yet a complete financial ledger.** Raw collections and event/state documents do not establish immutable executions, idempotent order persistence, ownership filters, or full restart recovery.
5. **No real broker-neutral adapter registry is present in the audited main backend tree.** Other branches contain candidate contracts and registry work, but configuration listings or interfaces alone do not prove live provider adapters.
6. **No order submission route is wired on main.** This is a useful safety property, but it also means risk, lifecycle and paper-execution work is not complete.
7. **The separate signal API is research-only.** It must not become a second production decision authority alongside the flow engine.

## Required integration and verification sequence

1. Preserve main's event-time/historical-vector commit.
2. Reconcile the paper-session, order-safety, execution-lifecycle, operating-cycle and Rust backtest implementations from the paper integration branch.
3. Reconcile broker-neutral contracts and allocation/routing policies without importing duplicate or conflicting execution logic.
4. Remove JEPA pathways repository-wide.
5. Establish Firebase identity propagation from Drogon to Axum and enforce user/account ownership in Mongo queries and WebSocket subscriptions.
6. Complete Mongo persistence, durable order state transitions, execution ledger, idempotency and restart recovery.
7. Complete paper trading end to end; keep live-money submission disabled.
8. Integrate real market-data adapters and broker adapters only where provider APIs/capabilities are documented and testable.
9. Run Rust formatting/tests, Python tests, Drogon native build, Mongo integration, contract tests, security checks and full CI on the working branch.
10. Update readiness documentation from observed test evidence only. Do not label the system production-ready while external provider credentials, deployment secrets, sandbox access or independent financial safety review remain unavailable.

## Verification status

- GitHub repository metadata, branch heads, commit metadata, recursive trees and selected source files were inspected.
- No local working-tree status is available through the repository API.
- No Cargo, Python, CMake, Docker, MongoDB or GitHub Actions test command was executed during this source inspection.
- No live broker credentials or provider sandbox were available to verify external integrations.
- This audit intentionally makes no claim that unexecuted tests pass.
