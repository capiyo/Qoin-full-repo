# Risk policy and hard gates

## Mandatory operational gates
- Live order submission is disabled and is not wired into the Axum API.
- Never expose the internal Axum service to the public internet.
- Never commit exchange credentials; use a secret manager and keys with withdrawals disabled.
- Do not enable real-money trading without explicit human approval and a separate risk review.
- Use paper/testnet and shadow mode before any production consideration.

## Target policy (not fully implemented)
- Risk per trade: 0.25% of equity, calculated from stop distance.
- Maximum leverage: 2x; enforce notional, correlated exposure, and position-count caps.
- Exchange-side protective stop, slippage limits, daily/weekly loss limits, drawdown circuit breaker, and kill switch.
- Idempotent client order IDs, timeout/partial-fill handling, exchange reconciliation, and crash recovery.
- Reject stale, out-of-order, malformed, or low-quality market data; abstain when spread or feed quality fails.

These are target requirements, not claims about current code. Until these controls are implemented and independently tested, the project must remain research/paper-only.


## Implemented fail-closed controls (research branch)
- Axum and Drogon both require `QOIN_ADMIN_TOKEN` for event ingestion and strategy mutation; an unset token denies mutation.
- State flow, traded size, event intensity, volatility memory, and liquidation counts decay using `QOIN_STATE_HALF_LIFE_MS` (default 30 seconds). Book imbalance is stored separately from trade-volume accounting.
- Entry candidates require fresh spread/state, known per-event-type sequence continuity, a configured persistence count, explicit regime approval, and a configured expected-edge estimate that exceeds `cost_multiplier × (fees + spread + slippage)`.
- Missing cost estimates, missing replay arrival timestamps, missing sequence continuity, incomplete risk policy, or absent account/exchange context fail closed.
- Risk evaluation is separate from model scoring and is persisted with decisions. `execution_allowed` remains false; the risk gate is not wired to an order-submission path.

The independent risk policy loader requires all of these owner-supplied settings before it will validate: `QOIN_MAX_POSITIONS`, `QOIN_MAX_CORRELATED_EXPOSURE_FRACTION`, `QOIN_MAX_DAILY_LOSS_FRACTION`, `QOIN_MAX_WEEKLY_LOSS_FRACTION`, `QOIN_MAX_DRAWDOWN_FRACTION`, `QOIN_RISK_MAX_SPREAD_BPS`, `QOIN_RISK_MAX_SLIPPAGE_BPS`, `QOIN_MAX_FEED_AGE_MS`, and `QOIN_RISK_MIN_STATE_QUALITY`. Per-trade risk (0.25%) and leverage (2x) are hard upper bounds in code. Supplying these variables alone is insufficient: equity, stop distance, open exposure, correlated exposure, realized losses, feed status, protective-stop confirmation, and exchange reconciliation must also come from trusted account/exchange state.

## Still required before any live-trading consideration
- Supply and review actual production values for position-count, correlated-exposure, daily/weekly loss, drawdown, spread, slippage, feed-age, and state-quality limits. Current `RiskLimits::target_policy()` deliberately fails validation because the owner has not provided these limits.
- Implement and test exchange-side protective-stop placement, reduce-only/stop semantics, timeout and partial-fill recovery, exchange reconciliation, and crash recovery for orders/positions.
- Validate sequence semantics against each actual exchange feed adapter. A sequence counter must be scoped to the correct exchange stream; missing sequence metadata currently blocks new entries.
- Prove the configured expected-edge estimator with out-of-sample research; setting `QOIN_EXPECTED_EDGE_BPS` is not evidence of an edge.
- Complete replay, paper/testnet, and shadow gates in this document, obtain an independent risk review, and obtain explicit written approval from the project owner.

The controls above are implementation progress, not a release authorization. Live trading must remain disabled.
