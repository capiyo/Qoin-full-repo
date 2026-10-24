# Experiment log

No performance experiment is currently considered valid. The geometric coefficients and thresholds are hand-specified baselines; no profitability or risk-adjusted performance claim is made.

Every future experiment must record: immutable data range and source, timezone and event ordering, symbol universe, train/validation/test windows, purging and embargo rules, configuration version, every tried configuration, fees/spread/slippage/funding, +50% transaction-cost stress, trade count, expectancy, profit factor, Sharpe/Sortino, max drawdown, time under water, regime splits, bootstrap confidence intervals, and comparison to no-trade, simple linear/logistic, and buy-and-hold baselines.

Compare fresh episode reset against carry-over and informed-prior alternatives without leaking future outcomes. Use chronological walk-forward validation and a multiple-testing/deflated-Sharpe correction. Any outcome-based adaptive learning must occur offline; never update live weights from a single completed trade.


## Current implementation gate status — 2026-10-24
- **Performance experiments:** none executed or considered valid by this change.
- **Profitability claim:** none.
- **Validated edge:** none.
- **Current baseline:** hand-specified and falsifiable only; attention ablation tests verify that the attention distribution changes with temperature, not that this improves trading results.
- **Required next experiment:** deterministic event-driven replay with recorded event and arrival timestamps; fee/spread/slippage and funding; partial fills and rejects; 3–5× volatile-period slippage and +50% total-cost stress; chronological purged/embargoed walk-forward windows; no-trade, OFI-only, and simple linear/logistic baselines; regime and parameter-sensitivity reporting; deflated Sharpe and bootstrap confidence intervals.
- **Promotion rule:** research configuration and offline experiments must never mutate the active strategy automatically. New maps require an explicit versioned, reviewed promotion.
