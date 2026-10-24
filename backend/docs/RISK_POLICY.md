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
