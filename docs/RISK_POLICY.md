# Risk policy and hard execution gates

## Non-negotiable release policy

Real-money execution is disabled by code policy. The only permitted execution modes before release acceptance are paper and exchange testnet. Setting `QOIN_TRADING_MODE=live`, adding credentials, or opening the operator gate must not enable production orders.

The private Axum execution route is not exposed through Drogon. Never expose the Axum service directly to the public internet. Never place exchange credentials in source control, frontend configuration, logs, or chat. Use a secret manager and exchange keys with withdrawals disabled.

## Current implementation (not production approval)

- `backend/rust-axum/src/trading.rs` contains an explicitly invoked Binance USD-M adapter. It supports testnet configuration, per-order quantity/notional caps, side-aware protective stop validation, reduce-only stop placement for the filled quantity, and an emergency reduce-only close attempt if stop placement fails.
- `POST /internal/orders` requires the private execution token, a persistent MongoDB new-entry gate, a unique client order ID, testnet configuration and Binance credentials. The route writes an intent before contacting the exchange and rejects duplicate client order IDs.
- Uncertain outcomes are marked `reconciliation_required`. `GET /internal/orders/{client_order_id}/reconcile` performs an operator-triggered exchange lookup. It is not an automatic reconciliation worker.
- `POST /internal/trading/kill-switch` blocks new entries. It does not cancel open orders, flatten positions, or guarantee emergency risk reduction.
- `order_safety.rs` contains deterministic risk-validation primitives. They are not fully wired into the order-submission route. The current per-order caps are not portfolio-level risk controls.
- `execution_lifecycle.rs` contains tested domain state transitions, but the full durable exchange lifecycle is not yet integrated end to end.

## Mandatory gates before any live execution is considered

1. Connect the portfolio-level risk validator to every order path; prove no bypass exists.
2. Verify exchange account ownership and permissions server-side.
3. Persist a complete order state machine: intent, submit, acknowledgement, partial fill, fill, cancel, amend, reject, timeout and recovery.
4. Reconcile orders, balances and positions automatically and safely after outages/restarts.
5. Implement an operational kill switch that can cancel/manage open orders and flatten exposure under a separately tested policy.
6. Synchronize account balances, positions, margin, funding and instrument constraints.
7. Pass Binance testnet end-to-end tests, including partial fills, stop failures, cancellation, amendment, timeouts, duplicate requests and process restart.
8. Complete independent security, operational and risk review; verify monitoring, alerting, backups, restore and incident procedures.
9. Verify the exact release commit in CI and production-like staging. Obtain explicit human approval for any future live-mode activation.

## Target portfolio policy

- Target risk per trade: 0.25% of equity, sized from stop distance and instrument contract rules.
- Maximum leverage: 2x unless a stricter instrument/account policy applies.
- Enforce gross/net exposure, correlated concentration, currency exposure, position-count, daily/weekly loss and drawdown limits.
- Reject stale, out-of-order, malformed or incomplete market data. Abstain when feed quality, spread, liquidity or protective-order capability is insufficient.
- Enforce tick size, lot step, minimum notional, contract multiplier, margin mode, funding, fees and trading-session rules from verified instrument metadata.
- Fail closed when risk state, account ownership, market data, broker connectivity, persistence, reconciliation or kill-switch health is unavailable.

Until every mandatory gate is implemented and independently verified, Qoin is a research and paper/testnet system. No unattended or production-safe execution claim is permitted.
