# Qoin architecture

## Ownership
- **Axum (Rust)** is the sole authority for event validation, bounded state estimation, feature construction, vector/attention decisions, strategy-map validation, and MongoDB writes.
- **Drogon (C++)** is the HTTP gateway. It forwards event, state, vector, and edge-strategy requests to Axum; it contains no independent model, risk, or order logic.
- **MongoDB** is the durable source of truth for raw market events, state snapshots, vector decisions, and the versioned active edge-strategy map.

## Edge strategy map in MongoDB
The `edge_strategy` collection stores one active document keyed by `strategy_key: "active"`. It includes a strategy version, named bullish/bearish feature-weight maps, feature scales, attention scale, angular threshold, magnitude threshold, spread gate, status, and update timestamp. On startup Axum loads and validates this map; if no map exists it seeds the explicit research baseline. Every accepted market event is evaluated using the in-memory copy of that persisted map. A validated update through `PUT /api/v1/strategy` is written to MongoDB before it replaces the active in-memory map. `GET /api/v1/strategy` returns the active map. The strategy version is embedded in each decision so historical outcomes can be attributed to the exact map version.

## Flow
1. A market-data adapter submits an event to `POST /api/v1/events` on the local Drogon gateway.
2. Axum validates and deduplicates the event by venue + symbol + event_id.
3. The event updates current symbol state and is persisted to `market_events`.
4. A state snapshot is written to `market_states`; the active MongoDB-backed edge map computes a vector decision, which is written to `trade_decisions`.
5. Consumers query `/api/v1/state?symbol=BTCUSDT`, `/api/v1/vector?symbol=BTCUSDT`, and `/api/v1/strategy`.

## Explicit limitations
This is an early research baseline, not an exchange-ready trading system. State is currently held in memory after process start and is not fully replayed from MongoDB after restart. Feature normalization is simple clipping/scaling, not yet robust past-only rolling normalization. The strategy map is configurable and versioned, but its weights are hand-specified and are not evidence of an exploitable or profitable edge. A signal is not an order; live order submission remains disconnected and disabled.


## Optional Version 2 JEPA research sidecar

The JEPA-inspired Python service is optional and remains non-authoritative. When `QOIN_JEPA_URL=http://127.0.0.1:8765` is configured, Axum maintains a bounded per-symbol history of the fixed 11-feature contract, rate-limits requests with `QOIN_JEPA_MIN_INTERVAL_MS`, and asynchronously stores latent-only predictions in `jepa_predictions`. `GET /internal/jepa?symbol=BTCUSDT` exposes the latest stored observation. `QOIN_JEPA_CONTEXT_STEPS` must match the checkpoint's configured context length.

The sidecar URL is restricted to loopback HTTP. Missing service, schema mismatch, timeout, invalid output, or persistence failure must never block event ingestion or change the Version 1 vector decision. The latent vector is not a BUY/SELL signal, not an uncertainty estimate, and does not demonstrate informed/insider activity. Live order submission remains disconnected and disabled.
