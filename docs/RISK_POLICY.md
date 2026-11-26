# Risk policy and hard gates

## Current implementation status

### Active integration branch update — 2026-11-26

On `qoin/backend-completion-auth`, the broker-neutral `submit_risk_checked` path now invokes both:
1. the Decimal account-currency portfolio gate (`validate_portfolio_order`), including quote freshness, instrument multiplier/precision, FX conversion, broker margin, exposure, daily loss and protective-order checks; and
2. the historical `order_safety::validate_order` gate from commit `3bef7b0c`, after converting the instrument quantity and reference/stop prices into account-currency exposure.

The second gate is a stricter pre-dispatch approval layer, not a substitute for the Decimal portfolio validator. It requires all five deployment-managed flags to be explicitly true:
`QOIN_LIVE_HUMAN_APPROVED`, `QOIN_LIVE_ACCOUNT_VERIFIED`, `QOIN_LIVE_WITHDRAWALS_DISABLED`, `QOIN_LIVE_KILL_SWITCH_HEALTHY`, and `QOIN_LIVE_RECONCILIATION_HEALTHY`. All are false in `.env.example`. The private order route additionally requires operator authorization and the persisted kill switch. No provider adapter is registered in the runtime registry, so live dispatch remains unavailable even if the approval flags are configured.

The gateway now applies bounded per-peer fixed-window limits to proxied HTTP requests and WebSocket connection attempts: 120 read/handshake requests and 30 writes per 60 seconds, with a bounded 20,000-key peer table and HTTP 429 / Retry-After response for HTTP limit violations. Axum's production router caps JSON bodies at one MiB; the standalone loopback research API caps bodies at 64 KiB. The current gateway limiter uses the direct peer address; deployments behind proxies must preserve source addresses or add an explicitly trusted proxy-address policy before relying on per-client limits.

- The Axum vector endpoint emits research signals only. Live order submission is a separate private route (`POST /internal/orders`) and is not reachable through the public Drogon gateway.
- The public market WebSocket (`/api/v1/stream`) forwards the caller's Firebase Bearer token through Drogon; Axum verifies the Firebase identity before accepting the WebSocket upgrade. Missing credentials or an unconfigured Firebase verifier fail closed. This authenticates stream access; market observations remain public-market data, not user-private portfolio data.
- The public Drogon gateway does not register order submission or order reconciliation routes. Those operator endpoints remain on the private Axum service and must not be exposed by a reverse proxy or deployment rule.
- Broker-account provisioning is also operator-only and has no public Drogon route. The private Axum endpoint requires the execution operator token and an environment allowlisted broker subaccount.
- `backend/rust-axum/src/order_safety.rs` implements deterministic pre-trade checks for identity, finite values, stop direction, risk-at-stop, order notional, gross leverage, daily loss, open-position count, market-data age, and exchange-side protective-stop capability. It rejects malformed policy configuration before order evaluation, including risk/daily-loss/notional fractions outside 0–100%, zero position caps, and negative market-data freshness windows.
- A new provider-neutral contract lives in `backend/rust-axum/src/broker_contract.rs`. It defines normalized instruments, account snapshots, order requests, broker capabilities, receipts, execution reports, a broker registry whose eligible accounts are selected by the user, and an async `BrokerAdapter` interface. Its `submit_risk_checked` pipeline retrieves the selected adapter's account, instrument and capability state, validates the portfolio risk gate, then submits through that adapter. Its shared portfolio gate calculates notional and stop risk in the selected account currency using an explicit, freshness-checked quote-to-account FX conversion, instrument multiplier/lot step, broker-calculated margin requirement, daily realized plus daily unrealized P&L, gross exposure and position caps. Unit tests cover Forex, equities and non-USDT crypto instrument examples. This is the active private Axum order-submission preflight and is provider-neutral. The runtime registry is intentionally empty and no concrete provider adapter is execution-ready, so the route fails closed before any provider dispatch.
- Live-mode validation requires all five explicit approvals: human approval, verified exchange account, withdrawals disabled, healthy kill switch, and healthy reconciliation.
- `backend/rust-axum/src/execution_lifecycle.rs` models broker acknowledgements, partial fills, final fills, cancellation, rejection, failure, duplicate execution IDs, and overfill rejection.
- The lifecycle state machine validates broker acknowledgements on the private order route. A MongoDB append-only `order_lifecycle_events` collection records owner-scoped submission, acceptance, rejection and reconciliation states with idempotent state keys. Partial-fill/execution-report ingestion and immutable per-execution persistence are still incomplete; the in-memory execution-ID set is not a substitute for durable execution deduplication.
- `backend/rust-axum/src/trading.rs` now requires a valid side-aware stop trigger for Binance USD-M market entries, submits a reduce-only `STOP_MARKET` protective order for the actually filled quantity, and attempts an immediate reduce-only market close if stop placement fails. The private Axum route requires `x-qoin-execution-token`, either `QOIN_TRADING_MODE=testnet` with `QOIN_TESTNET_TRADING_CONFIRM=I_UNDERSTAND_TESTNET_ORDERS` or `QOIN_TRADING_MODE=live` with `QOIN_LIVE_TRADING_CONFIRM=I_UNDERSTAND_REAL_MONEY_RISK`, and Binance API credentials. Testnet defaults to Binance Futures testnet; live defaults to Binance production. The adapter's configured per-order quantity/notional caps are not a portfolio risk engine.
- A MongoDB-backed operator gate (`POST /internal/trading/kill-switch`) can enable or block new order submissions at runtime. It is deliberately a new-entry gate: it does not cancel existing exchange orders or flatten open positions.
- The route writes a unique `client_order_id` intent to MongoDB before contacting the exchange. Duplicate IDs are rejected; uncertain outcomes are marked `reconciliation_required` and must not be blindly retried. `GET /internal/orders/{client_order_id}/reconcile` performs an authenticated Binance entry/protective-stop lookup and persists the observed exchange state. This is durable request idempotency plus operator-triggered reconciliation, not an automatic reconciliation worker.
- Live order submission remains disabled and is not wired into the Axum API.

## Mandatory operational gates
- Never expose the internal Axum service to the public internet.
- Never commit exchange credentials or request them in chat; use a secret manager and API keys with withdrawals disabled.
- The private live route requires an operator token, explicit live-mode confirmation, and exchange credentials. API keys must have withdrawals disabled and must be scoped to the intended account.
- Use paper/testnet and shadow mode before any production consideration.
- Keep the system fail-closed if risk state, market data, broker connectivity, protective orders, reconciliation, or kill-switch health is unavailable.

## Target policy
- Risk per trade: 0.25% of equity, calculated from stop distance.
- Maximum leverage: 2x; enforce notional, correlated exposure, and position-count caps.
- Exchange-side protective stop, slippage limits, daily/weekly loss limits, drawdown circuit breaker, and kill switch.
- Durable idempotent client order IDs, timeout/partial-fill handling, exchange reconciliation, and crash recovery.
- Reject stale, out-of-order, malformed, or low-quality market data; abstain when spread or feed quality fails.
- Broker-specific tick/step-size validation, fees, minimum notional, margin mode, and instrument permissions.

The risk and lifecycle modules and the current private order route are not proof of a complete autonomous execution system. Portfolio-level pre-trade risk integration, broker account/position reconciliation, an operational kill switch, exchange testnet lifecycle verification, order amendment/cancellation flows, and deployment controls remain outstanding. The current route is an explicitly invoked Binance USD-M adapter with a strict per-order cap and protective-stop attempt; it must not be described as an unattended production trading system.


## Equal multi-broker allocation

- A parent strategy order is a total portfolio-currency exposure budget. If the user selects N eligible broker accounts (currently bounded to 15), Qoin targets exactly 1/N of that total notional for each account; it does not copy the full quantity N times.
- `backend/rust-axum/src/broker_allocation.rs` implements a Decimal-based equal-notional allocation primitive. It converts each account's equal portfolio-notional share into that broker instrument's native quantity using the supplied portfolio value per native unit, then rounds quantity DOWN to the broker's quantity step.
- The allocator rejects duplicate broker/account pairs, invalid targets, more than 15 recipients, and shares that cannot meet an account's minimum quantity/notional. It reports the unallocated remainder from quantity-step rounding; it never redistributes that remainder to another account.
- Different broker symbols, contract multipliers, lot conventions, quote currencies, settlement currencies and account currencies must be normalized into the common portfolio currency before allocation. The caller must supply a fresh, correctly directed conversion chain; missing or stale conversion inputs must fail closed.
- The allocator is currently a tested domain primitive, not wired into the live `/internal/orders` route. The route remains Binance-only. The frontend broker collection is a status catalogue, not an account-linking or order-submission integration. Do not describe multi-broker live fan-out as available until per-account authentication, aggregate portfolio risk reservation, durable parent/child idempotency, concurrent dispatch, partial-failure handling and reconciliation are integrated and verified.
- Before dispatching ANY child, Qoin must reserve/check aggregate exposure and risk across all selected accounts. Child-order failure or timeout must not cause an untracked duplicate order or an automatic increase in another account's allocation.
