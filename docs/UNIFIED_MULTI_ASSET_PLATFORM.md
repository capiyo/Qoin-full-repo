# Unified multi-asset Qoin platform

## Product direction

Qoin is one deterministic vector engine across three selectable asset classes: foreign exchange, crypto assets, and listed equities. JEPA and all JEPA services, code, telemetry, tests, dependencies, workflows, and version-2 references are retired. There is no second predictive model.

## Instrument universe

The frontend selects an asset class, venue/exchange, instrument, and timeframe. The instrument catalogue is provider-backed and searchable; it must not hardcode a finite list or claim literally every instrument is supported. Catalogue records include canonical instrument ID, asset class, venue, symbol, base/quote or issuer metadata, trading session/calendar, quote currency, tick/lot metadata, supported timeframes, provider, and current data availability.

- Forex: discover currency pairs from connected market-data providers, including major, minor and supported exotic pairs. Normalize pip size, quote convention, sessions, spreads and rollover/swap context.
- Crypto: discover provider-listed spot and derivative instruments. Distinguish spot, perpetual and dated futures; normalize contract multiplier, quote currency, funding, open interest, liquidation and venue-specific market hours.
- Equities: discover listed shares by exchange and provider coverage. Preserve exchange MIC, currency, trading calendar, corporate-action adjustments, split/dividend treatment, tick size and session state.

The UI must distinguish catalogue availability from live data entitlement and from trading permission. A symbol is not tradeable merely because it appears in search.

## One vector mechanism

Keep one deterministic, auditable engine. Every feature is a pure function over causally aligned inputs. Each feature observation exposes: value, confidence, availability, provenance timestamp, normalized scale, directional contribution, non-directional risk contribution, and any signed context. Missing or stale data remains unavailable; it is never silently replaced with zero.

The engine must use instrument/timeframe-specific normalization and explicitly configured weights/interactions. It must avoid assuming crypto-only inputs such as funding/open interest exist for forex or ordinary equities. Feature availability and coverage are part of every decision. Low coverage, contradictory evidence, excessive spread/volatility/liquidity stress, stale prices, or closed sessions produce WAIT rather than fabricated conviction.

A decision response includes the complete trace:
- raw observation and source timestamp for each feature;
- normalized feature value and confidence;
- configured base weight and effective weight;
- each pairwise interaction and its signed contribution;
- aggregate directional vector components and magnitude;
- coverage, agreement, contradiction, and separate risk score;
- thresholds, hysteresis/persistence state, and the exact BUY/SELL/WAIT trigger explanation;
- strategy version and instrument-normalization profile.

The browser renders this returned trace as an interactive vector map for each selected instrument. It may calculate display geometry from the trace but must not independently recalculate the authoritative signal. Users can inspect each feature, see weights accumulate, expand interaction terms, and scrub historical decision snapshots.

## Identity, profile and entitlements

Authentication is required for saved user profiles and private workspace data. The backend validates signed sessions/tokens, derives the user ID from the verified identity, and scopes every profile, watchlist, selected instrument, strategy preference, saved trace, and workspace query to that identity. Never accept a client-supplied user ID as authority.

Entitlements are server-owned records associated with the authenticated user and subscription/account plan. The backend resolves permissions for asset classes, exchanges/venues, data tiers, timeframes, history depth, concurrent instruments, algorithm/strategy variants, research tools, and paper-trading features. The frontend receives a read-only entitlement projection for display and navigation. Every protected API checks the entitlement again on the server; hiding a button is not access control. Deny by default, audit changes, and do not infer paid permissions from a frontend flag.

Profile data is separated from authentication credentials. Passwords, provider tokens, API keys and session secrets must never be returned to the browser or logged. Use secure, HttpOnly, SameSite cookies for browser sessions where applicable, short-lived access credentials for native clients, rotation/revocation, CSRF protection for cookie-authenticated mutations, rate limits, and explicit account deletion/export paths.

## Frontend experience

Web and React Native share typed API contracts and the same information architecture:
1. Sign in / register / recover account.
2. User workspace and profile.
3. Asset-class selector: Forex / Crypto / Stocks.
4. Provider/exchange selector and searchable instrument catalogue.
5. Watchlist and instrument detail.
6. Live vector visualization with feature weights, interactions, risk and BUY/SELL/WAIT trigger.
7. Per-instrument history and replay.
8. Entitlements/settings screen showing included and unavailable capabilities with clear explanations.

The selected instrument is a user preference, not a global mutable engine setting. Multiple instruments must remain isolated by canonical instrument ID, venue, timeframe and provider. Web and mobile must not drift in signal math or entitlement behavior.

## Service boundaries

- React web and React Native: presentation, instrument discovery, account flows and read-only trace visualization.
- C++ Drogon: public-facing gateway, request validation, rate limiting and forwarding; no strategy math.
- Rust Axum: authenticated identity context, entitlement authorization, catalogue orchestration, event ingestion, feature assembly, vector decisions and risk gating.
- MongoDB: user profiles, entitlement grants, instrument metadata/cache, preferences, event/state/decision history, audit records and versioned strategy configuration.
- Market-data adapters: provider-specific discovery, streaming, normalization, provenance and licensing boundaries. No source may be represented as live or complete until connected and validated.

## Rollout and acceptance gates

1. Remove JEPA implementation and references; build and test only the deterministic vector path.
2. Add canonical instrument model, provider adapter interface and searchable catalogue.
3. Add authentication, profile isolation, server-side entitlements and authorization tests.
4. Wire forex, crypto and equity data adapters independently; prove symbol mapping, timestamp quality, sessions and missing-data behavior.
5. Return full decision traces and implement matching web/mobile vector visualizations.
6. Validate cross-platform contract parity, multi-instrument isolation, reconnect/replay, stale-feed behavior, entitlement denial, and CI.
7. Run chronological replay and paper/shadow evaluation with realistic spread, fee, slippage, funding/rollover, corporate actions and market-session costs.

Live order placement remains disconnected. A BUY or SELL is an analytical signal, not an instruction to submit an order. No live trading or automatic strategy promotion without independent risk validation and explicit written approval.
