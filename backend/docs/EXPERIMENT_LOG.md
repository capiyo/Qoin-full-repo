# Experiment log

No performance experiment is currently considered valid. The geometric coefficients and thresholds are hand-specified baselines; no profitability or risk-adjusted performance claim is made.

Every future experiment must record: immutable data range and source, timezone and event ordering, symbol universe, train/validation/test windows, purging and embargo rules, configuration version, every tried configuration, fees/spread/slippage/funding, +50% transaction-cost stress, trade count, expectancy, profit factor, Sharpe/Sortino, max drawdown, time under water, regime splits, bootstrap confidence intervals, and comparison to no-trade, simple linear/logistic, and buy-and-hold baselines.

Compare fresh episode reset against carry-over and informed-prior alternatives without leaking future outcomes. Use chronological walk-forward validation and a multiple-testing/deflated-Sharpe correction. Any outcome-based adaptive learning must occur offline; never update live weights from a single completed trade.


## Engineering baseline — 2026-10-24
- Branch: `qoin-hardening`.
- Code changes: exponential half-life state decay (5 s signed flow/trade size, 2 s event intensity, 30 s liquidation pressure, 60 s volatility memory), separate book imbalance, strict event-time and per-venue sequence ordering, server-arrival feed-age rejection, retry-aware event/state/decision persistence with startup replay of pending records, three-tick entry persistence, fresh price/book gate, normalized signed liquidation pressure, explicit fail-closed regime-confidence gate, shared strategy logic in `qoin-vector-engine`, attention temperature baseline v2 with an ablation test, cost gate, generic-engine exit hysteresis, independent fail-closed risk gate, and bearer auth on write routes.
- Cost model: baseline expected-edge conversion, fees, and slippage are intentionally unset. Consequently, the strategy cost gate abstains. No calibrated edge or valid performance experiment exists.
- Risk context: the service does not yet have reconciled equity, stops, positions, exposure, and daily/weekly P&L wired into the API. The independent risk gate vetoes candidate entries when that context is missing.
- Validation status: GitHub Actions is running Rust and Python checks for this branch. No result is considered verified until the latest runs complete successfully. No profitability claim is made.
