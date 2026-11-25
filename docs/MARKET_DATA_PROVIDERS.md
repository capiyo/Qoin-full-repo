# Market-data providers and adapter contract

Qoin's decision engine consumes normalized observations. It must not depend on an exchange-specific SDK, symbol format, authentication scheme, or transport. Each provider adapter owns its connection lifecycle and translates observations into the shared `MarketEvent` shape before submitting them to the internal Axum ingestion route.

## Implemented adapters on this branch

| Provider | Feed | Data | Configuration | Status |
|---|---|---|---|---|
| Binance USD-M | Combined public WebSocket + REST polling | Trades, best bid/ask, mark/funding updates, liquidation events, open interest | `QOIN_BINANCE_STREAM_ENABLED`, `QOIN_BINANCE_STREAM_SYMBOLS` | Implemented |
| Coinbase Advanced Trade | Public WebSocket `market_trades` | Spot trades | `QOIN_COINBASE_STREAM_ENABLED`, `QOIN_COINBASE_STREAM_PRODUCTS` | Implemented |
| Kraken Spot | Public WebSocket v2 `trade` | Spot trades | `QOIN_KRAKEN_STREAM_ENABLED`, `QOIN_KRAKEN_STREAM_SYMBOLS` | Implemented |
| Finnhub | Authenticated WebSocket + HTTP | Equity/FX/index/commodity trades, news and economic calendar | `FINNHUB_API_KEY`, `QOIN_FINNHUB_STREAM_SYMBOLS` | Implemented; requires valid subscription and symbol entitlements |

Each source has a distinct canonical `venue_id`. Spot instruments use venue-qualified engine symbols (for example, `BTCUSD_COINBASE` and `BTCUSD_KRAKEN`) so their rolling state and vector decisions cannot be accidentally blended. Observations from different venues are not merged into one replay: callers must select a venue when histories contain more than one source. Provider source and event identifiers remain attached to stored observations.

## Shared execution instrument contract

The Rust execution domain now has a broker-neutral contract in `backend/rust-axum/src/broker_contract.rs`. It models asset class, native quantity unit, contract multiplier, tick/lot constraints, quote and settlement currencies, broker/account identity, account-currency conversion, normalized orders, broker capabilities and execution reports. The conversion rate and market quote have separate freshness checks; the broker adapter must supply the account-currency margin requirement. The contract does not imply that a broker is connected or that its instruments are currently entitled.

## Normalization requirements

Every new adapter must:

1. Preserve the provider's event time; reject observations whose timestamps cannot be parsed rather than replacing them with receive time.
2. Normalize symbol, side, price and quantity without losing the original venue identity.
3. Generate stable provider-scoped event IDs so retries are idempotent.
4. Validate finite numeric values, non-negative prices and sizes, and valid book relationships before ingestion.
5. Reconnect with bounded exponential backoff and report connection/subscription errors.
6. Never block another provider's task or the vector engine when the source is unavailable.
7. Respect documented rate limits, exchange terms, data licensing and the account's actual entitlements.

## Important coverage limitations

- The Coinbase and Kraken adapters currently ingest trades only; they do not yet maintain complete level-2 books.
- Binance is the only current source for funding, liquidation and open-interest observations in the engine.
- Finnhub's availability depends on a real key, plan, exchange coverage and symbol entitlement. A registry row is not proof that a quote is available.
- The current instrument registry is curated, not a full instrument master. Symbol aliases, currency conversion and contract metadata still need a versioned reference-data service.
- A bounded, resumable Binance USD-M aggregate-trade backfill is available through the internal operator route. It covers aggregate trades only; Coinbase, Kraken and Finnhub historical backfills are not implemented, and it does not provide historical order books, funding, liquidation or open-interest series.
- Live ingestion stores a server receipt timestamp. An operator-protected per-venue/per-symbol endpoint reports measured ingestion age and source-event lag; historical-only rows are excluded. This is observation telemetry, not an exchange heartbeat or completeness guarantee.
- There is no automatic best-source arbitration or quote failover. Venue histories remain isolated intentionally to prevent invalid mixed-feed backtests.
- Provider health currently reports configured/enabled state, not a complete measured SLO, last-message freshness, sequence-gap rate or entitlement validation.

## Next provider integrations

The adapter interface should expand by market and data entitlement rather than by arbitrary provider count:

- Crypto spot and derivatives: additional exchange spot/derivatives feeds such as OKX, Bybit, Bitstamp and Coinbase Exchange.
- Equities: Polygon, Alpaca and exchange-authorized feeds.
- FX: OANDA and Twelve Data, with clear distinction between indicative and executable quotes.
- Commodities and indices: licensed CME/ICE/Cboe/index-owner data or broker-provided reference feeds.
- Rates and macro: FRED, central-bank, Treasury, IMF and World Bank datasets.
- News: licensed providers plus GDELT for open event discovery.

These are candidates, not active integrations or guaranteed entitlements. Qoin must verify provider contracts, regional availability, redistribution rights, symbol coverage and historical depth before enabling a feed in a paid product.

## Production gates before market-data claims

Do not describe the data layer as institution-grade or complete until it has measured freshness/uptime, gap and sequence monitoring, durable resumable backfill, immutable dataset versions, provider failover tests, clock-skew handling, symbol-master validation, licensed entitlements, and replay coverage reports.


## Immutable research dataset manifests

The Axum research API now exposes an operator-protected dataset snapshot endpoint:

- `POST /internal/datasets` requires the existing `x-qoin-ledger-maintenance-token` header.
- Request fields: `symbol`, `venue`, `from_ts_ms`, `to_ts_ms`, and optional `max_gap_ms`.
- The endpoint scans the canonical immutable event log in deterministic event-time order, validates stored events, computes per-event-type counts and a SHA-256 content identity, and persists an immutable manifest in `market_dataset_manifests`.
- Identical snapshots are idempotent through the unique dataset-version index.
- `GET /internal/datasets` lists manifests and supports optional `symbol`, `venue`, and bounded `limit` filters.

A manifest reports observed event counts, first/last timestamps, invalid stored rows, maximum observed inter-event gap and optional threshold breaches. Gap detection is opt-in because irregular trade events do not have a fixed expected cadence. The status `observed_clean` means only that the scanned stored events passed validation and the requested gap threshold; it does not prove the provider delivered every source event. These manifests version research inputs; they do not download missing history or establish exchange-side completeness.


## Bounded Binance USD-M historical aggregate-trade backfill

The internal Axum operator API provides a resumable, public-market-data import for Binance USD-M aggregate trades:

- POST /internal/market-data/backfill/binance-usdm requires the x-qoin-ledger-maintenance-token header.
- GET /internal/market-data/backfill/{job_id} returns the persisted progress record with the same operator token.
- The request accepts symbol, from_ts_ms, to_ts_ms, optional job_id, page_limit (1–1000, default 1000), and max_pages (1–10 per call, default 5).
- to_ts_ms must be at least five minutes behind server time. This avoids intentionally overlapping the live WebSocket window.
- Jobs are immutable by ID and request parameters. Repeating the same job_id resumes from the persisted aggregate-trade ID. Each page is upserted by provider-scoped event identity before its checkpoint is advanced, so retrying a page after a crash is idempotent.
- Each job is capped at 1,000 pages and 1,000,000 in-range observations. A call is bounded to at most 10 pages and uses request pacing plus bounded retries for rate limits and transient server errors.
- Imported rows are marked historical_only. They are available to dataset manifests, raw history and historical replay, but are excluded from the live V1 rolling-state rebuild. This prevents a late historical import from silently changing the live signal state.
- Run the existing operator-protected V2 set-ledger backfill after importing raw events; this market-data endpoint does not claim that V2 relation records have already been rebuilt.

This is a first historical source, not a complete market-data backfill system. It does not establish Binance source completeness, does not download historical order books/funding/liquidations/open interest, and does not provide Coinbase/Kraken/Finnhub history or automatic provider failover. Dataset coverage remains limited to observed source records.


## Measured live-ingestion freshness

The operator-protected `GET /internal/market-data/providers/health?venue=binance_usdm&symbol=BTCUSDT` endpoint reports the count of stored live observations, the latest event and server receipt timestamps, ingestion age, source-event lag, and a conservative freshness status. `max_age_ms` may be supplied from 100 to 600000 (default 5000). It uses server receipt time, not an exchange heartbeat, and excludes rows tagged `historical_only`.

Older live rows written before receipt timestamps were added are reported as unmeasured rather than being assumed fresh. Statuses such as `fresh` and `stale` describe only the latest server-observed ingestion for the requested venue and symbol. They do not prove provider uptime, exchange-side sequence continuity, source completeness, subscription entitlement or executable broker capability. This endpoint is diagnostic only: it does not arbitrate quotes, switch sources or trigger failover.
