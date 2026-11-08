# Qoin engineering backlog

Research and paper-trading only. Live execution, broker order submission, exchange credentials, production deployment, and merges to `main` are out of scope without explicit approval.

Items are ordered by dependency and risk. Do not start a lower item while a higher item has a failing verification gate.

## 1. Restore the intermittent Axum–Drogon–MongoDB integration check (open; retry green, root cause unknown)
- [ ] Reproduce the current failure on the exact draft-branch head and preserve the failing request/response evidence.
- [ ] Isolate whether the failure is in the gateway proxy, Axum handler, or test harness; add diagnostics that identify the failing request and process.
- [ ] Fix the smallest root cause without widening public write routes or changing the fixed architecture.
- [ ] Add a regression assertion for the exact failing sequence.
- [ ] Run the integration workflow and verify it is green on the resulting commit; record exact commit and job result.

## 2. Make historical replay causally executable
- [x] Separate decision timestamp from fill timestamp and preserve both event IDs in the fill record.
- [x] Record a replay-scoped order ID, pending/filled status, and decision, submission, and fill timestamps with source event IDs. Submission is explicitly assumed immediate; latency remains a later execution-model item.
- [x] Prevent same-event and same-timestamp fills; use a strictly later executable trade or side-specific book quote, otherwise leave the signal unfilled.
- [x] Use ask for book buys and bid for book sells; include the observed spread in executable prices.
- [x] Add a configurable 0–60,000 ms minimum delay between submission and the first eligible fill; zero still requires a strictly later event.
- [x] Bound fill quantity by observed trade size or side-specific top-of-book size; persist partial orders across observations and allocate sell cost basis proportionally.
- [x] Add bounded market impact as coefficient × squared per-fill participation, capped by the configured coefficient and reported separately from base slippage.
- [x] Expose fees, slippage, latency, max participation, market-impact coefficient, per-fill applied impact, and aggregate impact cost through the API and replay report.
- [x] Test timestamp ordering, same-timestamp no-fill, missing-next-observation pending orders, and no same-event look-ahead.
- [x] Add an explicit stale/out-of-order quote regression; replay timestamp-sorts the stale observation before the later signal so it cannot fill that signal.
- [x] Publish the active execution assumptions in every replay result.
- [ ] Add cost-only 1.5× and 2× fee/slippage/impact stress scenarios; label clearly that fills and liquidity are not recomputed.

## 3. Establish historical market-data completeness
- [ ] Inventory provider coverage, licensing constraints, canonical symbols/venues, and available history.
- [ ] Add idempotent historical backfill with pagination, rate-limit handling, resumability, and provenance.
- [ ] Normalize and deduplicate events with provider sequence/time semantics; detect gaps, duplicates, stale data, and clock anomalies.
- [ ] Version stored datasets and expose coverage diagnostics to replay callers.
- [ ] Test restart/resume and provider failure behavior without fabricating observations.

## 4. Build portfolio-grade research evaluation
- [ ] Add portfolio accounting across instruments, currencies, cash, funding, fees, and exposure.
- [ ] Add sizing and risk constraints for paper simulation, including concentration and correlated exposure.
- [ ] Add benchmark-relative metrics, Sharpe/Sortino/Calmar, turnover, exposure, holding time, and drawdown duration.
- [ ] Add walk-forward and out-of-sample evaluation, regime segmentation, parameter sensitivity, and cost stress.
- [ ] Make every run reproducible from immutable dataset, strategy, configuration, and execution-model versions.

## 5. Complete the multi-algorithm research runtime
- [ ] Define a stable Rust algorithm interface for identity/version, required data/features, validated config, signal output, and evaluation metadata.
- [ ] Register independently implemented strategies only when their data requirements and tests exist.
- [ ] Isolate per-algorithm state and configuration; reject unsupported symbols, venues, timeframes, and missing features.
- [ ] Add comparable evaluation reports and selection metadata without claiming strategy quality.
- [ ] Keep catalogue selection and market-access tiers preview-only until verified identity and server-side entitlements exist.

## 6. Secure the authenticated application boundary
- [ ] Verify Firebase ID tokens server-side and derive user identity only from verified claims.
- [ ] Scope every personalized read/write to the authenticated UID and test cross-user isolation.
- [ ] Add request validation, bounded payloads, rate limits, WebSocket connection/origin controls, and audit events.
- [ ] Keep payment grants inactive until signed webhook verification, reconciliation, refunds, and chargebacks are implemented.
- [ ] Do not activate paid access or change money-handling behavior without explicit approval.

## 7. Production operations for research and paper mode
- [ ] Add service metrics, structured logs, tracing correlation, alert thresholds, and provider health monitoring.
- [ ] Define MongoDB backup, restore drills, retention, and recovery objectives; verify restart recovery.
- [ ] Add secret-management guidance and ensure no provider keys are exposed to clients or committed.
- [ ] Add deployment/runbooks and environment separation while keeping deployment itself approval-gated.
- [ ] Exercise failure, restart, stale-feed, and degraded-provider scenarios.

## 8. Desktop and client release hardening
- [ ] Verify Tauri 2 packaging on Windows, macOS, and Linux on the current branch head.
- [ ] Verify updater/signing design separately; do not grant updater, shell, filesystem, or credential permissions by default.
- [ ] Verify web, React Native, and desktop use the same `@qoin/core` contracts and API behavior.
- [ ] Add end-to-end smoke coverage for authentication-gated Archive/Explore, instrument detail, and paper backtest flows.
- [ ] Document platform-specific release artifacts and manual approval gates.

## 9. Final paper-mode acceptance
- [ ] Confirm no live order-submission route is exposed or enabled in any client, gateway, or engine.
- [ ] Run the full Rust, C++, web, mobile, desktop, Python, and integration verification matrix.
- [ ] Review the complete diff for secrets, unsafe defaults, unbounded work, and accidental live-execution paths.
- [ ] Publish a limitations and operational-readiness report; do not describe the system as production-ready until all required gates are evidenced.
