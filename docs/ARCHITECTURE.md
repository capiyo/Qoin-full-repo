# Qoin architecture

## Service ownership

- **Rust/Axum** is the only authority for market-event validation, venue-scoped state, feature construction, vector decisions, strategy state, risk validation and private execution orchestration.
- **C++/Drogon** is the public HTTP/WebSocket gateway. It proxies requests and streams to Axum; it does not calculate features, decisions, risk or order sizing.
- **MongoDB** persists raw events, state snapshots, decisions, strategies, paper-session records, order intents and application data.
- **React web and React Native** share `@qoin/core`; neither client is authoritative for user identity, entitlements, portfolio state or decisions.
- **Python** provides research, reference models, analysis and reproducible evaluation. It does not submit exchange orders.
- **Tauri 2** packages the desktop client. The bounded vector-motion solver lives in the Rust workspace.

## Market-data and decision flow

1. Provider adapters normalize observations with provider, venue, canonical symbol, source timestamp and event identity.
2. Axum validates and deduplicates observations, updates the state keyed by venue and symbol, and persists the raw event.
3. Axum constructs the configured deterministic feature/vector state and records the decision with its strategy version and diagnostic trace.
4. WebSocket subscribers receive public market observations and decision updates through Drogon.
5. Research replay reads persisted raw events in event-time order and applies explicit execution assumptions. Replay is not historical data acquisition and cannot claim full coverage.

## Persistence and recovery

The immutable market-event log is the recovery source for the current in-memory market state. Startup replay rebuilds state in deterministic event-time order. Snapshot/decision persistence is separate from a cross-collection atomic transaction; any paper ledger or order lifecycle must therefore use durable idempotency and reconciliation rather than assuming multi-document writes are atomic.

## Provider capabilities

- Binance USD-M: trades, best bid/ask, mark/funding, liquidation events and open-interest polling.
- Coinbase Advanced Trade: public spot trades only.
- Kraken Spot WebSocket v2: public spot trades only.
- Finnhub: optional authenticated trade feed, news and calendar; actual availability depends on key, plan and symbol entitlement.

A provider's lack of book/funding/liquidation/OI support must be represented as unavailable, not synthesized. Venue histories remain isolated for replay unless a separately validated normalization/aggregation policy explicitly allows combination.

## Security boundaries

The Drogon gateway is the only public service in Compose. Axum and MongoDB remain private. Client Firebase authentication does not by itself authenticate a request to Axum; every personalized server route must verify Firebase ID tokens and derive UID from verified claims. Paid entitlements must be loaded and enforced server-side.

## Trading boundary

The private Binance adapter is operator-invoked and is not connected to autonomous strategy signals. Real-money mode is blocked by code policy pending the mandatory gates in `RISK_POLICY.md`. The current order route's durable intent and operator reconciliation are not a complete exchange lifecycle or portfolio risk system.
