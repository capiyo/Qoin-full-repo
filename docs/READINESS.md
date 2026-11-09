# Qoin readiness and completion evidence

**Candidate branch:** `qoin/integration-completion`  
**Last code-bearing commit:** `efbd78f11f37d543bfdac7faad2206507c19`  
**Current PR head:** `51aaa9fd6c836d66915d48d1cbdaa1f407f532ba` (documentation/workflow-path updates after the last code-bearing commit)  
**Base snapshot:** PR #15 head `280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7`  
**Safety posture:** real-money execution disabled by code policy; no merge to `main` and no production deployment.

Status meanings: **DONE** means implemented and verified on the exact candidate commit. **IN PROGRESS** means engineering work or verification remains. **BLOCKED** is reserved exclusively for an external exchange key, unavailable hardware, or a human decision/authorization. A missing local checkout, unavailable tool, unresolved conflict, or unfinished implementation is not a valid BLOCKED reason.

## Step 0 — Orientation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Inspect repository workspaces, crates, packages, CI, Dockerfiles and docs | DONE | Inspected root `Cargo.toml`, pnpm workspace layout, Axum/C++ manifests, Dockerfiles, Compose topology, workflow definitions, README, architecture, provider, risk and state docs | Full local build graph still needs a checked-out execution workspace |
| Inspect PRs #11, #12, #13 and #15 and choose integration line | IN PROGRESS | PR #15 is the base snapshot. PR #11 merged only into isolated `qoin/integration-chain` as `2d954aafb5f52ef8e2495316bdcc62466f6489d1`. PR #12 (`f6351787...`) and PR #13 (`33003c80...`) were each temporarily retargeted to `qoin/integration-completion`; GitHub reported `mergeable=false` for both. Their bases were restored to `main`. No commits from #12/#13 have been cherry-picked into this branch yet. | The conflicts must be reconciled by transplanting unique commits and resolving shared Axum, Drogon, Cargo, workflow, web and shared-core files. PR #15 is present as the base snapshot; #12/#13 remain unintegrated. Continue this as engineering work, not BLOCKED. |
| Run full pre-change verification baseline | IN PROGRESS | PR #15 head `280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7` had eight workflow runs queued (run IDs 42665961496, 42665961382, 42665961367, 42665961365, 42665961370, 42665961409, 42665962154, 42665961489) when inspected. Older SHA `14e6d9563f68ffca59cc43dd73ae18bdb1835b10` failed web typecheck with TS18047 at `App.tsx:461`: the guarded `decisionStrategy` branch rendered `strategy.updated_at_ms` instead of the narrowed `decisionStrategy.updated_at_ms`. The current candidate uses `decisionStrategy.updated_at_ms` within the non-null guard (current `frontend/apps/web/src/App.tsx`, strategy-map panel), so the nullability defect is corrected in source; web/mobile/core typecheck verification remains pending exact-head CI. Older SHA `d5360aa3bbb1516b13cc9cde993da890b83159a3` had Rust, engine, integration, Drogon, web and mobile PR checks green | No full baseline command suite was run before edits. On last code-bearing SHA `efbd78f11f37d543bfdac7faad2206507c19` (the current PR head adds only this readiness-document update), PR-triggered jobs 42669305114 (Rust workspace), 42669305203 (container), 42669305508 (Desktop), 42669305493 (Drogon), 42669305516 (live workspace), 42669305269 (engine), 42669305333 (React web), 42669305146 (React Native), and 42669305355 (Axum–Drogon–MongoDB integration) were all queued; push-triggered runs 42669291161 (React web) and 42669290783 (Rust workspace) were also queued. No job had started or produced command output when this report was updated. Current PR-head run set for `fd32bd0508d736fd464b62803cf4b35976a4d764`: 42672497645 (React web), 42672497872 (React Native), 42672497788 (engine/Python), 42672497775 (Drogon gateway), and 42672497678 (live workspace) are queued. Runs 42672497885 (Tauri desktop), 42672497674 (container), 42672497834 (Axum–Drogon–MongoDB integration) and 42672497973 (Rust workspace) have concluded failure with zero jobs exposed by the Actions API; no compile/test step output is available, so these are workflow-start failures, not test failures and not passes. Exact-head green verification is absent. |

## Phase 1 — Reliable foundation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Remove retired model runtime, sidecar, route, health field, Compose/env configuration and docs | IN PROGRESS | Commit `9ec10014c452b040c5f6c697afb4d14f9fb1b76f` removes the Axum sidecar/client/collection/handler, Drogon proxy, Compose and env references, model service, Python tests, Rust module, dedicated workflow and version document. A recursive Git tree scan found zero paths containing the retired model name; targeted runtime/documentation files had zero remaining text references. Repository-wide text scan and current-head Rust/C++/Python verification remain in progress. | Must verify a repository-wide case-insensitive search and pass Rust/C++/Python CI on the committed candidate |
| Correct execution/risk-policy contradictions | DONE | Rewrote `docs/RISK_POLICY.md` and `docs/STATE_OF_REPO.md` to distinguish existing adapter primitives from missing portfolio risk, reconciliation and lifecycle controls | Verify the final code/docs diff after PR consolidation |
| Reproduce and root-cause Drogon `std::length_error` | IN PROGRESS | Existing backlog records intermittent gateway process failure; later retry passed, but no reproducible failing trace or proven root cause was available. The candidate now bounds HTTP request bodies (1 MiB), upstream responses (4 MiB), WebSocket frames (64 KiB) and stream query parameters (4 KiB), rejecting oversized payloads before forwarding/allocation. The integration workflow now builds Drogon with ASan/UBSan, checks a 413 response for an oversized request and runs 20 repeated gateway health/signal/strategy/instrument/backtest cycles. These are defensive controls and planned CI assertions; no current-head run has executed them. | Root cause remains unproven. Capture a failing stack trace/core dump or deterministic reproducer; confirm the sanitizer build and 20-cycle loop execute and pass. A retry or a stress loop that does not reproduce the fault is not a root-cause fix. |
| Add regression test and run integration at least 20 consecutive times | IN PROGRESS | Existing `qoin-integration.yml` performs one integration sequence; no 20-run result exists | Add isolated repeat loop after root cause is known; preserve each run's exit code/log |
| Refresh stale README/architecture docs | DONE | Candidate README, architecture, state and version-map content rewritten for the fixed architecture and actual limitations | Validate links and statements against the final merged tree |
| Clean full-workspace CI baseline | IN PROGRESS | Historical CI is not exact-head evidence; latest PR #15 run set was queued | Must pass cargo build/test/clippy/fmt, gateway build/tests, web/mobile typecheck/lint/tests, Python tests, Tauri build checks, Compose and integration on exact candidate SHA |

## Phase 2 — Research platform

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Resumable, idempotent historical backfill with gap detection | IN PROGRESS | Provider docs and state report explicitly say no resumable backfill exists | Requires provider-specific historical APIs/entitlements, durable cursors, gap model, dedupe keys and restart tests |
| Provenance/completeness per instrument/provider | IN PROGRESS | Current registry/provider status is configuration-oriented, not a complete history/live coverage ledger | Requires persisted source metadata, freshness, gap and entitlement measurements |
| Provider failover and capability enforcement | IN PROGRESS | Current adapters: Binance USD-M has trades/book/funding/liquidations/OI; Coinbase and Kraken are trades-only; Finnhub is optional and plan/symbol-dependent | Automatic failover and complete live/historical coverage claims are not implemented; venue-isolated histories must be preserved |
| Portfolio accounting across currencies, correlated exposure, fees and funding | IN PROGRESS | Current paper account work is not complete portfolio-grade multi-currency accounting | Requires FX conversion source/time, cash ledger, financing/funding, fees, contract multipliers, covariance/correlation and exposure limits |
| Reproducible walk-forward/OOS and benchmarks | IN PROGRESS | Current replay is based on persisted events and has explicit cost assumptions; no complete walk-forward manifest/benchmark suite is verified | Requires immutable dataset/config versions, fixed seeds, chronological splits and benchmark reports |
| Independent algorithm runtime and durable selections | IN PROGRESS | Catalogue and entitlement domain primitives exist; independent runtime and durable user selections are not complete | Requires isolated state/config per algorithm, comparable metrics and verified UID-backed persistence |

## Phase 3 — User platform

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Firebase ID-token verification with JWKS cache, expiry, audience and issuer | IN PROGRESS | Repository state explicitly says Axum does not yet verify Firebase ID tokens for personalized routes | Requires backend verifier, key rotation/cache tests, issuer/audience config and tamper/expiry/replay tests |
| Server-side entitlements from verified UID | IN PROGRESS | Entitlement logic exists as a pure domain primitive; it is not connected to verified identity, persisted grants or payment reconciliation | Client claims must remain display-only; requires UID-derived lookup and refund/chargeback handling |
| Persist algorithm selection per user | IN PROGRESS | Shared TypeScript selection contracts exist; protected API and durable selection are not implemented | Requires verified UID, entitlement enforcement, version pinning and conflict semantics |
| Durable Archive and notification sync across web/mobile/desktop | IN PROGRESS | Client Firebase/profile foundations exist; PR #15 documents Archive as client-local and notification history as in-memory | Requires authenticated backend storage, sync cursors, device registration, idempotency and offline conflict handling |
| Complete web/mobile/desktop workflows | IN PROGRESS | UI foundations exist; current CI snapshots are not final-head verification and authenticated server flows are incomplete | Requires end-to-end auth, Explore, detail, Archive, notifications and paper session tests across all clients |

## Phase 4 — Execution infrastructure

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Real-money execution disabled by default and by code policy | IN PROGRESS | Candidate adds a live-mode rejection in `trading.rs`, a health status that reports live disabled, and unit tests for live rejection/testnet-only mode | Must run Rust tests and inspect every route/configuration path on final merged head |
| Wire portfolio-level risk validator into every order path | IN PROGRESS | `order_safety.rs` exists but is not integrated into `/internal/orders` | No order may bypass identity, portfolio exposure, loss, freshness and protective-order checks |
| Verify exchange-account ownership | IN PROGRESS | No verified account ownership flow is integrated | Requires authenticated account binding and exchange permissions proof |
| Durable order lifecycle submit/ack/partial fill/cancel/amend/reject | IN PROGRESS | Durable intent and duplicate client-order-ID protection exist; lifecycle module is not fully connected to adapter/store | Requires persisted state machine, transition constraints and crash-safe idempotency |
| Automatic reconciliation of orders, balances and positions | IN PROGRESS | Current reconciliation is operator-triggered and only queries order/protective-stop state | Requires background worker, leases, bounded retries, drift detection and safe repair policies |
| Kill switch manages existing exposure | IN PROGRESS | Existing Mongo-backed gate blocks new entries only | Must add tested cancel/flatten/position-management policy and independent emergency path |
| Account/position synchronization | IN PROGRESS | No full exchange account/position sync verified | Requires balances, positions, margin, funding and contract metadata reconciliation |
| Binance testnet end-to-end tests | IN PROGRESS | No exact-head testnet lifecycle evidence found | Requires testnet credentials stored in CI secrets and tests for all order transitions; never use production keys |

## Phase 5 — Release acceptance

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Full Rust/C++/JS/RN/Python/Tauri CI matrix | IN PROGRESS | Older PR run sets exist; current candidate SHA does not have completed verification evidence | Require all matrix jobs green on exact final PR head |
| Auth bypass, tampering, replay and injection tests | IN PROGRESS | No completed full backend identity boundary exists | Add adversarial auth, authorization and request validation tests |
| Provider/Mongo/gateway outage and restart recovery tests | IN PROGRESS | Event-log replay and some service restart integration checks exist; broad fault injection is incomplete | Add provider outage/failover, Mongo unavailability, gateway crash and idempotent recovery tests |
| Compose deployment and recovery verification | IN PROGRESS | Compose topology and container workflow exist; backup/restore drill and exact-head evidence are not established | Requires isolated Compose test, volume backup/restore and recovery assertions |
| Final operational readiness checklist | IN PROGRESS | This file records known gaps; operations/monitoring/backup/recovery are not complete | Requires measured SLOs, alerts, incident runbooks, backup/restore evidence and independent review |

## Required end-to-end trace

**Status: IN PROGRESS.** No automated test currently proves the complete chain:

`market-data ingestion → vector calculation → decision → paper execution → ledger → Archive`

The existing integration checks cover parts of ingestion, vector decisions, replay and operating-cycle persistence, but the complete cross-service, authenticated Archive path has not been verified as one test.

## External prerequisites genuinely BLOCKED

- **Binance Futures testnet lifecycle:** requires testnet API credentials stored in CI secrets; never use production keys.
- **Firebase identity integration tests:** require a Firebase project/test identities and authorized configuration supplied by the project owner.
- **Optional provider validation and commercial coverage:** Finnhub credentials/plan entitlements and a human decision confirming data licensing/redistribution rights for each provider/instrument.

These prerequisites do not block implementation of the corresponding interfaces, mocks, deterministic tests, or paper-mode workflows.

## Exact external prerequisites

- A repository-attached checkout/terminal (or equivalent CI workspace with interactive branch conflict resolution) is required to reconcile PRs #12/#13/#15 and run the full local command baseline. The available shell cannot reach github.com and there is no mounted checkout in this session. This is an execution-environment limitation, not an external project dependency; do not mark engineering work BLOCKED on this basis. Use CI or a repository-attached checkout to continue.
- Binance Futures testnet API credentials, stored as CI secrets, are required for real exchange lifecycle tests. Do not send credentials in chat.
- Firebase project configuration and test identities are required for ID-token/JWKS and cross-device authenticated workflow tests.
- Finnhub credentials and provider plan/symbol entitlements are required to validate optional live feeds/news/calendar; no key is committed.
- Data licensing and redistribution entitlements must be confirmed for every provider/instrument before claiming commercial coverage.

## Backlog policy

No unclassified work is considered complete. The backlog lists unfinished engineering as IN PROGRESS. BLOCKED is reserved for the external prerequisites below. Real-money mode must stay disabled; no merge to `main` or production deployment is authorized by this readiness report.
