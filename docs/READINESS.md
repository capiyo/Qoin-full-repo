# Qoin readiness and completion evidence

**Candidate branch:** `qoin/integration-completion`  
**Last code-bearing commit:** `efbd78f11f37d543bfdac7faad2206507c19fe12` (subsequent commits only update readiness evidence and CI trigger paths)  
**Base snapshot:** PR #15 head `280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7`  
**Safety posture:** real-money execution disabled by code policy; no merge to `main` and no production deployment.

Status meanings: **DONE** means implemented and verified on the exact candidate commit. **BLOCKED** means the work is not complete; the exact dependency or evidence required is listed. No item is marked DONE solely because documentation or an older CI run says it passed.

## Step 0 — Orientation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Inspect repository workspaces, crates, packages, CI, Dockerfiles and docs | DONE | Inspected root `Cargo.toml`, pnpm workspace layout, Axum/C++ manifests, Dockerfiles, Compose topology, workflow definitions, README, architecture, provider, risk and state docs | Full local build graph still needs a checked-out execution workspace |
| Inspect PRs #11, #12, #13 and #15 and choose integration line | BLOCKED | All four PRs target `main` and diverge from common base `79198891b6cabad760185f58a8f6f401c56240c1`; PR #11 merged to isolated `qoin/integration-chain` as `2d954aafb5f52ef8e2495316bdcc62466f6489d1`; PR #12 retargeted to that branch and GitHub rejected merge with HTTP 405: “Pull Request has merge conflicts” | Need a real repository checkout to reconcile shared Axum, Drogon, Cargo, workflow, web and shared-core files; PRs #12/#13/#15 are not consolidated |
| Run full pre-change verification baseline | BLOCKED | PR #15 head `280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7` had eight workflow runs queued (run IDs 42665961496, 42665961382, 42665961367, 42665961365, 42665961370, 42665961409, 42665962154, 42665961489) when inspected. Older SHA `14e6d9563f68ffca59cc43dd73ae18bdb1835b10` failed web typecheck with TS18047 at `App.tsx:461`; old logs identify `strategy` possibly null. Older SHA `d5360aa3bbb1516b13cc9cde993da890b83159a3` had Rust, engine, integration, Drogon, web and mobile PR checks green | No full baseline command suite was run before edits. On last code-bearing SHA `efbd78f11f37d543bfdac7faad2206507c19` (the current PR head adds only this readiness-document update), PR-triggered jobs 42669305114 (Rust workspace), 42669305203 (container), 42669305508 (Desktop), 42669305493 (Drogon), 42669305516 (live workspace), 42669305269 (engine), 42669305333 (React web), 42669305146 (React Native), and 42669305355 (Axum–Drogon–MongoDB integration) were all queued; push-triggered runs 42669291161 (React web) and 42669290783 (Rust workspace) were also queued. No job had started or produced command output when this report was updated. No completed run is attached to the current PR head `741c913d368ff10945c613760dd22779395add74`. |

## Phase 1 — Reliable foundation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Remove retired model runtime, sidecar, route, health field, Compose/env configuration and docs | BLOCKED | Commit `9ec10014c452b040c5f6c697afb4d14f9fb1b76f` removes the Axum sidecar/client/collection/handler, Drogon proxy, Compose and env references, model service, Python tests, Rust module, dedicated workflow and version document. A recursive Git tree scan found zero paths containing the retired model name, targeted runtime/documentation files had zero remaining text references, and GitHub code search returned zero matches. Rust/C++/Python CI on the final candidate is still queued, so compilation and behavioral verification remain blocked. | Must verify a repository-wide case-insensitive search and pass Rust/C++/Python CI on the committed candidate |
| Correct execution/risk-policy contradictions | DONE | Rewrote `docs/RISK_POLICY.md` and `docs/STATE_OF_REPO.md` to distinguish existing adapter primitives from missing portfolio risk, reconciliation and lifecycle controls | Verify the final code/docs diff after PR consolidation |
| Reproduce and root-cause Drogon `std::length_error` | BLOCKED | Existing backlog records intermittent gateway process failure; later retry passed, but no reproducible failing trace or proven root cause was available | Need failing stack trace/core dump or deterministic reproducer; a retry is not a fix |
| Add regression test and run integration at least 20 consecutive times | BLOCKED | Existing `qoin-integration.yml` performs one integration sequence; no 20-run result exists | Add isolated repeat loop after root cause is known; preserve each run's exit code/log |
| Refresh stale README/architecture docs | DONE | Candidate README, architecture, state and version-map content rewritten for the fixed architecture and actual limitations | Validate links and statements against the final merged tree |
| Clean full-workspace CI baseline | BLOCKED | Historical CI is not exact-head evidence; latest PR #15 run set was queued | Must pass cargo build/test/clippy/fmt, gateway build/tests, web/mobile typecheck/lint/tests, Python tests, Tauri build checks, Compose and integration on exact candidate SHA |

## Phase 2 — Research platform

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Resumable, idempotent historical backfill with gap detection | BLOCKED | Provider docs and state report explicitly say no resumable backfill exists | Requires provider-specific historical APIs/entitlements, durable cursors, gap model, dedupe keys and restart tests |
| Provenance/completeness per instrument/provider | BLOCKED | Current registry/provider status is configuration-oriented, not a complete history/live coverage ledger | Requires persisted source metadata, freshness, gap and entitlement measurements |
| Provider failover and capability enforcement | BLOCKED | Current adapters: Binance USD-M has trades/book/funding/liquidations/OI; Coinbase and Kraken are trades-only; Finnhub is optional and plan/symbol-dependent | Automatic failover and complete live/historical coverage claims are not implemented; venue-isolated histories must be preserved |
| Portfolio accounting across currencies, correlated exposure, fees and funding | BLOCKED | Current paper account work is not complete portfolio-grade multi-currency accounting | Requires FX conversion source/time, cash ledger, financing/funding, fees, contract multipliers, covariance/correlation and exposure limits |
| Reproducible walk-forward/OOS and benchmarks | BLOCKED | Current replay is based on persisted events and has explicit cost assumptions; no complete walk-forward manifest/benchmark suite is verified | Requires immutable dataset/config versions, fixed seeds, chronological splits and benchmark reports |
| Independent algorithm runtime and durable selections | BLOCKED | Catalogue and entitlement domain primitives exist; independent runtime and durable user selections are not complete | Requires isolated state/config per algorithm, comparable metrics and verified UID-backed persistence |

## Phase 3 — User platform

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Firebase ID-token verification with JWKS cache, expiry, audience and issuer | BLOCKED | Repository state explicitly says Axum does not yet verify Firebase ID tokens for personalized routes | Requires backend verifier, key rotation/cache tests, issuer/audience config and tamper/expiry/replay tests |
| Server-side entitlements from verified UID | BLOCKED | Entitlement logic exists as a pure domain primitive; it is not connected to verified identity, persisted grants or payment reconciliation | Client claims must remain display-only; requires UID-derived lookup and refund/chargeback handling |
| Persist algorithm selection per user | BLOCKED | Shared TypeScript selection contracts exist; protected API and durable selection are not implemented | Requires verified UID, entitlement enforcement, version pinning and conflict semantics |
| Durable Archive and notification sync across web/mobile/desktop | BLOCKED | Client Firebase/profile foundations exist; PR #15 documents Archive as client-local and notification history as in-memory | Requires authenticated backend storage, sync cursors, device registration, idempotency and offline conflict handling |
| Complete web/mobile/desktop workflows | BLOCKED | UI foundations exist; current CI snapshots are not final-head verification and authenticated server flows are incomplete | Requires end-to-end auth, Explore, detail, Archive, notifications and paper session tests across all clients |

## Phase 4 — Execution infrastructure

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Real-money execution disabled by default and by code policy | BLOCKED | Candidate adds a live-mode rejection in `trading.rs`, a health status that reports live disabled, and unit tests for live rejection/testnet-only mode | Must run Rust tests and inspect every route/configuration path on final merged head |
| Wire portfolio-level risk validator into every order path | BLOCKED | `order_safety.rs` exists but is not integrated into `/internal/orders` | No order may bypass identity, portfolio exposure, loss, freshness and protective-order checks |
| Verify exchange-account ownership | BLOCKED | No verified account ownership flow is integrated | Requires authenticated account binding and exchange permissions proof |
| Durable order lifecycle submit/ack/partial fill/cancel/amend/reject | BLOCKED | Durable intent and duplicate client-order-ID protection exist; lifecycle module is not fully connected to adapter/store | Requires persisted state machine, transition constraints and crash-safe idempotency |
| Automatic reconciliation of orders, balances and positions | BLOCKED | Current reconciliation is operator-triggered and only queries order/protective-stop state | Requires background worker, leases, bounded retries, drift detection and safe repair policies |
| Kill switch manages existing exposure | BLOCKED | Existing Mongo-backed gate blocks new entries only | Must add tested cancel/flatten/position-management policy and independent emergency path |
| Account/position synchronization | BLOCKED | No full exchange account/position sync verified | Requires balances, positions, margin, funding and contract metadata reconciliation |
| Binance testnet end-to-end tests | BLOCKED | No exact-head testnet lifecycle evidence found | Requires testnet credentials stored in CI secrets and tests for all order transitions; never use production keys |

## Phase 5 — Release acceptance

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Full Rust/C++/JS/RN/Python/Tauri CI matrix | BLOCKED | Older PR run sets exist; current candidate SHA does not have completed verification evidence | Require all matrix jobs green on exact final PR head |
| Auth bypass, tampering, replay and injection tests | BLOCKED | No completed full backend identity boundary exists | Add adversarial auth, authorization and request validation tests |
| Provider/Mongo/gateway outage and restart recovery tests | BLOCKED | Event-log replay and some service restart integration checks exist; broad fault injection is incomplete | Add provider outage/failover, Mongo unavailability, gateway crash and idempotent recovery tests |
| Compose deployment and recovery verification | BLOCKED | Compose topology and container workflow exist; backup/restore drill and exact-head evidence are not established | Requires isolated Compose test, volume backup/restore and recovery assertions |
| Final operational readiness checklist | BLOCKED | This file records known gaps; operations/monitoring/backup/recovery are not complete | Requires measured SLOs, alerts, incident runbooks, backup/restore evidence and independent review |

## Required end-to-end trace

**Status: BLOCKED.** No automated test currently proves the complete chain:

`market-data ingestion → vector calculation → decision → paper execution → ledger → Archive`

The existing integration checks cover parts of ingestion, vector decisions, replay and operating-cycle persistence, but the complete cross-service, authenticated Archive path has not been verified as one test.

## Exact external prerequisites

- A repository-attached checkout/terminal (or equivalent CI workspace with interactive branch conflict resolution) is required to reconcile PRs #12/#13/#15 and run the full local command baseline. The current session can inspect and edit GitHub files but cannot resolve arbitrary multi-file merge conflicts or execute repository commands locally; outbound DNS from the available shell is unavailable.
- Binance Futures testnet API credentials, stored as CI secrets, are required for real exchange lifecycle tests. Do not send credentials in chat.
- Firebase project configuration and test identities are required for ID-token/JWKS and cross-device authenticated workflow tests.
- Finnhub credentials and provider plan/symbol entitlements are required to validate optional live feeds/news/calendar; no key is committed.
- Data licensing and redistribution entitlements must be confirmed for every provider/instrument before claiming commercial coverage.

## Backlog policy

No unclassified work is considered complete. The only remaining backlog entries are the BLOCKED items above. Real-money mode must stay disabled; no merge to `main` or production deployment is authorized by this readiness report.
