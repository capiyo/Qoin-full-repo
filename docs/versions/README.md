# Qoin version map

Qoin follows one active deterministic Rust vector-engine architecture. The engine supports the configured market data and instrument capabilities documented in [MARKET_DATA_PROVIDERS.md](../MARKET_DATA_PROVIDERS.md).

## Active research platform

The current platform includes the deterministic vector baseline, multi-horizon research evaluation, market event replay and the shared web/mobile/desktop clients. These capabilities are research and paper simulation only; they are not independently validated trading strategies.

See [architecture](../ARCHITECTURE.md), [multi-horizon research](../MULTI_HORIZON_RESEARCH.md), [market-data providers](../MARKET_DATA_PROVIDERS.md), and [risk policy](../RISK_POLICY.md).

## Evaluation policy

Compare algorithms using identical timestamped data, causal feature availability, chronological train/test boundaries, benchmark baselines, explicit execution costs and out-of-sample periods. A lower loss or stronger in-sample metric is not proof of a trading edge.

## Safety

Paper and exchange testnet are the only permitted execution modes before every mandatory release gate is implemented and independently verified. Live execution remains disabled by code policy.
