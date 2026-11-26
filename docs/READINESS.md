# Qoin readiness and completion evidence

**Active integration branch:** `qoin/backend-completion-auth`  
**Current PR:** #32 (draft; based on `main`)  
**Base:** `main` at `0d9cba0ce8c88ca874c29c891a0785934d05aa17`  
**Current branch head at this update:** `9051312b816ff496abdeb30d22af3db451f18ab7`  
**Safety posture:** live-money execution remains disabled by code policy; no merge to `main` and no production deployment has been performed.

Status meanings: **DONE** means implemented and verified on the exact candidate commit. **IN PROGRESS** means engineering work or verification remains. **BLOCKED** is reserved exclusively for an external exchange key, unavailable hardware, or a human decision/authorization. A missing local checkout, unavailable tool, unresolved conflict, or unfinished implementation is not a valid BLOCKED reason.

## Step 0 — Orientation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Inspect repository workspaces, crates, packages, CI, Dockerfiles and docs | DONE | Inspected root `Cargo.toml`, pnpm workspace layout, Axum/C++ manifests, Dockerfiles, Compose topology, workflow definitions, README, architecture, provider, risk and state docs | Full local build graph still needs a checked-out execution workspace |
| Inspect PRs #11, #12, #13 and #15 and choose integration line | IN PROGRESS | PR #15 is the base snapshot. PR #11 merged only into isolated `qoin/integration-chain` as `2d954aafb5f52ef8e2495316bdcc62466f6489d1`. PR #12 (`f6351787...`) and PR #13 (`33003c80...`) were each temporarily retargeted to `qoin/integration-completion`; GitHub reported `mergeable=false` for both. Their bases were restored to `main`. PR #12 has six clean source commits cherry-picked (multi-horizon engine, causal aggregation, exports, event schema and research docs); its market-backed Axum endpoint and Drogon `GET /api/v1/market-horizons` proxy are also ported, with a bounded Mongo query and an integration assertion. PR #13 has four clean source commits cherry-picked (broker registry, paper-session model, operating-cycle model and replay docs), plus two documentation follow-up commits; the current PR #13 module snapshots were ported and declared in Axum, and the paper-session test fixture was adapted to the current EdgeDecision contract. PR #13 now also has Mongo-backed cycle and paper-session snapshots, Axum configure/start/stop/snapshot routes, Drogon proxies, ingestion-driven cycle diagnostics, and paper Mirror/trade writes during the paper-session phase. Recovery after partial writes, session replay, Archive lifecycle and client controls remain incomplete. The CI-based cherry-pick workflow run 42673468437 failed before job creation and contributed no changes. Remaining overlapping files are still being reconciled. | The conflicts must be reconciled by porting the remaining Axum endpoints, Drogon routes, market asset-class isolation, Mongo persistence, web/mobile flows and integration tests. PR #15 is present as the base snapshot; #12/#13 are only partially integrated. Continue this as engineering work, not BLOCKED. |
| Run full pre-change verification baseline | IN PROGRESS | PR #15 head `280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7` had eight workflow runs queued (run IDs 42665961496, 42665961382, 42665961367, 42665961365, 42665961370, 42665961409, 42665962154, 42665961489) when inspected. Older SHA `14e6d9563f68ffca59cc43dd73ae18bdb1835b10` failed web typecheck with TS18047 at `App.tsx:461`: the guarded `decisionStrategy` branch rendered `strategy.updated_at_ms` instead of the narrowed `decisionStrategy.updated_at_ms`. The current candidate uses `decisionStrategy.updated_at_ms` within the non-null guard (current `frontend/apps/web/src/App.tsx`, strategy-map panel), so the nullability defect is corrected in source; web/mobile/core typecheck verification remains pending exact-head CI. Older SHA `d5360aa3bbb1516b13cc9cde993da890b83159a3` had Rust, engine, integration, Drogon, web and mobile PR checks green | No full baseline command suite was run before edits. The source correction for TS18047 is present: the strategy panel now reads `decisionStrategy.updated_at_ms` within the non-null branch. A strict standalone `frontend/packages/core/tsconfig.json` was added, and both web and React Native workflows invoke it before their app typechecks. The latest full matrix on source-bearing SHA `0f033cc5aede9065bf0638f508110df19e0e9e5d` queued run 42675753300 (Axum–Drogon–MongoDB integration), 42675753818 (React Native), 42675753817 (Rust), 42675754105 (container), 42675753281 (Desktop), 42675753259 (engine/Python), 42675753781 (Drogon), 42675753277 (live workspace), and 42675753429 (React web). At last poll, these jobs were queued; the integration run exposed one queued job, but no completed output. The current branch head has since advanced to `5fa8526d776a7e1bbac78fc35582224bfa02a6ad` with a backlog-only documentation update. Prior workflow runs on the same branch have repeatedly concluded failure with zero jobs exposed by the Actions API (including integration and cherry-pick automation); no compile/test step output is available. These are not passes and do not identify source-level test failures. Exact-head green verification is absent. |

## Phase 1 — Reliable foundation

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Remove retired model runtime, sidecar, route, health field, Compose/env configuration and docs | IN PROGRESS | Commit `9ec10014c452b040c5f6c697afb4d14f9fb1b76f` removes the Axum sidecar/client/collection/handler, Drogon proxy, Compose and env references, model service, Python tests, Rust module, dedicated workflow and version document. A recursive Git tree scan found zero paths containing the retired model name; targeted runtime/documentation files had zero remaining text references. Repository-wide text scan and current-head Rust/C++/Python verification remain in progress. | Must verify a repository-wide case-insensitive search and pass Rust/C++/Python CI on the committed candidate |
| Correct execution/risk-policy contradictions | DONE | Rewrote `docs/RISK_POLICY.md` and `docs/STATE_OF_REPO.md` to distinguish existing adapter primitives from missing portfolio risk, reconciliation and lifecycle controls | Verify the final code/docs diff after PR consolidation |
| Reproduce and root-cause Drogon `std::length_error` | IN PROGRESS | Historical failure is now directly evidenced by workflow run [42514628818](https://github.com/capiyo/Qoin-full-repo/actions/runs/42514628818), attempt 1, job [128834833613](https://github.com/capiyo/Qoin-full-repo/actions/runs/42514628818/job/128834833613), on `eb650cf58fbf397f22887be8ce938c21af807273`: `curl: (52) Empty reply from server`, Drogon exited before health readiness, then `std::length_error` / `basic_string::_M_create`. Attempt 2 on the same SHA passed. The first attempt had no native stack trace. The candidate now bounds HTTP request bodies (1 MiB), upstream responses (4 MiB), WebSocket frames (64 KiB) and stream query parameters (4 KiB), logs upstream response sizes, catches bounded response-copy `std::length_error`, and prints a Linux terminate backtrace. CI now builds Drogon with ASan/UBSan and debug symbols, checks oversized request rejection, and runs 20 repeated ingestion/vector/backtest gateway cycles. None of these new checks has executed successfully on the current candidate. | Root cause remains unproven. The historical failure occurred before the gateway health endpoint became ready; the current backtrace handler and response-size diagnostics must run on a recurrence. Confirm the sanitizer build and 20-cycle loop execute and pass. A retry or a stress loop that does not reproduce the fault is not a root-cause fix. |
| Add regression test and run integration at least 20 consecutive times | IN PROGRESS | `qoin-integration.yml` now adds a 20-iteration loop covering health, signal, strategy, instruments, event ingestion, vector lookup and backtest. The loop is not yet executed; it does not count as 20 successful runs until CI reports every iteration. | Add isolated repeat loop after root cause is known; preserve each run's exit code/log |
| Refresh stale README/architecture docs | DONE | Candidate README, architecture, state and version-map content rewritten for the fixed architecture and actual limitations | Validate links and statements against the final merged tree |
| Clean full-workspace CI baseline | IN PROGRESS | Historical CI is not exact-head evidence; latest PR #15 run set was queued | Must pass cargo build/test/clippy/fmt, gateway build/tests, web/mobile typecheck/lint/tests, Python tests, Tauri build checks, Compose and integration on exact candidate SHA |

## Phase 2 — Research platform

| Work item | Status | Evidence | Residual risk / requirement |
|---|---|---|---|
| Resumable, idempotent historical backfill with gap detection | IN PROGRESS | Provider docs and state report explicitly say no resumable backfill exists | Requires provider-specific historical APIs/entitlements, durable cursors, gap model, dedupe keys and restart tests |
| Provenance/completeness per instrument/provider | IN PROGRESS | Current registry/provider status is configuration-oriented, not a complete history/live coverage ledger | Requires persisted source metadata, freshness, gap and entitlement measurements |
| Provider failover and capability enforcement | IN PROGRESS | Current adapters: Binance USD-M has trades/book/funding/liquidations/OI; Coinbase and Kraken are trades-only; Finnhub is optional and plan/symbol-dependent | Automatic failover and complete live/historical coverage claims are not implemented; venue-isolated histories must be preserved |
| Multi-horizon causal research engine and market-backed API | IN PROGRESS | PR #12's independent horizon and causal aggregation modules are now in `qoin-vector-engine`; Axum exposes `/internal/market-horizons` and Drogon proxies `/api/v1/market-horizons`, filtering by symbol and exact venue with a 50,000-event query cap | Current CI has not executed the new endpoint assertion; asset-class isolation, freshness/data-quality contract, full client controls and historical backfill remain unfinished |
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
| Paper operating-cycle and session ledger | IN PROGRESS | Mongo-backed cycle/session snapshots, configure/start/stop routes, ingestion-driven diagnostics, paper Mirror decisions and paper trades are now wired; Drogon proxies the control routes and integration assertions cover configure/start/snapshot/stop | No real-money orders are created; partial-write recovery, event replay after restart, Archive sync and current-head CI evidence remain incomplete |
| Binance testnet end-to-end tests | BLOCKED | No exact-head testnet lifecycle evidence found | Requires owner-supplied testnet credentials stored in CI secrets; mock and paper lifecycle tests remain IN PROGRESS; never use production keys |

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

The integration workflow now asserts market-horizon API behavior and paper operating-cycle configure/start/snapshot/stop, while ingestion records paper Mirror/trade rows during the paper-session phase. These are separate assertions; the complete cross-service, authenticated Archive path, crash recovery and ledger reconciliation have not been verified as one test.

## External prerequisites genuinely BLOCKED

- **Binance Futures testnet lifecycle:** requires testnet API credentials stored in CI secrets; never use production keys.
- **Firebase identity integration tests:** require a Firebase project/test identities and authorized configuration supplied by the project owner.
- **Optional provider validation and commercial coverage:** Finnhub credentials/plan entitlements and a human decision confirming data licensing/redistribution rights for each provider/instrument.

These prerequisites do not block implementation of the corresponding interfaces, mocks, deterministic tests, or paper-mode workflows.

## Exact external prerequisites

- **Execution-environment note, not a project blocker:** the available shell cannot reach github.com and there is no mounted checkout. The CI-based PR cherry-pick workflow (run 42673468437) failed before any job was created, so it did not provide a remote Git workspace. Continue treating PR consolidation and all unverified engineering as IN PROGRESS; do not reclassify them as BLOCKED. Use CI or a repository-attached checkout to continue.
- Binance Futures testnet API credentials, stored as CI secrets, are required for real exchange lifecycle tests. Do not send credentials in chat.
- Firebase project configuration and test identities are required for ID-token/JWKS and cross-device authenticated workflow tests.
- Finnhub credentials and provider plan/symbol entitlements are required to validate optional live feeds/news/calendar; no key is committed.
- Data licensing and redistribution entitlements must be confirmed for every provider/instrument before claiming commercial coverage.

## Backlog policy

No unclassified work is considered complete. The backlog lists unfinished engineering as IN PROGRESS. BLOCKED is reserved for the external prerequisites below. Real-money mode must stay disabled; no merge to `main` or production deployment is authorized by this readiness report.

## Frontend/backend integration rework (2026-11-17)

**Status: IN PROGRESS — source corrections committed; exact-head verification pending.**

### Corrections applied on `qoin/full-integration-rework-paper`

- Removed hardcoded starter instrument registries from both React web and React Native. Both clients now initialize with an empty registry and render only instruments returned by `GET /api/v1/instruments`; Archive membership is no longer fabricated from a fixed BTC/ETH/SOL list.
- Replaced the mobile instrument-registry failure handler that silently swallowed errors with a visible error state. The web client also surfaces the registry failure in Explore.
- Corrected the shared API request helper so every non-2xx HTTP response throws an error instead of treating selected error payloads as successful application data.
- Removed the retired-model health field from the shared TypeScript API contract.
- Confirmed the frontend API paths for algorithms, backtest, calendar, events, instruments, news, strategy, stream, vector and vector history have matching Drogon gateway handlers and Axum internal routes on this branch.
- Added shared TypeScript request/response contracts and client methods for operating-cycle configuration, start/stop/status and paper-session snapshots, matching the existing Drogon and Axum routes. The corresponding interactive configuration and monitoring panels are not yet wired into the web/mobile screens.

### Boundaries confirmed

- Firebase Authentication and Firestore profile/Archive persistence are client-side integrations. They are not currently routed through Axum or MongoDB.
- Axum Firebase ID-token verification, UID-derived server-side authorization, durable Mongo-backed user Archive/notification endpoints and server-side paid-entitlement enforcement remain incomplete.
- Public market-data WebSocket and read-only market APIs are not evidence of authenticated user-specific backend access.
- Algorithm catalogue and market-access tiers remain preview-only.
- The full chain from provider ingestion through vector decision, paper-session ledger and authenticated Archive has not yet been verified as one end-to-end test.

### Required verification before marking complete

- Run web production build and strict shared-core typecheck.
- Run React Native typecheck/build.
- Run Rust workspace tests and Axum–Drogon–MongoDB integration.
- Run Drogon gateway build and repeated integration stress tests.
- Run Python reference tests, Compose/container smoke tests and desktop platform checks.
- Add and pass tests for registry failure rendering, no fabricated instruments, HTTP non-2xx propagation, cross-client API contract parity, Firebase token verification, UID isolation and Archive synchronization.
- Record exact final commit SHA and completed workflow/job results. No older CI run is accepted as evidence for this branch.


## Current backend completion pass — 2026-11-26

| Area | Current state | Evidence / remaining boundary |
|---|---|---|
| Repository integration | Active isolated branch and draft PR #32; no frontend application source was imported from feature branches | GitHub branch and PR history |
| JEPA retirement | Removed from the integrated backend runtime, research package, workflow and active version documentation | Source tree and CI workflow tree |
| Rust vector engine | Existing authoritative Rust `Signal` pipeline retained; historical event-time lookup and causal late-event protection retained | `qoin-vector-engine`, flow-engine tests |
| Paper trading | Paper-session module consumes the authoritative `qoin-vector-engine::Signal`; MongoDB session/decision/trade collections and Axum/Drogon routes added | Rust workspace tests passed on commit `3a4ff25bbf241f63bb89d6ef065d3a10fbe2bf53`; simulator is currently a shared system session, not a per-user portfolio |
| Credential vault | AES-256-GCM envelope encryption, key IDs, previous-key decryption and tamper tests added; user connection records are UID-scoped and credential material is omitted from responses | Rust workspace tests passed on `3a4ff25...`; deployment key must be configured before writes are enabled |
| Firebase identity | Server-side Google JWKS signature, issuer, audience and time-claim validation integrated | Firebase revocation watermark/check against Firebase Admin user metadata remains outstanding |
| Broker provider scope | Exact 10 crypto + 10 FX/CFD provider directory retained; adapter types are explicit fail-closed scaffolds and the runtime registry remains empty | See `BROKER_PROVIDER_SCOPE.md`; no provider is execution-ready |
| Broker account lifecycle | Encrypted connection create/list/rotate/revoke routes exist; new/rotated connections remain `pending_validation` and disabled | Per-user credentials are not yet consumed by concrete adapters; account discovery/validation is not implemented |
| Drogon gateway | Auth forwarding, paper-session and broker-connection proxy routes added | Drogon gateway CI passed on `a467e55...`; the newest route update is awaiting its own CI result |
| MongoDB integration | User-owned broker connection and audit collections/indexes added; event log and existing state recovery retained | Latest Axum/Drogon/Mongo integration and container runs were still in progress when this evidence was written |
| Paper execution realism | Fees, spread/slippage and quote-side execution exist in the session simulator | Partial fills, liquidity participation and per-user portfolio isolation still need implementation and tests |
| Live execution | Disabled by code policy; no provider adapter is registered | No live-money execution is authorized by this branch |

### Latest completed verification evidence

- `cargo test --workspace` passed on commit `3a4ff25bbf241f63bb89d6ef065d3a10fbe2bf53`, including the Rust workspace tests present at that commit.
- The Drogon gateway workflow passed on commit `a467e55b167462e97b504d1ccee3369d8ee25b90`.
- The paper-session and credential-vault test additions were included in the successful Rust workspace run.
- The gateway routes and environment documentation changed after those completed runs; those exact latest commits still require CI confirmation.
- No local MongoDB runtime, live broker sandbox, production Firebase project, or deployment environment was exercised in this session.

This section supersedes earlier branch/PR labels in the historical readiness notes below. Historical test results remain historical and must not be treated as verification of the current branch head.

## Latest autonomous backend completion pass — 2026-11-26

**Source branch:** `qoin/backend-completion-auth`  
**Backend code candidate:** `843f99a24671416d21fbaeb875f1a05774567520`  
**Documentation follow-up:** `cfe0fe438e0424277a5509b628eba1fa019bd9d8`

### Implemented in this pass

- Added AES-256-GCM credential envelope encryption with random 96-bit nonces, key IDs, previous-key decryption, tamper rejection tests and deployment-managed key configuration.
- Added Firebase UID-scoped broker connection create/list/rotate/revoke handlers. Ciphertext is omitted from API responses and connections remain `pending_validation`, `trading_enabled=false`, `execution_ready=false`.
- Added Drogon gateway proxies for broker connection lifecycle.
- Corrected the paper-session isolation defect: replaced the single global active session with a UID-keyed session map and owner-scoped v2 Mongo collections. GET/start/stop derive ownership from Firebase verification. Paper decision and trade writes include owner UID and idempotent upsert filters.
- Serialized paper-session state persistence under the session map lock so different symbol-ingest stripes cannot persist older snapshots after newer ones.
- Added authenticated owner-scoped paper trade and decision history endpoints and Drogon proxies.
- Added OpenAPI contracts for the new paper-session and broker-connection endpoints.
- Rewrote the credential security guide to accurately describe the active AES-GCM-at-rest implementation and its limitations.
- Refreshed the backend audit with source-backed findings, release gates and truthful provider status.

### Verification at this update

- Rust workspace passed on source commit `19f7a2a080993be15b172ae811dd09f74eccbfb7`, which includes the UID-scoped paper session implementation and credential vault.
- Engine checks passed on `19f7a2a...`.
- The latest code candidate `843f99a...` includes the paper history endpoints and gateway proxies. Its Rust workspace, Axum/Drogon/Mongo integration, gateway and container workflows were queued or running when this note was written; they must be rechecked before merging.
- The Axum/Drogon/Mongo integration and gateway workflows for the earlier `19f7a2a...` candidate were still running at the last status poll.
- Composite live-workspace CI failed only in the React Native typecheck job; web, Rust and Drogon jobs passed. The reported TypeScript failure is missing React type declarations in the existing frontend package. It is outside this backend change and is not represented as a backend test pass.
- Desktop and mobile checks remain separate from backend verification.
- No live broker credentials, production Firebase project, broker sandbox or deployed production environment were used.

### Remaining production blockers

- No concrete provider adapter is registered. All twenty provider catalogue entries remain fail-closed scaffolds.
- The new encrypted per-user credential records are not yet consumed by provider adapters; provider credential validation and account discovery are not implemented.
- Firebase ID-token revocation/disabled-user checks are not implemented by JWKS signature validation alone.
- Paper session ownership is now UID-scoped, but partial-fill/liquidity participation, crash-window replay and transactional paper-ledger recovery still require work.
- Order lifecycle domain types and broker-neutral preflight exist, but immutable execution events and complete broker reconciliation are not yet connected end to end.
- User-private portfolio/order WebSocket topics are not implemented. Existing market streaming is public market data and must not be mistaken for private account streaming.
- Production TLS, ingress rate/body limits, secrets-manager wiring, backups/restore drills and deployed observability remain unverified.

This section supersedes earlier progress descriptions that referred to a single global paper session or implied that provider adapters were implemented. The branch is not production-ready until the release gate in `QOIN_BACKEND_COMPLETION_AUDIT.md` passes.

## Latest risk, lifecycle and gateway hardening — 2026-11-26

**Code candidate:** `0fe1913a812a89b4efe5a013ef72d965f2540c95`  
**Audit documentation follow-up:** `9051312b816ff496abdeb30d22af3db451f18ab7`

- Connected the historical `order_safety` approval gate into the broker-neutral order dispatch preflight. It is applied to opening orders after account-currency normalization; the Decimal portfolio risk gate remains the authoritative quantitative check.
- All five separate live-approval flags default to false. The registry remains empty and live dispatch remains disabled.
- Distinguished pre-dispatch rejection from unknown broker submission outcome; the latter requires reconciliation and is never automatically retried.
- Added an append-only, owner-scoped order lifecycle event collection with idempotent state keys and indexes. Broker acknowledgements pass through the existing lifecycle state machine before acceptance is persisted.
- Added bounded per-peer Drogon HTTP rate limits (120 reads / 30 writes per minute).
- Added Axum graceful shutdown on Ctrl-C and SIGTERM.
- Added paper-session resume and protection against overwriting a paused session that still has an open position or pending action.

### Verification boundary

The code candidate `843f99a...` passed Rust workspace, Axum/Drogon/Mongo integration and Drogon gateway CI. The newer risk, lifecycle, rate-limit, session-resume and graceful-shutdown changes on `0fe1913...` were queued for exact-head CI at the time of writing. They are not marked DONE until the new runs finish successfully. The audit document records the current limitations and release gate.
