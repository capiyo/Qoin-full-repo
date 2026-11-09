# Engineering decisions

These decisions apply to the Qoin integration candidate unless superseded by an explicit repository decision.

1. **Architecture is fixed.** Rust/Axum remains authoritative for vector calculation, decisions, risk and state. C++/Drogon remains an HTTP/WebSocket gateway only. MongoDB is persistence. React web and React Native share `@qoin/core`. Python is for research/backtesting/analysis. Tauri 2 is the desktop shell and the vector-motion solver remains Rust.
2. **One deterministic decision authority.** The active engine is the deterministic Rust vector engine. No second model service or client-side decision implementation may affect decisions.
3. **Multi-market support stays active.** The current multi-asset registry and provider adapters are retained. Capability differences must be explicit; unavailable book/funding/liquidation/OI fields must never be synthesized.
4. **Real-money execution stays disabled in code.** Environment variables, credentials and the operator gate alone cannot enable live execution. Paper and exchange testnet are the only allowed modes until every release gate is independently verified.
5. **No production merge or deployment.** Work is isolated on `qoin/integration-completion`; `main` and production are not changed.
6. **PR integration order is #11, #12, #13, #15.** The PRs were found to share a common base rather than form a linear ancestry. PR #11 was merged into the isolated `qoin/integration-chain` branch. Retargeting PR #12 to that branch exposed merge conflicts, so the remaining PRs must be reconciled in a real checked-out workspace rather than force-merging divergent snapshots.
7. **Evidence is commit-specific.** A green workflow on an older SHA is historical evidence only. A release claim requires all relevant jobs to pass on the exact final PR head.
8. **Unknown failures remain open.** A retry that passes does not resolve the intermittent Drogon `std::length_error`; root cause and repeatability evidence are required before closure.


## PR reconciliation record — 2026-11-09

The integration target is PR #15's head snapshot (`280d8481a939a9f12c4cfcd8f9f0fbbd0bb6f0a7`) with the foundation/safety changes already committed on `qoin/integration-completion`. This is not yet a completed consolidation.

- **PR #15 — base retained.** The branch was created from this PR's head. Its Archive/Explore UI, auth-gated research flows, algorithm catalogue, Tauri shell/vector-motion work, replay hardening, and existing PR-specific changes remain in the candidate. No PR #15 commit was intentionally dropped.
- **PR #12 — partial duplicate removal only; unique work not yet transplanted.** Its retired-model/JEPA removal and overlapping architecture-document commits are superseded by the candidate's own removal and architecture rewrite; those duplicate commits are not needed as cherry-picks. Its distinct multi-horizon research API, causal horizon aggregation, asset-class isolation/catalogue, web/mobile research panels, and related tests are **not yet present by virtue of PR #12** and must be transplanted. GitHub reported `mergeable=false` when PR #12 was temporarily targeted at this branch; its base was restored to `main`. No PR #12 commit was merged or cherry-picked.
- **PR #13 — unique work not yet transplanted.** Its dynamic broker capability registry, configurable/durable operating-cycle state machine, Mongo-backed paper-session accounting, replay/backtest diagnostics and web/mobile controls are **not yet integrated from PR #13**. GitHub reported `mergeable=false` when PR #13 was temporarily targeted at this branch; its base was restored to `main`. No PR #13 commit was merged or cherry-picked.
- **PR #11 — isolated historical integration only.** It was merged into `qoin/integration-chain` as `2d954aafb5f52ef8e2495316bdcc62466f6489d1`; it is not asserted to be present in this PR #16 branch unless confirmed by ancestry/diff review.

No unique feature is classified as intentionally dropped. The only dropped material is superseded duplicate JEPA-removal and duplicate documentation work from PR #12. The unintegrated unique PR #12/#13 features remain IN PROGRESS and must be cherry-picked/reconciled, with conflict decisions recorded before closure.


## File-level reconciliation inventory

A Git-tree comparison against common base `79198891b6cabad760185f58a8f6f401c56240c1` identified the following path-level integration surface (changed-file tree comparison, not a commit-level cherry-pick):

- **PR #12:** 22 changed paths; 6 do not overlap changes in the PR #15 candidate and are clean transplant candidates: `backend/rust-axum/crates/qoin-vector-engine/src/lib.rs`, `market_data.rs`, `multi_horizon.rs`, `docs/MULTI_HORIZON_RESEARCH.md`, `shared/contracts/market-event.schema.json`, and `.github/workflows/react-mobile.yml`. The other 16 overlap candidate edits, including Axum `main.rs`/`Cargo.toml`, Drogon `main.cc`, web/mobile screens, shared API contracts, and workflows; those need semantic conflict resolution.
- **PR #13:** 18 changed paths; 6 are clean transplant candidates: `backend/config/brokers.json`, `backend/research/PAPER_REPLAY_AND_MIRROR.md`, `backend/research/QOIN_VECTOR_MONGODB_ADDENDUM.md`, `backend/rust-axum-api/src/main.rs`, `backend/rust-axum/src/operating_cycle.rs`, and `backend/rust-axum/src/paper_session.rs`. The other 12 overlap candidate edits, including Axum `main.rs`/`Cargo.toml`/`backtest.rs`, Drogon `main.cc`, web/mobile screens, shared API contracts and workflows.
- **Automation attempt:** `.github/workflows/integrate-prs.yml` was added to cherry-pick patch-unique commits in #12 then #13 order using an Actions runner. Its first run failed before a job was created (run `42673257551`); a simplified retry is queued as `42673468437` but later concluded failure with zero jobs. No cherry-pick command ran, so no source commits were integrated by that automation.

Do not describe either PR as consolidated until the unique commits are actually applied, all 28 overlapping paths are reconciled, and the final diff and tests are reviewed.


## Actual PR-source transplants applied to `qoin/integration-completion`

- **PR #12 — six clean source commits cherry-picked:** `5bfb360d74f97ed150279665ec2e0338d46511ff` (multi-horizon engine), `7d2b01759fa47b85ce6998c24b489ed3ac4aae10` (module export), `68a839ca62b3a28239a1e7442c0b20c4376c999f` (causal market aggregation), `d0d73fcd852396fa413eff8d111aa1c327dce461` (aggregation export), `2ee75d2fa7` (research semantics documentation), and `041437e168` (market-event contract). These were applied as individual commits with source commit references. The Axum endpoint, asset-class filtering/isolation, Drogon route, web/mobile panels and related integration tests remain unported because their source commits touch files that diverged in PR #15.
- **PR #13 — four clean source commits cherry-picked:** `82cdf9e995` (broker capability registry), `0813245286` (paper replay/Mirror documentation), `92467f78a2` (paper session model), and `34f5f262f1` (operating-cycle model). The current PR #13 head versions of `paper_session.rs` and `operating_cycle.rs` were then ported to retain its later contract/metrics refinements; `main.rs` now declares both modules and the paper-session test fixture was adapted to the current `EdgeDecision` contract. Documentation follow-up commits `9310403530` and `1dce25e41c` were also cherry-picked.
- **Still not consolidated:** PR #12's Axum/C++ endpoint and UI commits, and PR #13's durable Mongo wiring, event-driven paper-session integration, Archive lifecycle, broker registry runtime, backtest venue/currency behavior and client flows. The source PRs remain open/draft. No claim of full PR #12/#13 consolidation is made.


## Follow-on runtime port

After the clean source transplants, the PR #12 market-backed research endpoint was ported into the current Axum `main.rs` and exposed through Drogon at `GET /api/v1/market-horizons`. It filters by exact symbol and venue, bounds the Mongo event query to 50,000 records, uses causal horizon aggregation, and labels output `research_only`; no order path consumes its result. The integration workflow now asserts the route and eight horizon observations. This does not yet include PR #12's asset-class isolation, instrument catalogue, web/mobile panels or complete data-quality/freshness contract.

PR #13's operating-cycle and paper-session modules are now present in the Axum crate, declared by `main.rs`, and their tests were adapted to the current `EdgeDecision` shape. Axum now persists cycle and paper-session snapshots in MongoDB, exposes configure/start/stop/snapshot routes, and updates cycle diagnostics plus paper Mirror/trade ledgers from ingestion during the paper-session phase. Drogon proxies these controls. The integration workflow asserts configure/start/snapshot/stop behavior. This is still IN PROGRESS: crash-safe replay after a partial persistence failure, cycle/session archive lifecycle, full client controls and executed CI evidence remain incomplete.
