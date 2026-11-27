# Qoin Archive, Explore, live market and vector-space contract

## Client workspace

The web and React Native clients share the Archive / Explore instrument workspace. Explore loads the backend instrument registry, supports Crypto, Equities, FX, Indices and Commodities filters, and displays provider-attributed news and economic-calendar items. Cards subscribe to the same read-only market stream and show last observed price and current research action when those observations exist.

Archive changes are available after Firebase sign-in. The authenticated user's `archivedSymbols` list is read from and written to Firestore at `users/{uid}` through shared `@qoin/core` functions used by both web and React Native. Firestore rules restrict access to the matching UID and cap the list at 500 entries, so signed-in clients share the same Archive. This is a Firestore user preference, not a MongoDB-backed subscription service. The signed-out starter preview defaults to BTC/USD, ETH/USD and SOL/USD.

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

Clients reconnect with bounded exponential backoff. The server broadcasts only after the event, state snapshot and decision have been persisted. The stream is read-only: it never submits orders. The current public market stream is unauthenticated because it carries public market observations only; before exposing it broadly, add edge-level connection limits, rate limits and origin controls. Do not put account or portfolio-private data on this channel.

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

- Archive add/remove is gated by Firebase Authentication and persisted in Firestore at `users/{uid}.archivedSymbols`; web and React Native use the same per-user document. This is not a MongoDB-backed subscription service.
- The 26-item registry is a curated discovery catalogue, not a dynamically queried exchange-wide instrument master. Provider support/entitlements and symbol availability must be verified per instrument.
- Binance public stream is wired for the default crypto perpetual symbols. Finnhub stream/news/calendar require a real configured API key and supported entitlements. No provider key is committed.
- The engine still holds live symbol state in memory after process start; full MongoDB replay/recovery remains incomplete. Persisted raw events and decisions exist, but this does not yet guarantee crash-safe reconstruction.
- The current feature baseline uses hand-specified scales and weights. Its attention is deterministic and not a trained transformer; no predictive edge or profitability has been demonstrated.
- There is no live order-submission route. The WebSocket transports observations and research vectors only. This work does not enable live execution, exchange credentials, position sizing or order management.

Keep Qoin research/paper-only. The existing `docs/RISK_POLICY.md` remains binding.


## Live notification centre

The web and React Native clients now expose a notification centre fed directly by the existing read-only market WebSocket. It creates deduplicated alerts when a symbol's engine action transitions into BUY or SELL and when a liquidation event is received. Each alert shows the symbol, timestamp, event category, engine rationale or liquidation side/price, and the underlying JSON payload for inspection. Users can mark the notification list read or clear it for the current app session.

On web, users may opt into browser desktop notifications from the notification centre. Those browser notifications are emitted only while the page is running and has an active WebSocket connection. The mobile app currently shows in-app notifications while the app is running; it does not yet register FCM/APNs/Expo push tokens or deliver background notifications.

Notification history is currently in-memory and resets when the app process/page is restarted. It is not stored per user in MongoDB, synchronized across devices, or backed b## Light/blue workspace visual system (2026-11-27)

- Shared Qoin brand tokens in `frontend/packages/core/src/theme.ts`: background `#F7F9FC`, surface `#FFFFFF`, border `#E6EBF2`, blue accent `#2867DC`, primary text `#24344D`, and muted text `#8794A7`.
- The web workspace uses light page/card surfaces and blue Archive/Explore active states.
- React Native uses the shared theme tokens; its floating bottom navigation has exactly two destinations, Archive and Explore. The bar uses the white surface and shared border token, with a light-blue active surface `#EAF1FF` and blue active text/indicator.
- Instrument mechanics remain an inspection surface. V1 is the implemented deterministic vector baseline; Set & Ledger V2 is a read-only finite-set projection, not a complete independent decision engine.
- Qoin remains research/paper-only. The broker-neutral order contract and owner-scoped identity checks exist, but `BrokerRegistry` starts empty, no provider adapter is registered, and the readiness endpoint keeps live dispatch disabled. See `docs/BROKER_NEUTRAL_EXECUTION.md`.

preview engine until independent execution, persisted state, entitlement checks and validation exist. UI styling does not imply that Version 2 is running.
- No broker selection, order submission or live execution was added. Research/paper-only safety boundary is unchanged.


## Instrument mechanics workspaces and Archive card modes (2026-11-18)

- Each Archive card keeps a per-instrument view mode: Vector (default) or Backtest. Switching the card mode does not open the workspace; tapping the card opens the mechanics page for the selected mode.
- Web routes are addressable and refresh-safe: `/app/instrument/{SYMBOL-SLUG}/vector` and `/app/instrument/{SYMBOL-SLUG}/backtest` (for example, `/app/instrument/BTC-USD/backtest`). Browser back/forward restores the selected instrument and mode. The mode preference is stored locally in browser storage.
- The web build includes a Netlify SPA rewrite for `/app/*` so direct links and refreshes reach the Vite entry point. Other hosting providers must provide the equivalent history fallback.
- React Native presents the selected instrument mechanics as a full-screen, safe-area-aware workspace. Its per-instrument mode preference is persisted with AsyncStorage; Android hardware back closes the workspace.
- The Backtest workspace explains the replay pipeline before the controls/results: persisted event tape, deterministic ordering, V1 state reconstruction, strategy evaluation, strictly-later-event execution eligibility, bid/ask execution, latency/liquidity constraints, fees/slippage/impact/funding, and audit outputs.
- Vector and Backtest are two views over the same instrument and existing V1 research strategy. Backtest remains a historical paper simulation only; no broker orders are submitted. V2 Set & Ledger remains a separate read-only research projection and cannot influence V1 decisions.
