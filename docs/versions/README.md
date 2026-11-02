# Qoin architecture map

Qoin uses one unified deterministic vector-engine architecture across supported forex, crypto and equity instruments. The JEPA model and its service track have been retired from the active codebase.

## Unified vector platform

The multi-market feature library supplies independently calculated, timestamp-causal observations. All asset classes use the same vector decision contract while each instrument and venue supplies valid data, units, trading sessions, liquidity references and feature availability.

See [Unified multi-asset platform](../UNIFIED_MULTI_ASSET_PLATFORM.md) and [functional multi-market confluence](../FUNCTIONAL_MULTIMARKET_CONFLUENCE.md).

## Safety

Signals remain research/paper-only. No live order execution or automatic strategy promotion is enabled. User entitlements are enforced by the backend, never trusted from frontend claims.
