# State of repository

The repository uses React web and React Native clients, a Rust/Axum authoritative event and vector engine, a C++/Drogon HTTP gateway, and MongoDB persistence. The vector engine now has a multi-horizon research evaluator and a market-event aggregation path.

## Multi-horizon implementation
- Eight horizons are supported: 1m, 5m, 15m, 30m, 1h, 4h, 1d and 1w.
- Market events accept `forex`, `crypto` and `stock` asset classes; the market-horizon query can isolate a class to prevent cross-class symbol collisions.
- Each horizon is derived from its own trailing event window using events no later than the decision timestamp.
- The market-backed endpoint reads up to 50,000 events from the preceding week and requires both trade and order-book evidence plus a configurable minimum event count before emitting a signal.
- Missing funding, open-interest and basis values are explicitly identified in diagnostics and treated as neutral by the current baseline; dedicated basis events are supported.
- Web and mobile research screens can request horizon observations for a selected instrument.

## Verified status

GitHub Actions passed on code commit `34bda6f`:
- Rust workspace `cargo test --workspace`.
- Axum + Drogon + MongoDB integration, including service restart, strategy persistence, event ingestion, instrument discovery, market-horizon readiness and asset-class state retrieval.
- Drogon gateway build.
- React web production build.
- React Native strict TypeScript check.
- Python reference tests and market-event JSON schema validation.

A subsequent documentation-only commit updated the asset-class feed requirements. CI reruns for the latest head are also being monitored; no live deployment or profitability claim is made.

## Limitations
- Market-data adapters for complete forex, crypto and stock universes are not yet wired to this endpoint. The API aggregates events already ingested; it does not fetch exchange/vendor data itself.
- Current normalization is a research baseline, not past-only rolling calibration across venues or asset classes.
- No event-driven backtester, walk-forward performance report, realistic fee/slippage model, shadow deployment, or profitability evidence exists yet.
- No live-order route is registered. Authentication/authorization, production risk engine, protective stops, position/order reconciliation, kill switch, and crash recovery remain incomplete.
- Market data must be venue-normalized and symbol-mapped before cross-asset comparisons are meaningful.

Keep this system research/paper-only. Do not claim production readiness or profitability.
