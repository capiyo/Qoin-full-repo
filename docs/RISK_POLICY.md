# Risk policy and hard gates

Audit date: 2026-11-28  
Authoritative branch: `master`

## Current execution status

Qoin's vector and set/ledger algorithms produce research outputs. The private Axum source includes order-submission, reconciliation, and operator kill-switch routes, but this does not mean that live execution is available.

The runtime initializes `BrokerRegistry::default()`, which contains no registered provider adapters. The named provider adapter scaffolds return unsupported results and are deliberately not registered. The broker catalogue and configured environment-variable names are discovery/configuration metadata, not evidence of a connected account or executable provider. As a result, order dispatch must fail closed until an independently tested provider adapter is implemented and explicitly registered.

The private order handler additionally requires:
- A valid server-side execution token.
- A verified Firebase ID token and UID-derived owner scope.
- An active broker-account mapping owned by that UID, with trading enabled.
- A backend environment allowlist authorizing the broker account.
- An enabled operator trading control; the control defaults closed.
- A durable unique client-order intent before any adapter submission.
- The shared broker-neutral risk-preflight and adapter contract.

The order and reconciliation routes are internal Axum routes and must not be exposed through the public Drogon gateway. No real-money dispatch is authorized by this policy.

## Implemented safety foundations

- Firebase ID-token verification is performed server-side for private user-scoped operations. Client-supplied user IDs are not authoritative.
- Broker credentials are read from backend deployment environment secrets. They are not accepted from frontend clients or returned by status APIs.
- Broker account ownership is checked against the authenticated Firebase UID.
- Broker-neutral contracts describe normalized instruments, accounts, quotes, capabilities, margin, order requests, receipts, execution reports and adapter operations.
- The shared risk-preflight contract covers account-currency conversion, instrument multiplier and lot step, margin, daily P&L, gross exposure and position limits.
- Order intent persistence and client-order-ID deduplication are designed to prevent blind duplicate submissions.
- Reconciliation is required after uncertain adapter outcomes; an ambiguous outcome must never trigger an automatic blind retry.
- Execution lifecycle primitives model acknowledgements, partial fills, fills, cancellations, rejections, failures, duplicate execution IDs and overfills.
- Equal-notional allocation primitives are available, but their presence alone does not establish a production multi-account dispatcher.

These are software foundations, not proof of production execution safety.

## Mandatory fail-closed rules

- Keep the broker registry empty until a concrete adapter passes its complete conformance and sandbox test suite.
- Never enable real-money dispatch based only on a provider appearing in the broker catalogue or having environment variables configured.
- Never expose internal Axum routes directly to the public internet or proxy private order routes through Drogon.
- Never accept broker secrets from the browser or store them in frontend state, Firestore profiles, logs, or API responses.
- Derive account ownership from verified Firebase identity and operator-provisioned account mappings.
- Reject stale, malformed, out-of-order, non-finite, or venue-mixed market data.
- Reject missing, stale, or incorrectly directed FX conversion inputs.
- Reject invalid risk policy, unknown broker capabilities, missing protective-order support, or unavailable risk state.
- Persist order intent before dispatch. On timeout, partial failure, persistence failure, or unknown broker outcome, require authoritative reconciliation before any retry.
- Keep the operator trading control closed by default. A kill switch must not be treated as proof that existing positions or orders have been cancelled or flattened.
- Keep research signals and V1/V2 algorithm outputs separate from execution authorization.

## Gates before any future live-execution consideration

1. Implement a concrete adapter for each intended provider. Each adapter must retrieve account, instrument, quote, margin and capability data from the provider and implement submit, lookup, cancellation and reconciliation semantics.
2. Prove provider account ownership, trading permissions, credential rotation, withdrawal restrictions and account allowlisting.
3. Run provider-specific sandbox/testnet conformance tests covering rejects, timeouts, partial fills, duplicate requests, stop placement, cancellation, and restart recovery.
4. Wire aggregate portfolio risk reservation across all selected accounts before dispatching any child order.
5. Implement durable parent/child order state, concurrent dispatch controls, idempotent reconciliation workers and explicit partial-failure handling.
6. Independently review the risk model, kill-switch operations, deployment isolation, audit trail, monitoring, incident response and recovery procedures.
7. Require a separate explicit authorization and release decision. Passing software tests alone does not authorize real-money trading.

## Research and paper-trading limits

Backtests and paper sessions are simulations. They must disclose data coverage, venue, timestamp gaps, fees, slippage, fill assumptions, missing market microstructure, funding treatment, leverage assumptions and limitations. They must not be described as proof of profitability or as equivalent to live execution.

Qoin remains research/paper-only. Live order dispatch is not currently available and must remain fail-closed.
