# Qoin engineering backlog

Research and paper-trading only. Live execution, broker order submission, exchange credentials, production deployment, and merges to `main` remain approval-gated.

Items are ordered by dependency and risk. Do not start a lower item while a higher item has a failing verification gate.

## Current audit baseline (2026-11-17)

- Primary implementation PR: [#15 — Qoin live market streams, vector space and multi-asset Explore](https://github.com/capiyo/Qoin-full-repo/pull/15), draft, base `main`.
- Audited PR head: `118f3404770259ae65d3b98770dbe035fd1475a7`.
- At audit time, eight PR-triggered workflow runs were queued: live workspace, engine, Rust workspace, container build, desktop, React Native, React web, and Axum–Drogon–MongoDB integration. Queued is not passed; recheck exact-head results before closing release gates.
- JEPA removal is recorded as completed on the active PR branch: Python sidecar/tests, Rust sidecar module, JEPA workflow, API/proxy/Compose references, client health field and obsolete JEPA version specification were removed. Keep a repository-wide search in final acceptance to prevent reintroduction.
- V1 remains the deterministic vector decision engine. V2 now has durable append-only observation/relation records, bounded resumable backfill, reconstruction, hash/integrity auditing and correction proposals. It remains non-authoritative and must not alter V1 decisions.
- The equal-notional allocator supports up to 15 selected broker-account targets as a tested planning primitive. The execution route is still not wired to multi-broker fan-out.

## 1. Restore and stabilize Axum–Drogon–MongoDB integration

- [ ] Reproduce any intermittent gateway process abort on the exact current PR head and preserve request/response plus service logs.
- [ ] Isolate gateway proxy, Axum handler, database or harness failure; diagnostics must identify the failing request and process.
- [ ] Add a deterministic regression test for the exact failing sequence.
- [ ] Repeat the integration workflow enough to detect flakiness; record exact commit and run IDs.
- [ ] Do not close this item based on a successful retry alone when the original failure remains unexplained.

## 2. Complete equal multi-broker execution foundations

- [x] Decimal equal-notional allocation primitive: up to 15 targets, per-account native quantity conversion, step-size round-down and unallocated residual.
- [x] Document explicit user-selected equal split; do not silently consolidate, drop selected accounts or redistribute rounding residual.
- [ ] Implement verified per-user broker-account linking and a server-side credential vault; secrets never enter clients or chat.
- [ ] Implement real provider adapters for each supported broker and distinguish market-data-only providers from executable broker accounts.
- [ ] Connect the allocator to a durable parent/child order model with stable idempotency keys.
- [ ] Add atomic aggregate risk/margin reservation across every selected account before dispatch.
- [ ] Dispatch all eligible child orders concurrently; persist each acknowledgement, partial fill, rejection, timeout and unknown outcome independently.
- [ ] Add automatic broker-side reconciliation and crash recovery; never blindly retry an uncertain child or increase another account's share.
- [ ] Add testnet end-to-end tests for all selected accounts, mixed currencies, instrument multipliers, minimum quantities, partial failures and duplicate delivery.
- [ ] Keep live multi-broker execution disabled until all of the above are verified and explicitly approved.

## 3. Complete V2 set / measure / relation ledger

- [x] Define typed finite event sets, counting measures, observed trade-size measures and explicit missing-size counts.
- [x] Define bounded temporal-precedence relations with source/target identities, lag statistics and truncation diagnostics; never describe temporal order as causation.
- [x] Persist source-attributed observation records and relation edges append-only with deterministic IDs and source-event SHA-256 fingerprints.
- [x] Add bounded resumable backfill from canonical raw events; duplicate retries are idempotent.
- [x] Add reconstruction from durable observations with hash verification and source/ledger coverage diagnostics.
- [x] Add bounded integrity audit for missing observations, invalid rows, hash mismatches, unresolved relation endpoints and truncated relation scans.
- [x] Add operator-only correction proposals that preserve source identity and require an expected source hash; preserve history rather than mutating evidence silently.
- [ ] Add pagination/cursors to user-facing ledger reconstruction and relation browsing so large ledgers are not truncated without a continuation path.
- [ ] Add property-based tests for set membership, measure additivity, duplicate event delivery, ordering, correction supersession and reconstruction determinism.
- [ ] Add operational maintenance runbook for secret rotation, resumable backfill, integrity scans, correction review and recovery.
- [ ] Keep V2 descriptive/research-only until its independent state-transition and validation model is specified; it must never become hidden V1 decision authority.

## 4. Establish historical market-data completeness

- [ ] Inventory provider coverage, licensing/redistribution constraints, canonical symbols/venues and available history.
- [ ] Add idempotent historical backfill with pagination, rate-limit handling, resumability and source provenance.
- [ ] Normalize and deduplicate events with provider sequence/time semantics; detect gaps, duplicates, stale data and clock anomalies.
- [ ] Version immutable datasets and expose coverage diagnostics to replay callers.
- [ ] Add Coinbase/Kraken book-depth coverage only where provider APIs and licensing permit; do not infer depth from trades.
- [ ] Add provider arbitration/failover and measured freshness/uptime SLOs; stale or unavailable feeds must fail closed.
- [ ] Test restart/resume and provider failure without fabricating observations.

## 5. Build portfolio-grade research evaluation

- [ ] Add portfolio accounting across instruments, currencies, cash, funding, fees and exposure.
- [ ] Add paper sizing and risk constraints, including concentration and correlated exposure.
- [ ] Add benchmark-relative metrics, Sharpe/Sortino/Calmar, turnover, exposure, holding time and drawdown duration.
- [ ] Add walk-forward and out-of-sample evaluation, regime segmentation, parameter sensitivity and cost stress.
- [ ] Make every run reproducible from immutable dataset, strategy, configuration and execution-model versions.
- [ ] Clearly separate measured results from estimates; no profitability claims without independent evidence.

## 6. Complete the multi-algorithm research runtime

- [ ] Define a stable Rust algorithm interface for identity/version, required data/features, validated config, signal output and evaluation metadata.
- [ ] Register independently implemented strategies only when their data requirements, isolated state and tests exist.
- [ ] Isolate per-algorithm state and configuration; reject unsupported symbols, venues, timeframes and missing features.
- [ ] Add comparable evaluation reports and selection metadata without implying strategy quality.
- [ ] Persist authenticated user selections and enforce entitlements server-side only after identity and payment reconciliation are verified.
- [ ] Keep catalogue tiers preview-only; do not activate paid access until signed webhook verification, reconciliation, refunds and chargebacks are implemented.

## 7. Secure the authenticated application boundary

- [ ] Verify Firebase ID tokens server-side and derive user identity only from verified claims.
- [ ] Scope every personalized read/write to authenticated UID and test cross-user isolation.
- [ ] Add request validation, bounded payloads, rate limits, WebSocket connection/origin controls and audit events.
- [ ] Keep public market-data streaming free of private account data; apply edge controls before broad public exposure.
- [ ] Keep payment grants inactive until signed webhook verification and reconciliation are implemented.

## 8. Notifications and synchronized Archive

- [ ] Persist notification history per authenticated user and synchronize unread/read state across devices.
- [ ] Add durable queue/retry semantics and user-configurable alert preferences.
- [ ] Add device-token registration and background push delivery for supported mobile platforms; web browser alerts remain permission-gated.
- [ ] Persist Archive subscriptions server-side and synchronize across web, mobile and desktop.
- [ ] Test duplicate transitions, reconnect gaps, offline delivery, token rotation and user isolation.

## 9. Production operations and release hardening

- [ ] Add service metrics, structured logs, tracing correlation, alert thresholds and provider health monitoring.
- [ ] Define MongoDB backup, restore drills, retention and recovery objectives; verify restart recovery.
- [ ] Pin Rust dependencies with a committed lockfile; pin container image digests and add SBOM/vulnerability scanning.
- [ ] Add secret-management guidance and verify no provider keys or tokens are exposed to clients or committed.
- [ ] Verify Compose topology, least-privilege database access, private Axum binding and gateway-only ingress.
- [ ] Verify Tauri 2 packaging on Windows, macOS and Linux on the exact current head; keep updater/shell/filesystem permissions disabled unless separately reviewed.
- [ ] Verify web, React Native and desktop use the same `@qoin/core` contracts and API behavior.
- [ ] Add end-to-end smoke coverage for auth-gated Archive/Explore, instrument detail, V1 transparency, V2 ledger and paper backtest.
- [ ] Publish runbooks, environment separation and limitations; deployment itself remains approval-gated.

## 10. Final paper-mode acceptance

- [ ] Confirm no live order-submission route is exposed or enabled in any client or public gateway.
- [ ] Confirm private execution routes remain disabled by default, private, authenticated and explicitly operator-gated.
- [ ] Run full Rust, C++, web, mobile, desktop, Python, emulator-security and integration verification on one exact release commit.
- [ ] Run repository-wide JEPA/obsolete-version search and inspect secrets, unsafe defaults, unbounded work and accidental live paths.
- [ ] Review the complete diff and verify all PR checks are green on the exact head.
- [ ] Publish a limitations and operational-readiness report; do not describe the system as production-ready until all gates are evidenced.
