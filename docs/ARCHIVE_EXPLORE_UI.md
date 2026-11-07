# Qoin Archive, Explore, live market and vector-space contract

## Client workspace

The web and React Native clients share the Archive / Explore instrument workspace. Explore loads the backend instrument registry, supports Crypto, Equities, FX, Indices and Commodities filters, and displays provider-attributed news and economic-calendar items. Cards subscribe to the same read-only market stream and show last observed price and current research action when those observations exist.

Archive toggles are currently client-local state. They are not authenticated, persisted to MongoDB, or synchronized between web and mobile. The starter archive defaults to BTC/USD, ETH/USD and SOL/USD. Do not describe this as a cross-device user archive until the user identity and persistence endpoints are implemented.

## Instrument registry and canonical symbols

`GET /api/v1/instruments` returns the current curated 26-instrument registry with display symbol, exact engine/provider symbol, asset class, venue, provider, description, quote currency and provider configuration status.

Crypto symbols use Binance USD-M perpetual identifiers (for example, BTC/USD maps to BTCUSDT). Equities use Finnhub symbols such as AAPL. FX, index and commodity references use Finnhub/OANDA symbol identifiers such as OANDA:EUR_USD and OANDA:XAU_USD. The UI must call engine endpoints using `engine_symbol`, not the display label.

A registry entry is not proof that its provider currently emits data. A live quote appears only after an actual provider event has been ingested. Unsupported, unsubscribed, closed-session, stale or unavailable instruments remain visibly empty rather than being populated with synthetic prices.

## Real-time WebSocket protocol

The public client endpoint is `GET /api/v1/stream?symbols=BTCUSDT,ETHUSDT` (WebSocket upgrade). The Drogon gateway proxies this connection to the private Axum `/internal/stream` endpoint; do not expose Axum directly to the public internet.

Message types:
- `stream.ready`: protocol version, initial subscription list and server timestamp
- `market.update`: raw accepted event, current symbol state, persisted decision and server timestamp
- `stream.subscribed`: acknowledgement for a client subscription update
- `stream.gap`: broadcast backpressure caused dropped messages; clients must refresh REST snapshots before trusting the current display
- `stream.error`: gateway/upstream stream failure

Clients reconnect with bounded exponential backoff. The server broadcasts only after the event, state snapshot and decision have been persisted. The stream is read-only: it never submits orders.

### Providers currently wired

**Binance USD-M public WebSocket** (no API key) subscribes to trade, bookTicker, markPrice and forceOrder streams for the configured symbols. The default set is BTCUSDT, ETHUSDT, SOLUSDT, BNBUSDT, XRPUSDT and DOGEUSDT. A separate 30-second public REST poll ingests open interest for the same symbols. Trade aggressor side is inferred only from Binance's buyer-maker flag. Liquidation side is derived from the liquidation order side. The adapter posts observations to the local Axum ingestion endpoint and reconnects with bounded backoff.

**Finnhub WebSocket** is optional and requires `FINNHUB_API_KEY`. It subscribes to the configured equities, FX, index and commodity symbols and ingests actual provider trade observations. Finnhub trade messages do not provide a reliable aggressor side in this adapter, so side is left unknown; the engine must not fabricate buy/sell pressure from that field. Provider entitlements and symbol coverage vary. Backend logs expose rejected subscriptions and connection failures.

Environment:
- `QOIN_BINANCE_STREAM_ENABLED`
- `QOIN_BINANCE_STREAM_SYMBOLS`
- `FINNHUB_API_KEY`
- `QOIN_FINNHUB_STREAM_SYMBOLS`
- `QOIN_AXUM_ADDR`, `QOIN_AXUM_URL`, `QOIN_AXUM_WS_URL`

The mobile client must be built with a reachable `EXPO_PUBLIC_QOIN_API_URL` and `EXPO_PUBLIC_QOIN_WS_URL` for the gateway. For a physical Android device, loopback refers to the device itself; do not assume `127.0.0.1` reaches a development PC. The web client can use its current origin or `VITE_QOIN_API_URL` / `VITE_QOIN_WS_URL`.

## Explore news and macro events

- `GET /api/v1/news?category=general|forex|crypto|merger` retrieves live, attributed Finnhub headlines, preserving source, headline, summary, URL and publication timestamp.
- `GET /api/v1/calendar` retrieves the next seven days of Finnhub economic-calendar releases with country, event, impact, actual, estimate, previous and provider timestamp.

Both require `FINNHUB_API_KEY`. When it is missing or the provider fails, the UI shows the provider status and no fake headlines/events are generated. News refreshes every five minutes; the economic calendar refreshes every fifteen minutes.

## Vector and event transparency

Selecting an instrument opens the vector detail surface:
- `GET /api/v1/vector?symbol=<engine_symbol>`: latest persisted Axum decision
- `GET /api/v1/vector/history?symbol=<engine_symbol>&limit=120`: chronological persisted decision-vector trajectory
- `GET /api/v1/events?symbol=<engine_symbol>&limit=200`: raw persisted market-event tape

The display includes the two-component bullish/bearish decision vector and its historical path; a six-axis view of the six normalized engine input features; raw and attended feature values; the complete 6×6 attention matrix; magnitude, angle, strategy version, action and rationale; live state including price, spread, order-flow imbalance, funding, open interest, liquidation count, event count, volatility estimate and state quality; and the most recent raw event plus an expandable raw event tape.

The six-axis chart is a direct visualization of the six normalized input features. Open interest is visible in market state and raw events but is not currently one of the six decision features. The chart is not a trained embedding or PCA projection. The bullish/bearish plot is the engine's actual two-dimensional decision vector, not a learned latent embedding. Attention weights are deterministic baseline model internals, not causal attribution.

## Explicit integration gaps and safety boundaries

- Archive add/remove state is still client-local. Authentication and MongoDB-backed per-user archive synchronization are not implemented.
- The 26-item registry is a curated discovery catalogue, not a dynamically queried exchange-wide instrument master. Provider support/entitlements and symbol availability must be verified per instrument.
- Binance public stream is wired for the default crypto perpetual symbols. Finnhub stream/news/calendar require a real configured API key and supported entitlements. No provider key is committed.
- The engine still holds live symbol state in memory after process start; full MongoDB replay/recovery remains incomplete. Persisted raw events and decisions exist, but this does not yet guarantee crash-safe reconstruction.
- The current feature baseline uses hand-specified scales and weights. Its attention is deterministic and not a trained transformer; no predictive edge or profitability has been demonstrated.
- There is no live order-submission route. The WebSocket transports observations and research vectors only. This work does not enable live execution, exchange credentials, position sizing or order management.

Keep Qoin research/paper-only. The existing `docs/RISK_POLICY.md` remains binding.
