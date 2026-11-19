# Version 1 — Explicit vector architecture (reference baseline)

Version 1 is the existing Qoin repository architecture on branch `version-1-vector-baseline`. Version 2 retains it as a complete baseline rather than replacing it.

- Frontend clients and shared contracts remain unchanged in responsibility.
- Drogon (C++) remains a gateway only.
- Axum (Rust) remains authoritative for event validation, state estimation, explicit vector decisions, strategy-map validation, and persistence coordination.
- MongoDB remains the durable source of truth for market events, states, decisions, and versioned strategy maps.
- The deterministic explicit vector engine remains available for baseline decisions and evaluation.

The baseline consumes the shared `qoin-market-space` schema v1: a fixed 53-coordinate, timestamped market state with explicit missingness and quality. Price, volatility, momentum, liquidity, cross-asset, derivatives, events, regime, portfolio and temporal features share one coordinate contract. Regime descriptors are coordinates in that same state, not a separate label. The `decide_from_market_vector` adapter maps the V1 force dimensions and applies a regime-risk overlay: elevated volatility, liquidity stress, correlation stress, transition intensity and uncertainty raise the evidence threshold; regime volatility also increases volatility dampening. Missing required coordinates fail closed.

The baseline uses measurable market dimensions, explicit weights, direction/magnitude/persistence checks, costs and regime constraints, and WAIT as a valid decision. Existing weights are hand-specified research values and are not proof of a profitable edge.

Known implementation gaps and mandatory safety gates remain those documented in the root README, `docs/ARCHITECTURE.md`, and `docs/RISK_POLICY.md`. Live order submission stays disabled. No real-money trading without all required controls, independent verification, and explicit written approval.
