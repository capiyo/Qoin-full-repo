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

If an equal share falls below even one selected account's minimum quantity or minimum notional, the equal-split plan is rejected. Qoin then selects the best feasible single account among the selected execution-ready accounts. If no single account can execute the full order, the current policy returns a no-feasible-account error rather than silently dropping a selected target.

## Example

With a portfolio notional of 100 and 15 selected accounts, the equal target is approximately 6.67 per account. If any account has a minimum order notional of 10, the 15-way split is infeasible. If one selected account can execute the full 100, the policy consolidates there.

With larger capital, all 15 child orders may become feasible. Qoin still splits only if the measured child-size slippage and other estimated execution costs beat the best consolidated route after overhead and the configured saving hurdle.

## Execution safety boundary

A routing plan is **not** a submitted order. Before dispatch, the execution layer must refresh and validate every selected account, instrument, quote, adapter capability, aggregate portfolio risk and margin requirement; persist the parent and all child intents with idempotency keys; then dispatch and reconcile each child independently. After any child has been submitted, multi-broker execution is not an atomic transaction. Partial fills, rejects, timeouts and unknown outcomes must remain visible and be reconciled rather than blindly retried.

The current integration directory is not equivalent to live account connectivity. The branch currently reports one execution adapter (Binance USD-M), several market-data-only providers and pending adapters. User account linking and multi-account execution are not enabled by this allocation-policy work.
