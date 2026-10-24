# Service responsibilities

- **Axum (Rust)** owns event validation, state estimation, causal feature work, the vector/attention engine, strategy-map validation, signal decisions, and all MongoDB persistence.
- **Drogon (C++)** is the public HTTP gateway and forwards requests to Axum. It must not calculate features, alter model decisions, train weights, or send exchange orders.
- **MongoDB** stores raw events (`market_events`), state snapshots (`market_states`), vector decisions (`trade_decisions`), and the versioned active edge strategy (`edge_strategy`).
- **Edge strategy map** is the versioned configuration object with named bullish/bearish weights, feature scales, attention scale, angular/magnitude thresholds, and spread gate. Axum loads and validates it on startup. Strategy updates are persisted before becoming active. Each decision records the map version.
- **Research tools** provide deterministic references and offline experiments. Backtest results must not automatically mutate the active strategy; promotion of a new map must be an explicit reviewed update.
- **Execution** is intentionally not wired into the signal path. Any future execution service must pass a separate risk gate and remain disabled pending human approval.
