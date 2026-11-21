# Qoin capital-aware broker allocation

## Routing objective

Qoin supports a maximum of 20 selected broker-account targets for one parent order. The routing policy prefers a feasible consolidated order. It considers an equal split only when **every selected target** is execution-ready and healthy, each child meets that account's instrument quantity/notional minimums, each account has sufficient estimated margin, and the equal-split route has a lower estimated all-in cost after the configured multi-account overhead and minimum-saving hurdle.

A broker being present in the directory or providing market data does not make it execution-ready. Only a real authenticated adapter/account with the required instrument and order-type capability can be considered.

## Cost comparison

The caller supplies fresh estimates in the portfolio currency:

- variable commission in basis points and the broker's per-order minimum commission;
- full-parent-order slippage and expected slippage for that account's smaller equal-split child;
- currency-conversion cost;
- margin requirement and available margin;
- the operational/reconciliation overhead and minimum saving hurdle.

The split slippage estimate must be derived for the child order size from a fresh market-depth/quote snapshot. The policy does not assume that splitting improves slippage. The consolidated and split cost estimates are normalized to the same parent notional before comparison, so a route cannot appear cheaper merely because quantity-step rounding leaves more capital unallocated.

## Sizing and residuals

Each selected target receives an equal **target notional**, not an equal native quantity. The caller converts one native quantity unit into portfolio-currency value using the instrument price, contract multiplier and currency-conversion chain. Qoin rounds each child quantity down to the broker's quantity step. The resulting remainder stays unallocated; it is never redistributed and the parent notional is never exceeded.

If an equal share falls below even one selected account's minimum quantity or minimum notional, the equal-split plan is rejected. Qoin then selects the best feasible single account among the selected execution-ready accounts. If no single account can carry the parent order but every selected account can safely carry its equal child (including margin), Qoin may select the split as the only feasible route. If neither the consolidated nor the complete equal-split route is feasible, the policy fails closed; it never silently drops an account.

## Example

With a portfolio notional of 100 and 15 selected accounts, the equal target is approximately 6.67 per account. If any account has a minimum order notional of 10, the 15-way split is infeasible. If one selected account can execute the full 100, the policy consolidates there.

With larger capital, all 15 child orders may become feasible. Qoin still splits only if the measured child-size slippage, per-order minimum commissions, variable fees and conversion costs beat the best consolidated route after overhead and the configured saving hurdle. If no single account has enough margin or order capacity, a complete feasible split can be chosen without pretending an infeasible consolidated comparator exists.

## Aggregate portfolio risk

A new Decimal-based aggregate gate requires a fresh portfolio-currency snapshot and fresh market-data timestamp before a parent plan can be converted into child intents. It checks the full parent notional, projected gross exposure, portfolio leverage, open-position cap and maximum loss at the protective stop. It also reserves that order's maximum stop loss against the remaining daily-loss budget: current daily loss plus the proposed stop loss must remain strictly below the configured daily cap. The approval exposes projected daily loss and the headroom remaining after this reservation. Each adapter must still repeat its own account/instrument/margin/capability checks for every child. Reduce-only requests are conservatively treated as not increasing gross exposure; the broker must independently enforce reduce-only semantics.

## Parent/child lifecycle foundation

The Rust fan-out domain now builds deterministic parent and child IDs from a server-verified owner scope plus the caller's idempotency key. Equal-split plans must include every selected account exactly once. Consolidated plans explicitly record which selected accounts receive no child order. Parent records preserve allocated notional and the unallocated rounding remainder. Child lifecycle states include submitting, accepted, partial fill, filled, cancel pending, cancelled, rejected, failed and unknown. Execution IDs are idempotent; an unknown submission outcome cannot be retried until reconciliation resolves it. The Mongo persistence helper stores the parent and all child intents in one atomic parent document, creates unique indexes for parent ID, owner/idempotency key and child client-order ID, and distinguishes a newly created order from an existing idempotent request. Only a receipt for a newly inserted document can unlock the in-memory coordinator; an existing order cannot be dispatched again.

These are serializable, testable domain foundations only. Initial parent/child insertion is atomic in one Mongo document; parent and child transition helpers use optimistic revision checks, and a reload helper restores lifecycle state and refuses inconsistent or partially omitted child records. The coordinator must persist each state transition and reload after a revision conflict. The persistence helpers are not wired to an API route or AppState collection. Verified Firebase owner-scope extraction, adapter registration for authenticated user accounts, concurrent adapter dispatch and live reconciliation are not connected. Do not enable or infer live fan-out from these types.

## Execution safety boundary

A routing plan is **not** a submitted order. The generic broker contract uses Decimal for quantities, prices, currency conversions, account values, fees and risk limits; it validates instrument quantity steps and price ticks using checked arithmetic. Each adapter must report account health, trading permission and broker-native margin for the exact child order. A concurrent all-selected-account preflight helper gathers every account, instrument, capability and margin snapshot and applies the account risk gate without submitting anything. A failed child preflight invalidates the entire selected set. Before dispatch, the execution layer must persist the parent and all child intents with idempotency keys and persist the dispatch-started transition; only then may it dispatch and reconcile each child independently. After any child has been submitted, multi-broker execution is not an atomic transaction. Partial fills, rejects, timeouts and unknown outcomes must remain visible and be reconciled rather than blindly retried.

The current integration directory is not equivalent to live account connectivity. The branch currently reports one execution adapter (Binance USD-M), several market-data-only providers and pending adapters. The Decimal allocation and capital-aware plan selector are pure planning primitives; they are not yet wired into the live `/internal/orders` route. User account linking, a per-user credential vault, a multi-account adapter registry in Axum, durable parent/child fan-out records and concurrent child dispatch remain prerequisites. Do not infer that selecting directory entries enables live execution.


## Provider directory and selection limit

Qoin exposes a 20-entry broker/exchange directory through `GET /api/v1/brokers` (Axum `GET /internal/brokers`). Selected IDs from web and mobile are sent to `GET /api/v1/trading/readiness?brokers=...` (Axum `GET /internal/trading/readiness`) to report account count, provider readiness and dispatch blockers. This endpoint is a pre-trade gate only; it never submits an order. Web and React Native load this same API catalogue. Selection preferences are capped at 20 and stored under the signed-in user's own Firebase `users/{uid}.brokerSelections` profile field, protected by the existing owner-only Firestore rules. Guest preferences remain device-local.

The 20 directory entries are intentionally balanced: ten crypto venues (Binance USD-M, Coinbase Advanced Trade, Kraken Spot, Bybit, OKX, Bitget, Gate.io, KuCoin, Deribit and Hyperliquid) and ten forex/CFD brokers (OANDA, IG, FXCM, Capital.com, Deriv, XTB, Pepperstone, IC Markets, FOREX.com and CMC Markets). The API reports `market_balance: { crypto: 10, forex: 10 }`.

Directory selection is not account linking or execution eligibility. The frontend now calls the readiness endpoint whenever selection changes; the current response correctly reports zero connected accounts, zero execution-ready providers, and dispatch disabled. The current Binance USD-M executor is not registered as a generic multi-account adapter and has no verified per-user account connection. All entries are returned with `execution_ready=false`; other provider adapters remain pending. No market-data provider or pending provider can receive an order. Qoin remains research-only until each adapter, user authorization, credential vault, account snapshot, instrument metadata, order lifecycle and reconciliation path is implemented and tested.
