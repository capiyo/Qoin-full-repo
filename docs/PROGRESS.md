# Qoin implementation progress

Updated: 2026-10-23

## Verified in the available audited snapshot
- Python reference-model and vector-model tests: 12/12 passed in GitHub Actions (including causal vector kinematics, regularized pairwise interactions, and Pareto top-k selection).
- JSON event schema parses successfully.
- OpenAPI YAML parses successfully (6 paths in the audited snapshot).
- Smoke script passes shell syntax validation.
- Architecture, dependency inventory, risk-policy, runbook, experiment-log, and state-of-repository notes are documented.

## Present in this integration branch
- Rust Axum service manifest, event/state/vector API source, deterministic vector baseline, and isolated gated Binance adapter source.
- Research-only causal position/velocity/acceleration and Pareto top-k helpers; example versioned parameter contract at `config/vector-model-v1.example.json` (not loaded by runtime).
- C++ Drogon gateway proxy source and CMake configuration.
- MongoDB development compose file and .env.example with loopback bindings and live mode disabled.
- Architecture and risk documentation.

## Not verified here
- Rust compilation and unit tests: GitHub Actions passed `cargo test`; 3 Rust unit tests passed.
- Drogon/C++ compilation: Drogon development package unavailable.
- MongoDB integration and restart recovery.
- Real market-data feed, testnet, event-driven backtesting, walk-forward validation, and shadow-mode comparison.
- Profitability, live reliability, or safe production readiness.

## Next required milestones
1. Run CI with pinned toolchains and lockfiles; fix every native build/test failure.
2. Add full state replay and data-quality/time-order guards.
3. Implement versioned model configuration and past-only normalization.
4. Build an event-driven backtester that uses the exact production signal code; include fees, spread, slippage, funding, and +50% cost stress.
5. Add an independent risk gate, protective stops, reconciliation, circuit breakers, and recovery before even testnet execution.
6. Keep real-money execution disabled pending explicit human approval.

## Autonomous backlog initialization (2026-11-08)
- Added root `BACKLOG.md` because no root backlog existed on the inspected branch; ordered work by verification dependencies and kept paper/research-only and approval gates explicit.
- Re-read branch head and last five commits. GitHub reports the remote branch snapshot; no local checkout is mounted in this engineering session, so local `git status` and local command execution are unavailable.
- Latest head before backlog initialization: `eb650cf58fbf397f22887be8ce938c21af807273`.
- The Axum–Drogon–MongoDB integration job failed after the mixed-venue replay assertion, when the next gateway request received an empty reply and Drogon terminated with `std::length_error`. A failed-job rerun was started to establish reproducibility; no root-cause fix is claimed yet.
- The first backlog item is the integration failure. No feature work is started ahead of that verification gate.
- Follow-up: the failed integration job was rerun on the same code SHA; attempt 2 passed all integration steps. The original Drogon `std::length_error` is still unexplained and is tracked in root `BLOCKERS.md`; the backlog item remains open.
- Causal replay change committed on `3d964bc471b3908119ebda116317da5c7ad59936`: signals now fill only on a strictly later timestamped trade/book event; book buys use ask and sells use bid; final unfilled signals retain decision provenance. OpenAPI replay description updated.
- Verification on that code commit: `cargo test --workspace` passed; Axum engine tests passed (31 passed, 0 failed); Python reference tests and event-schema validation passed; React web, React Native typecheck, Drogon gateway build and Axum–Drogon–MongoDB integration step passed. Desktop packaging and remaining workflow cleanup were still running at the time of this entry.
- A compile error in the first implementation attempt (`&str` to `String` for the queued action) was caught by CI and corrected in `86cd49b`; a same-timestamp guard and regression fixture were then added in `3d964bc`.
- Replay order lifecycle follow-up committed on `884d14e5785225241a6b12de19f71d51b28813b6`: every replay order now has a deterministic replay-scoped ID, pending/filled status, decision/submission/fill timestamps, and source event IDs. Immediate submission is an explicit assumption; routing latency remains unmodeled.
- Verification on the order-lifecycle code commit: `cargo test --workspace` passed (Axum engine 31 passed, 0 failed; other Rust workspace suites 6 passed each); dedicated engine workflow passed (31 passed, 0 failed) and Python reference/schema checks passed. Axum–Drogon–MongoDB integration passed. React Native typecheck passed. The live-workspace React web, Drogon gateway and React Native jobs passed; its duplicate Rust job was still running at the time of this entry.
- Desktop Tauri 2 packaging passed on Windows, macOS and Ubuntu for the preceding code-equivalent desktop snapshot `3d964bc471b3908119ebda116317da5c7ad59936`; subsequent commits changed only backend replay code and documentation, not desktop packaging inputs.
- Configurable replay latency added across Axum, Drogon query forwarding, OpenAPI, shared TypeScript contracts, and the web replay panel. Accepted range is 0–60,000 ms; zero still requires a strictly later event. The report includes the applied latency.
- The web replay table now shows order ID/status and decision, submission, and fill times; a final pending order is surfaced explicitly. Mobile typecheck passed on the same UI code snapshot.
- Rust workspace verification on code snapshot `46d4504897ca81e5f3bb813eacd0eb96fa9d8e52`: 33 Axum engine tests passed, 0 failed; the other Rust workspace suites passed (6 tests each). The tests include latency gating and rejection of negative or over-limit latency.
- Axum–Drogon–MongoDB integration passed on the same code snapshot. A subsequent workflow-only commit `e7cdab34e4789d812a605b9851098664f9ba9ce8` adds an explicit end-to-end assertion that `latency_ms=50` is forwarded through Drogon and returned by Axum; that run is still pending. React web and desktop builds for the updated panel are also pending.
