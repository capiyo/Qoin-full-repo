# Service responsibilities

- **Axum** owns event validation, state estimation, the vector baseline, and writes to MongoDB. Model logic must not be duplicated in the gateway.
- **Drogon** owns the local HTTP gateway and forwards requests to Axum. It must not calculate features, alter model decisions, or send exchange orders.
- **MongoDB** stores raw events (`market_events`), state snapshots (`market_states`), and vector decisions (`trade_decisions`).
- **Research tools** provide deterministic Python references and offline experiments. No offline training result may automatically mutate live model weights.
- **Execution** is intentionally not wired into the signal path. Any future execution service must pass a separate risk gate and remain disabled pending human approval.
