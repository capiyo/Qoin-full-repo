# Experiment log

No performance experiment is currently considered valid. The geometric coefficients and thresholds are hand-specified baselines; no profitability or risk-adjusted performance claim is made.

Every future experiment must record: immutable data range and source, timezone and event ordering, symbol universe, train/validation/test windows, purging and embargo rules, configuration version, every tried configuration, fees/spread/slippage/funding, +50% transaction-cost stress, trade count, expectancy, profit factor, Sharpe/Sortino, max drawdown, time under water, regime splits, bootstrap confidence intervals, and comparison to no-trade, simple linear/logistic, and buy-and-hold baselines.

Compare fresh episode reset against carry-over and informed-prior alternatives without leaking future outcomes. Use chronological walk-forward validation and a multiple-testing/deflated-Sharpe correction. Any outcome-based adaptive learning must occur offline; never update live weights from a single completed trade.
