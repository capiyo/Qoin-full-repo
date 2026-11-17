# Qoin architecture

## Ownership

- **Axum (Rust)** is the sole authority for event validation, bounded state estimation, feature construction, deterministic vector decisions, strategy-map validation and MongoDB writes.
- **Drogon (C++)** is the HTTP gateway. It forwards event, state, vector and strategy requests to Axum; it contains no independent model, risk or order logic.
- **MongoDB** is the durable source of truth for raw market events, state snapshots, vector decisions and the versioned active strategy map.
- **Research tooling** may produce experiments and evaluation reports, but cannot override the authoritative decision engine or bypass risk controls.

## Strategy map in MongoDB

The `edge_strategy` collection stores the active strategy document keyed by `strategy_key: "active"`. It includes a strategy version, named feature-weight maps, feature scales, attention scale, angular threshold, magnitude threshold, spread gate, status and update timestamp. On startup Axum loads and validates this map; if no map exists it seeds the explicit research baseline. A validated update is written to MongoDB before replacing the active in-memory map. Each decision records the strategy version used so historical outcomes can be attributed to the exact configuration.

## Event and decision flow

1. A market-data adapter submits a normalized event to the local gateway.
2. Axum validates and deduplicates the event by venue, symbol and event identity.
3. The event updates current symbol state and is persisted to `market_events`.
4. A causal state snapshot is written to `market_states`; the deterministic vector engine evaluates the active research strategy.
5. The resulting research decision and strategy version are written to `trade_decisions`.
6. Consumers query state, vector decisions and the active strategy through the documented API.

## Version boundaries

Version 1 is the deterministic vector baseline. Version 2 is a separate research track for set-theoretic attention and relational-ledger provenance. Version 2 experiments must remain independently versioned and comparable with Version 1. Neither research track may introduce a hidden decision authority, bypass the risk gate or submit live orders.

## Explicit limitations

This is a research baseline, not an exchange-ready trading system. In-memory state recovery, causal normalization, robust past-only feature scaling, realistic execution simulation, and complete order lifecycle controls require independent verification. Hand-specified weights are not evidence of an exploitable or profitable edge. A signal is not an order; live order submission remains disconnected and disabled.
