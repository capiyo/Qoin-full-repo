# Qoin equal multi-broker allocation policy

## User-selected routing is authoritative

Qoin supports up to 15 broker-account targets per parent strategy order. The user selects the broker accounts in the frontend. If N accounts are selected, Qoin targets exactly 1/N of the parent order's portfolio-currency notional at each selected account. It must not silently consolidate the order into one broker, prefer a broker by cost, or omit a selected account.

A broker directory entry or market-data connection is not an executable account. Each selected target must have a verified linked account, a healthy authenticated execution adapter, the instrument and order-type capability, a fresh quote and conversion path, and sufficient available margin. If any selected target is not execution-ready, the complete multi-account dispatch is rejected before any child order is submitted. The UI must identify the unavailable account and reason; it must not quietly route to the remaining accounts.

## Equal notional, broker-native quantities

Equal allocation means equal target notional in the common portfolio currency, not equal native quantity. For each account, convert its 1/N share into the broker's native quantity using the instrument price, contract multiplier, lot convention and a fresh, correctly directed FX/conversion chain into that account's quote or settlement currency.

Round each child quantity down to that broker's quantity step. Keep the resulting rounding remainder unallocated. Never redistribute it to another broker and never let aggregate child notional exceed the parent notional. Reject the whole plan if any child falls below that account's minimum quantity/notional or cannot satisfy margin and risk checks.

Example: a 100-unit parent notional selected across 15 eligible accounts has a target of 6.666… portfolio-currency units per account. The native quantities may differ because prices, multipliers, lot steps and account currencies differ. If any account cannot meet its minimum, the 15-way plan is infeasible and must be rejected; do not consolidate or silently drop that account.

## Pre-dispatch risk reservation

Before dispatching any child, refresh all selected account and instrument snapshots and calculate aggregate portfolio exposure, correlated exposure, currency exposure, stop risk, margin, daily loss and position-count limits across the entire selected set. Reserve the aggregate risk budget atomically before any network submission. If reservation or any account check fails, submit zero child orders and release any reservation safely.

## Parent/child order lifecycle

Persist one parent intent and N child intents before dispatch. Each child has a stable, unique idempotency key derived from the parent and broker-account identity. Dispatch all eligible child orders concurrently; do not wait for one broker's acknowledgement before beginning the others. Track acknowledgements, partial fills, final fills, rejects, cancellations, timeouts and unknown outcomes independently.

Multi-broker submission is not an atomic transaction. On partial failure, preserve the actual exposure and all child states, stop unsafe further actions, and reconcile every uncertain child against its broker before retrying. Never blindly retry an unknown outcome, increase another account's share, or claim that all brokers filled merely because requests were sent. A parent is complete only when every child reaches a reconciled terminal state and aggregate filled notional / residual is reported.

## Currency and ledger rules

Keep each account's native cash, fees, margin, realized/unrealized P&L and settlement currency distinct. Convert reporting values into the user's portfolio currency only with timestamped, source-attributed FX rates and explicit conversion costs. Do not assume USD, KSh, USDT or any other currency is interchangeable. Missing, stale or ambiguous conversion data fails closed.

## Current implementation boundary

The Decimal equal-notional allocator is a tested planning primitive. The provider-neutral broker contract and risk/lifecycle types are domain foundations. They do not yet establish that multiple live accounts can be linked or that multi-broker orders can be dispatched. The existing private Binance USD-M order route is a separate single-adapter path; the multi-broker parent/child lifecycle, account credential vault, adapter implementations, aggregate reservation, concurrent dispatch and automatic reconciliation worker remain unfinished. Keep live multi-broker execution disabled until those components and testnet end-to-end tests pass.

Never request broker secrets in chat or commit them. Store credentials only in a server-side secret vault, use least-privilege keys with withdrawals disabled, and keep paper/testnet as the default.