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
- There is no resumable historical backfill service yet. WebSocket streams provide live observations, not complete historical datasets.
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
