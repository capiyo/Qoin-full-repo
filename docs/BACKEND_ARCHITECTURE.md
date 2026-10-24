# Backend architecture and ownership

## Request flow

`React / React Native client → Drogon public gateway → Axum engine → MongoDB`

### Drogon (C++)
Owns the public HTTP gateway, request routing/forwarding, request-level validation where appropriate, transport concerns, and normalized error responses. It does not calculate trading signals or mutate strategy weights independently.

### Axum (Rust)
Owns event validation and idempotency, state estimation, vector/attention geometry, strategy validation and evaluation, decision/risk metadata, and persistence. This is the only authoritative implementation of market decision logic.

### MongoDB
Stores the active versioned edge strategy map and the market events/states/decisions required by the engine. Clients and Drogon must not become competing sources of truth.

## Shared contract
The frontend consumes the API exposed through Drogon. `contracts/openapi.yaml` and `contracts/market-event.schema.json` define the wire format shared by the branches.

## Validation gate
Build/test Axum and Drogon independently, then run the MongoDB integration smoke test covering gateway health, strategy GET/PUT, persistence after Axum restart, and event-to-decision flow. Signals are research-only; live execution is not connected.
