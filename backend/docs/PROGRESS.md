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
