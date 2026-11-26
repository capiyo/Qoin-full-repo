# Qoin quantitative research guide

## Research objective
Study whether short-horizon market-force vectors provide incremental predictive value after realistic trading costs. Current coefficients are hypotheses, not learned truths. Current market state should dominate the signal; past observations are used only for causal normalization, smoothing, and context.

## Candidate forces
- Order-flow imbalance (OFI) and trade aggressor flow.
- Bid/ask depth and spread/liquidity stress.
- Volatility and filtered return changes.
- Funding pressure and open-interest changes.
- Liquidation pressure and event intensity.
- Basis and cross-venue divergence when reliable synchronized feeds exist.

## Geometric baseline
Represent each normalized force as a point/vector. Use angular alignment, magnitude, and stability gates; abstain when data quality or spread fails. Derivatives must be computed only after causal smoothing and robust filtering. Curvature/acceleration remains disabled until it demonstrates out-of-sample value. Use regularized pairwise interactions and a top-k/Pareto view for interpretation rather than allowing all noisy features to dominate.

## Learning protocol
Start a fresh adaptive episode at each new position. Partial fills belong to the same episode; closing completes it; a reversal must be a separate close and open lifecycle. Persist all feature observations, configuration versions, weights, decisions, fills, and outcomes. Completed-trade outcomes are offline calibration data only; do not let one trade update production weights immediately.

The standalone `qoin-axum-api` workspace binary is a loopback-only research server on port 8082 by default. The production `qoin-flow-engine` service is the private Axum/MongoDB service on port 8081 behind Drogon. These listeners are deliberately separate; the research binary is not a broker gateway or production order service.

## Production-engine parity

Python must not implement a second decision engine. `research.vector_model.evaluate_signal` is a thin HTTP bridge to Axum `POST /v1/signal`; Axum invokes the production Rust `qoin-vector-engine` crate and returns its versioned `Signal`. It validates and serializes the Rust `ForceObservation` contract but contains no model coefficients, attention calculation or decision thresholds.

For chronological backtests, use `research.backtest_client.run_rust_backtest`, a thin client for the Rust `/internal/backtest` replay path (or a Rust library binding built from the same crate), rather than calling a Python reimplementation. It passes the time window, venue, fees, slippage, latency, participation and market-impact assumptions to Rust and returns the Rust replay result. Python's `vector_dynamics.py` remains research-only descriptive analysis; it cannot authorize, submit or mutate production decisions. The removed Python reference decision function was replaced by the Rust API bridge, and its tests now verify delegation/serialization rather than reproducing model outputs.

The Rust replay result now reports net/annualized return, annualized volatility, Sharpe, Sortino, Calmar, maximum drawdown, completed-trade win rate, net winning/losing trade P&L, profit factor, maximum gross exposure, transaction costs and a same-instrument buy-and-hold comparison. Risk-adjusted metrics are `null` when the replay window or sample does not support a valid estimate (for example, zero volatility or no completed exits). The annualization uses 365.25 calendar days and a zero risk-free rate; short samples remain statistically weak. No metric is a forecast or guarantee.

## Evaluation protocol
Use the same signal code in paper, backtest, and any future live path. Replay events chronologically; split by time with purge and embargo; include realistic fees, spread, slippage, funding, market impact, and +50% cost stress. Compare against no-trade, simple linear/logistic, and buy-and-hold baselines. Report expectancy, profit factor, Sharpe/Sortino, drawdown, time under water, trade counts, regime splits, and bootstrap confidence intervals. Track every tried configuration and apply multiple-testing/deflated-Sharpe corrections.

No profitability claim is valid until this protocol has been executed on unseen chronological data.
