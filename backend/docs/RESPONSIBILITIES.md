# Service responsibilities

- **Axum (Rust)** owns event validation, decayed state estimation, data-freshness and ingestion gates, the independent risk veto, and MongoDB persistence. It calls the shared vector-engine crate for every strategy decision; it must not contain a second model implementation.
- **Drogon (C++)** is the public HTTP gateway and forwards requests to Axum. It must not calculate features, alter model decisions, train weights, or send exchange orders.
- **MongoDB** stores raw events (`market_events`), state snapshots (`market_states`), vector decisions (`trade_decisions`), and the versioned active edge strategy (`edge_strategy`).
- **Vector engine crate** owns deterministic feature attention, strategy-map validation, angular/magnitude gates, transaction-cost gate, regime-confidence gate, and signal hysteresis. The current calibrated-edge and portfolio context are deliberately absent, so the default strategy abstains. Axum loads and validates it on startup. Strategy updates are persisted before becoming active. Each decision records the map version.
- **Research tools** provide deterministic references and offline experiments. Backtest results must not automatically mutate the active strategy; promotion of a new map must be an explicit reviewed update.
- **Execution** is intentionally not wired into the signal path. The independent risk module fails closed without reconciled portfolio context. The `trading.rs` adapter is not compiled or registered; live execution remains disabled pending all policy gates and written human approval.
