# Qoin historical vector bridge for Axon

Qoin exposes two private internal market-vector reads:

- `GET /internal/vector?symbol=BTCUSDT` returns the latest market-state vector.
- `GET /internal/vector/history?symbol=BTCUSDT&at_ms=...` returns the latest decision whose source market event timestamp is less than or equal to `at_ms`.

Every newly persisted decision now includes `event_ts_ms`, copied from the market event that caused the decision. A compound `symbol + event_ts_ms` index supports historical lookup. The endpoint never falls back to the current vector when no historical decision exists; it returns 404 instead.

Axon uses the current endpoint for agent handover queries and the historical endpoint when indexing an approved memory, using that memory's original creation timestamp. This prevents a historical memory from being incorrectly tagged with today's market state.

The internal endpoints are intended for private service-to-service traffic only. Keep Qoin's internal listener off the public internet. Existing decision documents written before `event_ts_ms` was added cannot be queried historically until their source event timestamps are backfilled or the relevant market history is replayed. Do not infer or fabricate missing timestamps.

The returned `attended_features` vector is a six-dimensional market-state representation. It is not a natural-language embedding.
