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

## Evaluation protocol
Use the same signal code in paper, backtest, and any future live path. Replay events chronologically; split by time with purge and embargo; include realistic fees, spread, slippage, funding, market impact, and +50% cost stress. Compare against no-trade, simple linear/logistic, and buy-and-hold baselines. Report expectancy, profit factor, Sharpe/Sortino, drawdown, time under water, trade counts, regime splits, and bootstrap confidence intervals. Track every tried configuration and apply multiple-testing/deflated-Sharpe corrections.

No profitability claim is valid until this protocol has been executed on unseen chronological data.


## Deterministic replay harness
`research.replay` replays caller-supplied events in arrival order and rejects non-causal timestamps rather than sorting them. It models fee, spread, slippage, volatile-period slippage multipliers, funding, stops, partial fills, rejects, sequence gaps, stale feeds, regime approval, per-trade risk, leverage, position count, correlated exposure, daily/weekly loss limits, and a latched drawdown breaker. Entries require an expected-edge estimate above the configured cost multiple. Use `total_cost_stress_multiplier=1.5` for +50% cost stress and `volatile_slippage_multiplier=3.0` through `5.0` for volatile periods.

The harness exposes no-trade, OFI-only, and simple linear baselines, chronological purged/embargoed split ranges, a probabilistic deflated-Sharpe diagnostic, and `record_experiment` for append-only JSONL metrics. It does not download market data or claim an edge; a researcher must supply a clean event dataset, explicit arrival timestamps, and an experiment ID/configuration. The current repository has not yet run a multi-year, multi-asset experiment, so no strategy is approved for paper/live promotion.


Run a recorded replay after supplying a reviewed JSONL dataset:

```bash
PYTHONPATH=backend python -m research.run_replay \
  --input /path/to/chronological-events.jsonl \
  --experiment-id qoin-2026-10-24-baseline-001 \
  --strategy ofi_only \
  --cost-stress 1.5 \
  --volatile-slippage 4.0
```

The command fingerprints the exact input bytes and appends configuration and net-cost metrics to the JSONL experiment log. Run all three built-in baselines against the same dataset and execution assumptions; do not select a strategy using the test window.
