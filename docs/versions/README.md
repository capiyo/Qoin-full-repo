# Qoin version map

Qoin currently follows one authoritative strategy path: the deterministic Rust vector baseline, extended with independent multi-horizon observations.

## Vector baseline and multi-horizon extension

The engine evaluates separately constructed causal observations at 1m, 5m, 15m, 30m, 1h, 4h, 1d and 1w. Each horizon retains its own decision and configuration. The agreement/divergence summary is descriptive metadata only; it does not average signals, allocate capital, or create orders.

See [multi-horizon research specification](../MULTI_HORIZON_RESEARCH.md), [architecture](../ARCHITECTURE.md), and [risk policy](../RISK_POLICY.md).

## Safety

All strategies remain research/paper-only. Live trading requires every risk gate to be implemented and independently verified plus explicit written approval. No profitability or predictive edge is claimed.
