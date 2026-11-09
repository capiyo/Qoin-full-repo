# Qoin engineering backlog

This file contains unfinished work only. **IN PROGRESS** is the default for engineering tasks. **BLOCKED** is reserved for an external exchange key, unavailable hardware, or a human decision/authorization; lack of a local checkout or unresolved code is not a blocker.

## Integration and baseline
- [ ] **IN PROGRESS — PR #12 transplant:** six clean engine/contract/documentation commits have been cherry-picked (multi-horizon module, causal market aggregation, exports, schema and semantics docs). The market-backed Axum endpoint and Drogon `GET /api/v1/market-horizons` proxy are now ported with a bounded Mongo query and a CI assertion. Asset-class isolation/catalogue, data-quality/freshness, web/mobile panels and full integration tests remain to be ported from the remaining overlapping paths. Duplicate JEPA-removal and duplicate docs are superseded; see `docs/DECISIONS.md`.
- [ ] **IN PROGRESS — PR #13 transplant:** broker registry, paper replay/Mirror docs, paper-session and operating-cycle modules have been transplanted; both modules are declared in Axum and the paper-session test fixture was adapted to the current decision type. The modules are not yet connected to durable Mongo state, event ingestion, session transitions, ledger persistence, Archive lifecycle, replay diagnostics or client controls; these remain to be ported from 12 overlapping paths. See `docs/DECISIONS.md`.
- [ ] **IN PROGRESS — PR ancestry audit:** confirm whether isolated PR #11 changes are represented in the candidate; do not assume the separate `qoin/integration-chain` merge is included. CI cherry-pick automation failed before job creation; do not treat it as completed.
- [ ] **IN PROGRESS — TypeScript baseline:** source correction for TS18047 is present (`decisionStrategy.updated_at_ms` is read inside its non-null guard); obtain exact-head green web, mobile and `@qoin/core` checks.
- [ ] **IN PROGRESS — full baseline:** Rust fmt/clippy/build/test, Axum integration, Drogon build/tests, JS web build/typecheck/lint/tests, React Native typecheck/tests, Python tests, Tauri desktop matrix, Compose and security checks on the exact PR head.

## Drogon stability
- [ ] **IN PROGRESS — `std::length_error`:** capture a failing stack/core trace or deterministic reproducer; inspect all untrusted length/count conversions and allocation boundaries; add defensive bounds checks where justified; add regression coverage; run at least 20 consecutive integration cycles. Root cause remains unproven until a failing trace is captured.

## Phase 2 — Research platform
- [ ] **IN PROGRESS — historical ingestion:** resumable/idempotent backfill, durable cursors, gap detection, deduplication and restart tests.
- [ ] **IN PROGRESS — data quality:** per-instrument/provider provenance, freshness, completeness and gap ledger.
- [ ] **IN PROGRESS — provider capability/failover:** enforce capabilities, health-based failover and venue-isolated history without fabricating unavailable fields.
- [ ] **IN PROGRESS — portfolio accounting:** quote-currency cash ledger, FX valuation, fees, funding/financing, contract multipliers and correlated exposure limits.
- [ ] **IN PROGRESS — research validation:** immutable datasets/configs, chronological walk-forward/OOS splits, benchmarks and reproducible manifests.
- [ ] **IN PROGRESS — algorithm isolation:** independent runtime/config/state, durable per-user selection, version pinning and comparable metrics.

## Phase 3 — User platform
- [ ] **IN PROGRESS — Firebase verification:** ID-token signature/JWKS cache, issuer/audience/expiry checks, key rotation and adversarial tests.
- [ ] **IN PROGRESS — entitlements:** server-side UID-derived grants, payment/refund reconciliation and authorization tests.
- [ ] **IN PROGRESS — durable preferences:** protected API and persistent algorithm selection with entitlement enforcement.
- [ ] **IN PROGRESS — cross-device sync:** durable Archive/notification storage, cursors, idempotency, offline conflict handling and device registration.
- [ ] **IN PROGRESS — client workflows:** authenticated Explore/detail/Archive/notifications/paper-session flows across web, mobile and desktop.

## Phase 4 — Execution infrastructure
- [ ] **IN PROGRESS — safety gate verification:** test the code-level real-money rejection on every route/configuration path; real-money remains disabled.
- [ ] **IN PROGRESS — risk integration:** enforce portfolio risk, identity, exposure, loss, freshness and protective-order validation on every order path.
- [ ] **IN PROGRESS — account binding:** verify exchange-account ownership and least-privilege permissions.
- [ ] **IN PROGRESS — durable lifecycle:** persisted submit/ack/partial-fill/cancel/amend/reject state machine and crash-safe idempotency.
- [ ] **IN PROGRESS — reconciliation:** background order/balance/position reconciliation, leases, drift detection and safe repair.
- [ ] **IN PROGRESS — kill switch:** block entries and manage existing exposure with tested cancel/flatten policy and independent emergency path.
- [ ] **IN PROGRESS — account sync:** balances, positions, margin, funding and instrument metadata.
- [ ] **BLOCKED — Binance testnet credentials:** testnet order lifecycle E2E requires owner-supplied testnet API keys in CI secrets; never use production keys. Implement mocks and deterministic tests meanwhile.

## Phase 5 — Release acceptance and E2E
- [ ] **IN PROGRESS — adversarial tests:** auth bypass, tampering, replay, injection and authorization boundary tests.
- [ ] **IN PROGRESS — failure/recovery:** provider/Mongo/gateway outages, restart recovery, idempotency and fault injection.
- [ ] **IN PROGRESS — Compose recovery:** isolated deployment, backup/restore drill and recovery assertions.
- [ ] **IN PROGRESS — operations:** SLOs, alerts, incident runbooks, backup/restore evidence and independent review.
- [ ] **IN PROGRESS — full E2E:** automated ingestion → vector → decision → paper execution → ledger → authenticated Archive test with persisted assertions and replay/idempotency coverage.

## External prerequisites
- [ ] **BLOCKED — Firebase test project/identities:** owner must provide the authorized Firebase project and test identities/configuration for real JWKS/auth integration tests. Unit tests and verifier implementation remain IN PROGRESS.
- [ ] **BLOCKED — provider credentials/licensing decision:** Finnhub plan credentials and human confirmation of data licensing/redistribution rights are required to validate optional provider coverage. Provider abstractions, mocks and capability enforcement remain IN PROGRESS.

Do not merge to `main`, deploy production, or enable real-money execution until the exact final PR head passes all release gates and operational review.
