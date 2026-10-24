# Risk policy and hard gates

## Mandatory operational gates
- Live order submission is disabled and is not wired into the Axum API.
- Never expose the internal Axum service to the public internet.
- Never commit exchange credentials; use a secret manager and keys with withdrawals disabled.
- Do not enable real-money trading without explicit human approval and a separate risk review.
- Use paper/testnet and shadow mode before any production consideration.

## Implemented controls in the current research branch
- Axum ingestion rejects event timestamps that do not advance per symbol, enforces contiguous sequence numbers whenever a stream supplies them, and uses unique venue/symbol/event IDs for duplicate detection. In `live` feed mode it also enforces server-arrival freshness; `replay` mode permits historical timestamps for deterministic research and never enables order submission.
- Flow, event intensity, and liquidation pressure use explicit exponential half-life decay; book imbalance is stored separately from trade-flow accounting.
- Entry signals require fresh price/book state, eligible event types, and three consecutive same-direction candidate ticks.
- Strategy entries fail closed unless calibrated expected-edge, fee, spread, slippage, and cost-multiplier inputs pass the cost gate and a fresh bull/bear/range regime classification meets a separately configured confidence threshold.
- Axum and Drogon write endpoints require separate bearer tokens (`QOIN_INGEST_TOKEN` and `QOIN_ADMIN_TOKEN`); empty tokens deny writes.
- The independent risk module fails closed without portfolio context. The current API does not yet provide reconciled portfolio context, so candidate entries are vetoed.

## Target policy (not fully implemented)
- Risk per trade: 0.25% of equity, calculated from stop distance.
- Maximum leverage: 2x; enforce notional, correlated exposure, and position-count caps.
- Exchange-side protective stop, slippage limits, daily/weekly loss limits, drawdown circuit breaker, and kill switch.
- Idempotent client order IDs, timeout/partial-fill handling, exchange reconciliation, and crash recovery.
- Reject stale, out-of-order, malformed, or low-quality market data; abstain when spread or feed quality fails.

These are target requirements, not claims about current code. Until these controls are implemented and independently tested, the project must remain research/paper-only.
