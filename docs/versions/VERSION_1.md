# Version 1 — Explicit vector architecture (reference baseline)

Version 1 is the existing Qoin repository architecture on branch `version-1-vector-baseline`. Version 2 retains it as a complete baseline rather than replacing it.

- Frontend clients and shared contracts remain unchanged in responsibility.
- Drogon (C++) remains a gateway only.
- Axum (Rust) remains authoritative for event validation, state estimation, explicit vector decisions, strategy-map validation, and persistence coordination.
- MongoDB remains the durable source of truth for market events, states, decisions, and versioned strategy maps.
- The deterministic explicit vector engine remains available for baseline decisions and evaluation.

The baseline uses measurable market dimensions, explicit weights, direction/magnitude/persistence checks, costs and regime constraints, and WAIT as a valid decision. Existing weights are hand-specified research values and are not proof of a profitable edge.

Known implementation gaps and mandatory safety gates remain those documented in the root README, `docs/ARCHITECTURE.md`, and `docs/RISK_POLICY.md`. Live order submission stays disabled. No real-money trading without all required controls, independent verification, and explicit written approval.
