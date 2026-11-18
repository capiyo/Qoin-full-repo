# Qoin capital-aware broker allocation

## Routing objective

Qoin supports a maximum of 15 selected broker-account targets for one parent order. The routing policy prefers a feasible consolidated order. It considers an equal split only when **every selected target** is execution-ready and healthy, each child meets that account's instrument quantity/notional minimums, each account has sufficient estimated margin, and the equal-split route has a lower estimated all-in cost after the configured multi-account overhead and minimum-saving hurdle.

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

A new Decimal-based aggregate gate requires a fresh portfolio-currency snapshot and fresh market-data timestamp before a parent plan can be converted into child intents. It checks the full parent notional, projected gross exposure, portfolio leverage, daily-loss budget, open-position cap and maximum loss at the protective stop. Each adapter must still repeat its own account/instrument/margin/capability checks for every child. Reduce-only requests are conservatively treated as not increasing gross exposure; the broker must independently enforce reduce-only semantics.

## Parent/child lifecycle foundation

The Rust fan-out domain now builds deterministic parent and child IDs from a server-verified owner scope plus the caller's idempotency key. Equal-split plans must include every selected account exactly once. Consolidated plans explicitly record which selected accounts receive no child order. Parent records preserve allocated notional and the unallocated rounding remainder. Child lifecycle states include submitting, accepted, partial fill, filled, cancel pending, cancelled, rejected, failed and unknown. Execution IDs are idempotent; an unknown submission outcome cannot be retried until reconciliation resolves it. The Mongo persistence helper stores the parent and all child intents in one atomic parent document, creates unique indexes for parent ID, owner/idempotency key and child client-order ID, and distinguishes a newly created order from an existing idempotent request. Only a receipt for a newly inserted document can unlock the in-memory coordinator; an existing order cannot be dispatched again.

These are serializable, testable domain foundations only. The Mongo persistence helper exists but is not wired to an API route or AppState collection. Verified Firebase owner-scope extraction, adapter registration for authenticated user accounts, concurrent adapter dispatch and live reconciliation are not connected. Do not enable or infer live fan-out from these types.

## Execution safety boundary

A routing plan is **not** a submitted order. Before dispatch, the execution layer must refresh and validate every selected account, instrument, quote, adapter capability, aggregate portfolio risk and margin requirement; persist the parent and all child intents with idempotency keys; then dispatch and reconcile each child independently. After any child has been submitted, multi-broker execution is not an atomic transaction. Partial fills, rejects, timeouts and unknown outcomes must remain visible and be reconciled rather than blindly retried.

The current integration directory is not equivalent to live account connectivity. The branch currently reports one execution adapter (Binance USD-M), several market-data-only providers and pending adapters. The Decimal allocation and capital-aware plan selector are pure planning primitives; they are not yet wired into the live `/internal/orders` route. User account linking, a per-user credential vault, a multi-account adapter registry in Axum, durable parent/child fan-out records and concurrent child dispatch remain prerequisites. Do not infer that selecting directory entries enables live execution.
