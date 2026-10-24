 # Qoin architecture

## Ownership
- **Axum (Rust)** is authoritative for event validation, bounded in-memory state estimation, deterministic vector decisions, and MongoDB writes.
- **Drogon (C++)** is an HTTP gateway. It proxies requests to Axum and contains no independent model or risk logic.
- **MongoDB** stores raw market events, state snapshots, and vector decisions.

## Flow
1. A market-data adapter submits an event to POST /api/v1/events on the local Drogon gateway.
2. Axum validates and deduplicates the event by venue + symbol + event_id.
3. The event updates current in-memory symbol state and is persisted to market_events.
4. A state snapshot is written to market_states; the vector baseline decision is written to trade_decisions.
5. Consumers query /api/v1/state?symbol=BTCUSDT or /api/v1/vector?symbol=BTCUSDT.

## Explicit limitations
This is an early research baseline, not an exchange-ready trading system. Current state reconstruction after process restart is incomplete; the in-memory estimator does not yet rebuild its full state from MongoDB. Feature scaling, thresholds, and coefficients are hand-specified rather than loaded from a versioned configuration. The current decision is not a probability and is not connected to an order lifecycle. No live order route is registered in the Axum router.
