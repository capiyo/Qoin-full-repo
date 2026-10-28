# Vector engine and MongoDB addendum

## Collections
- `market_events`: validated source events, deduplicated by `(venue, symbol, event_id)`.
- `market_states`: append-only state snapshots for audit and replay.
- `trade_decisions`: append-only decisions including features, attention, vector magnitude/angles, action, reason, and model version.
- `trade_orders`: reserved for future order lifecycle records; this branch does not register an order endpoint.

## Current implementation caveats
The current in-memory state is not yet fully rebuilt from persisted events after restart. State, feature scaling, weights, thresholds, and model version are not loaded from a versioned runtime configuration. Mongo writes are not a multi-document transaction; a response can indicate that the event persisted while a later snapshot or decision write failed. Consumers must treat this as a partial pipeline outcome and reconcile by event ID.

## Next implementation steps
1. Add replay from raw events in event-time order, with late/out-of-order event policy and bounded causal windows.
2. Add a versioned config contract, config hash, and persisted config version to every decision.
3. Add robust past-only normalization and causal filtering before derivative features.
4. Add a true event-driven backtester that imports the exact production signal code.
5. Add an independent risk gate, order lifecycle, exchange reconciliation, protective stops, circuit breakers, and crash recovery before testnet integration.
