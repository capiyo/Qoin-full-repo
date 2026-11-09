# Risk policy and hard gates

## Current implementation status
- The Axum vector endpoint emits research signals only. It does not submit orders.
- `backend/rust-axum/src/order_safety.rs` implements deterministic pre-trade checks for identity, finite values, stop direction, risk-at-stop, order notional, gross leverage, daily loss, open-position count, market-data age, and exchange-side protective-stop capability. It rejects malformed policy configuration before order evaluation, including risk/daily-loss/notional fractions outside 0–100%, zero position caps, and negative market-data freshness windows.
- Live-mode validation requires all five explicit approvals: human approval, verified exchange account, withdrawals disabled, healthy kill switch, and healthy reconciliation.
- `backend/rust-axum/src/execution_lifecycle.rs` models broker acknowledgements, partial fills, final fills, cancellation, rejection, failure, duplicate execution IDs, and overfill rejection.
- These are tested domain primitives only. They are not yet connected to a durable order store or an exchange adapter. The lifecycle's in-memory duplicate-ID set is not a substitute for persistent idempotency.
- `backend/rust-axum/src/trading.rs` now requires a valid side-aware stop trigger for Binance USD-M market entries, submits a reduce-only `STOP_MARKET` protective order for the actually filled quantity, and attempts an immediate reduce-only market close if stop placement fails. This adapter remains isolated from the public signal path and has no Axum order route. The adapter's configured per-order quantity/notional caps are not a portfolio risk engine.
- Live order submission remains disabled and is not wired into the Axum API.

## Mandatory operational gates
- Never expose the internal Axum service to the public internet.
- Never commit exchange credentials or request them in chat; use a secret manager and API keys with withdrawals disabled.
- Do not enable real-money trading without explicit human approval, verified account ownership, and a separate risk review.
- Use paper/testnet and shadow mode before any production consideration.
- Keep the system fail-closed if risk state, market data, broker connectivity, protective orders, reconciliation, or kill-switch health is unavailable.

## Target policy
- Risk per trade: 0.25% of equity, calculated from stop distance.
- Maximum leverage: 2x; enforce notional, correlated exposure, and position-count caps.
- Exchange-side protective stop, slippage limits, daily/weekly loss limits, drawdown circuit breaker, and kill switch.
- Durable idempotent client order IDs, timeout/partial-fill handling, exchange reconciliation, and crash recovery.
- Reject stale, out-of-order, malformed, or low-quality market data; abstain when spread or feed quality fails.
- Broker-specific tick/step-size validation, fees, minimum notional, margin mode, and instrument permissions.

The risk and lifecycle modules are not proof of a complete execution system. Until adapter integration, durable state, authentication, reconciliation, kill-switch operation, testnet lifecycle tests, and deployment controls are implemented and independently verified, the project must remain research/paper-only.
