# Service responsibilities

- **Axum (Rust)** owns event validation, causal state estimation, sequence/freshness checks, risk-gate invocation, versioned strategy persistence, and MongoDB persistence. It delegates every model decision to `crates/qoin-vector-engine`.
- **Drogon (C++)** is the public HTTP gateway and forwards requests to Axum. It must not calculate features, alter model decisions, train weights, or send exchange orders.
- **MongoDB** stores raw events (`market_events`), state snapshots (`market_states`), vector decisions (`trade_decisions`), and the versioned active edge strategy (`edge_strategy`).
- **`crates/qoin-vector-engine` (Rust)** is the sole owner of pure deterministic model decisions, force normalization, attention, angular/magnitude gates, persistence/cost/regime/freshness gates, and candidate reasons. Axum must not contain a parallel scoring implementation.
- **Edge strategy map** is versioned configuration for order-flow imbalance, book imbalance, funding pressure, liquidity stress, volatility shock, event intensity, liquidation pressure, open-interest change, and basis. Legacy maps are migrated and persisted before use. Each decision records the map version.
- **Research tools** provide deterministic references and offline experiments. Backtest results must not automatically mutate the active strategy; promotion of a new map must be an explicit reviewed update.
- **Risk** is independent of the model. Missing risk limits, account/exposure state, feed continuity, or protective-stop confirmation must veto entries.
- **Execution** is intentionally not wired into the signal path. The `live-execution` feature only compiles the adapter; it does not connect it to signals. Real-money execution remains disabled pending every risk-policy gate, independent review, and explicit written owner approval.
