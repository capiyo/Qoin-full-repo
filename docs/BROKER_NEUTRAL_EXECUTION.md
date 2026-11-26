# Broker-neutral execution architecture

## Decision

Qoin's order boundary is broker- and asset-class-neutral. Vector V1 and Set + Ledger V2 must never call a provider SDK or construct provider-specific order payloads. Both engines produce an intent that is normalized into the shared execution contract in `backend/rust-axum/src/broker_contract.rs`.

Binance is not the execution core. The old Binance-only order handler and executor have been removed from the active Axum runtime. Provider-specific behavior belongs only in separately registered implementations of `BrokerAdapter`.

## Runtime path

1. A strategy emits a proposed order intent. It does not choose API endpoints or hold credentials.
2. The intent is normalized to `NormalizedOrder` with a stable Qoin instrument ID, account ID, side, order type, native quantity, prices, time-in-force and reduce-only semantics.
3. The caller selects a `broker_id`; Axum resolves that ID through `BrokerRegistry`.
4. The adapter supplies the authenticated account snapshot, broker instrument specification, capabilities and broker-calculated margin.
5. Shared validation checks account health/trading permission, instrument tick and quantity steps, quote and conversion freshness, account-currency notional, stop distance, margin, leverage, daily loss and position limits.
6. Qoin persists the idempotent order intent before invoking `submit_order`.
7. The adapter receives the protective stop separately from the order-type trigger and must submit the entry plus native attached protection atomically. Providers that cannot guarantee native protection are rejected for opening orders.
8. The broker acknowledgement is persisted. Timeouts, mismatches or failed persistence require client-order-ID reconciliation; they must never trigger blind resubmission.

The public gateway does not expose order submission or reconciliation. Those Axum paths remain private and fail closed until the complete operator release gates pass.

## Contract coverage

- Asset classes: Forex, crypto, equity, ETF, commodity, index, fixed income, derivatives and other.
- Quantity units: base units, standard lots, shares, contracts and generic units.
- Order types: market, limit, stop and stop-limit.
- Instrument-specific contract multipliers, tick sizes, quantity steps, minimum quantities/notionals, quote/settlement currencies and timestamped FX conversion into account currency.
- Account snapshots, margin requirements, capabilities, receipts, order status, cancellation and authoritative client-order-ID reconciliation.
- Shared risk validation has no USD or USDT assumption.

## API shape

`POST /internal/orders` is a private Axum-only route and is not proxied by the public Drogon gateway. Its normalized request shape, when an operator explicitly enables the private execution path, is:

```json
{
  "broker_id": "provider_stable_id",
  "order": {
    "client_order_id": "qoin-unique-order-id",
    "account_id": "provider-account-reference",
    "instrument_id": "qoin-canonical-instrument-id",
    "side": "buy",
    "order_type": "market",
    "quantity": "0.01",
    "limit_price": null,
    "stop_price": null,
    "time_in_force": "GTC",
    "reduce_only": false
  },
  "preflight": {
    "protective_stop_price": "1.0800"
  }
}
```

Decimal fields are serialized as decimal strings by the Rust decimal serializer. The client supplies no reference quote or timestamp: the selected provider adapter returns fresh bid/ask and the Axum server supplies the current time. The protective stop is passed to the provider as an attached bracket instruction, not confused with a stop-entry trigger. The execution token is required at the internal service boundary.

`GET /internal/orders/{client_order_id}/reconcile` is private to the Axum service and resolves the adapter from the persisted broker ID for authoritative lookup.

## Current honest capability state

The generic contract, owner-scoped adapter interface, runtime registry, server-sourced quote preflight, native protective-stop submission contract, shared risk-checked submission sequence, durable intent-before-submit path, Firebase identity verification, owner-scoped reconciliation, server-side broker environment loader, and operator-provisioned account mapping are implemented. Broker secrets are read only from QOIN_BROKER_* deployment environment variables, zeroized on drop, never serialized, and never returned by status endpoints. Account IDs must also be present in each provider's backend allowlist. The registry starts empty. No Forex or crypto provider adapter is currently registered, and dispatch remains disabled. The previous Binance-only order implementation has been removed rather than silently retained as the universal path.

The broker catalogue is a discovery/selection directory, not a claim that all listed APIs are implemented. A provider becomes executable only after its adapter implements every required contract method, passes provider-specific conformance and sandbox tests, proves account ownership and trading permissions, and is registered by the backend using deployment environment secrets. Broker accounts are mapped to Firebase UIDs only by an operator-authorized private provisioning route after the account ID passes the server-side allowlist. Users never submit broker credentials, and the public gateway does not expose credential connection or live-order routes.

## Release gates

- Verify Firebase identity server-side and derive owner scope from the verified identity; never trust a client-supplied user ID.
- Implement a separately tested adapter per provider/API family, including credential rotation and deployment-secret rotation; adapter code must retrieve secrets only through BrokerEnvironment accessors.
- Verify account identity, permissions, instrument mapping, leverage/margin, lot/contract sizing, tick/step rules, fees, funding, protective orders, cancel/status and complete idempotency lookup.
- Run sandbox conformance tests, partial-fill/rejection/timeout/restart tests, duplicate-submit tests, ledger reconciliation and kill-switch tests.
- Keep live dispatch disabled until the operator explicitly authorizes the deployment after those gates pass.

No live-money readiness is implied by the presence of the generic route or provider catalogue.


## Provider adapter skeletons (2026-11-19)

- Added named Rust adapter types for all 20 directory providers, using the common `BrokerAdapter` contract.
- Every scaffold returns `Unsupported` for account discovery, capabilities, instrument metadata, quotes, margin and order operations. The scaffolds are deliberately not registered in `BrokerRegistry`.
- The catalogue reports `adapter_scaffolded`, while `execution_ready` remains false. A scaffold is not a provider integration and cannot submit, cancel or reconcile orders.
- Provider credentials are supplied only through deployment secret management. No credential entry, browser encryption, credential upload, or user-managed broker connection exists in the frontend. Operator account provisioning is separate from user Firebase authentication.
- Provider activation remains per-provider and fail-closed until API authentication, account ownership, instrument/precision mapping, fresh pricing, margin, native attached protection, cancellation, authoritative idempotency lookup and sandbox conformance are all verified.
