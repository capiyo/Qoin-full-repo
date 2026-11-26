# Qoin historical vector bridge for Axon

Qoin exposes two private internal market-vector reads:

- `GET /internal/vector?symbol=BTCUSDT` returns the latest market-state vector.
- `GET /internal/vector/history?symbol=BTCUSDT&at_ms=...` returns the latest decision whose source market event timestamp is less than or equal to `at_ms`.
- `GET /internal/vector/history?symbol=BTCUSDT&limit=120` retains Qoin's existing bounded decision-history response for clients that omit `at_ms`.

The point-in-time lookup filters and sorts on `input_event.event_ts_ms`, which is copied from the source market event embedded in every persisted decision. A compound `symbol + input_event.event_ts_ms` index supports the query. It never falls back to the current vector when no historical decision exists; it returns 404 instead. The optional query parameter preserves the existing Qoin history API.

Axon uses the current endpoint for agent handover queries and the historical endpoint when indexing an approved memory, using that memory's original creation timestamp. This prevents a historical memory from being incorrectly tagged with today's market state.

The internal endpoints are intended for private service-to-service traffic only. Keep Qoin's internal listener off the public internet. Historical lookup can only return decisions that have been persisted; it does not fabricate or infer missing market observations.

The returned `attended_features` vector is a six-dimensional market-state representation. It is not a natural-language embedding.
