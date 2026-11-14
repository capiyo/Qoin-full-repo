# Qoin algorithm catalogue and access model

## Product direction

Qoin is a platform for multiple independently developed, deterministic and inspectable investment-research algorithms. A user chooses an algorithm they are entitled to use; the selected algorithm evaluates the user's instruments and produces its own versioned decision trace.

The algorithms are products, not cosmetic presets of one hidden strategy. They may use different feature sets, horizons, mathematical models, normalization profiles, thresholds and risk characteristics. They share the Qoin platform services, but one algorithm must not silently inherit another algorithm's state or performance history.

Users gain access according to the initial fee / access level they purchase. The exact prices, currency, billing provider and whether later renewals exist are product configuration, not hard-coded algorithm logic. A higher fee buys access to a different algorithm or capability set; it does not imply or guarantee higher returns.

## Core domain model

Keep these concepts separate:

- **Algorithm**: stable identity and public product description, e.g. `vector-core`, `momentum-flow`, or `options-context`.
- **Algorithm version**: immutable executable/configuration identity, e.g. `vector-core@1.2.0`. A running user selection pins an exact version.
- **Access tier**: a purchasable catalogue grouping that grants access to one or more algorithm IDs and explicitly enumerated capabilities.
- **Purchase / entitlement grant**: server-owned evidence that an authenticated user acquired a tier or direct algorithm licence. It records provider reference, amount/currency, status, effective time, expiry if applicable, and audit metadata. Never trust a client-supplied paid flag.
- **User algorithm selection**: the authenticated user's active algorithm ID and pinned version, with activation time and selection history.
- **Algorithm run / decision trace**: output from one algorithm version for one canonical instrument and timeframe, linked to the exact input event window and configuration fingerprint.

An algorithm is not a tier, a tier is not a payment, and a payment event is not itself permission until it has been verified and converted into a grant.

## Algorithm manifest / contract

Every algorithm registered with Qoin must declare a validated manifest:

- stable ID, display name, description, semantic version and lifecycle status (`preview`, `active`, `deprecated`, `disabled`);
- supported asset classes, venues/providers, canonical instruments or instrument capabilities, and supported timeframes;
- required and optional input features, data freshness limits, minimum history and readiness conditions;
- normalization profile, parameters, parameter schema and configuration fingerprint;
- directional outputs and non-directional risk outputs, including explicit `WAIT` / not-ready behavior;
- risk limits and fail-closed conditions;
- provenance and explanation schema for every decision;
- compatibility range for engine/runtime and trace schema;
- evaluation metadata and validation status, including the datasets / periods used.

Registration must reject duplicate IDs/versions, invalid parameter schemas, unknown features, unsupported timeframes and incompatible runtime/trace versions. A manifest describes capability; it does not prove a provider feed is currently available.

## Runtime boundaries

Shared platform services remain common:

1. Authentication and verified user identity.
2. Market-data ingestion, provider licensing, normalization and event provenance.
3. Canonical instrument catalogue and user workspace.
4. Entitlement resolution and authorization.
5. Common risk policy, audit trail and immutable run/decision history.
6. WebSocket delivery and web/mobile API contracts.

The algorithm runtime is isolated behind a typed interface. Conceptually:

```text
AlgorithmInput {
  instrument_id,
  venue,
  timeframe,
  event_window,
  feature_observations,
  account_context,
  runtime_timestamp
}

AlgorithmOutput {
  algorithm_id,
  algorithm_version,
  config_fingerprint,
  readiness,
  decision,          // BUY | SELL | WAIT
  decision_vector,
  risk_diagnostics,
  feature_contributions,
  interactions,
  rationale,
  input_provenance
}
```

The real Rust types must use the repository's existing canonical event, feature and signal types rather than introducing duplicate market models. Algorithm code must be deterministic for the same ordered inputs and pinned configuration. It must not own payment, user identity, database authorization or broker execution.

Each algorithm receives an isolated state namespace keyed at minimum by algorithm ID, algorithm version, canonical instrument ID, venue and timeframe. No mutable state, coefficients, rolling windows or historical performance may leak between algorithms. Shared raw market observations may be reused only as immutable, provenance-stamped inputs.

## Fee and entitlement resolution

The backend is authoritative. For every algorithm catalogue request, selection mutation, run request, saved trace and protected history request:

1. Resolve the user from the verified authentication context; never accept a caller-provided user ID as authority.
2. Load verified, active entitlement grants for that user.
3. Resolve the selected tier / direct licence into an explicit allow-list of algorithm IDs and capabilities.
4. Check the requested algorithm ID, version, asset class, timeframe, data tier, history depth and concurrency limits.
5. Deny by default when the grant is absent, pending, refunded, revoked, expired or ambiguous.
6. Persist an audit event for grant changes and algorithm selection changes.

The frontend receives a read-only entitlement projection for display. It may hide unavailable choices, but every protected backend operation must repeat authorization. A modified client, stale local storage value or direct API call must not bypass a tier restriction.

Payment-provider webhooks must be signature-verified, idempotent and reconciled. Store provider transaction references and normalized payment state; never store raw card credentials. A pending checkout does not unlock an algorithm. Refund, chargeback, cancellation and administrative revocation must update the grant through an auditable state transition.

Do not hard-code prices or currency in Rust strategy code. Store the offer catalogue separately from algorithm manifests. Keep one-time access, time-limited access and renewal semantics explicit in the offer/grant schema; do not infer expiry from the fee amount.

## Switching algorithms

A user may switch only to an algorithm covered by their active grants.

- Switching changes the active selection for future runs; it does not rewrite old decisions.
- Each run and persisted trace retains its algorithm ID, exact version and configuration fingerprint.
- Switching must not erase instruments in Archive, user preferences, market history, paper ledger records or previous algorithm traces.
- The previous algorithm's state and performance history remain available for inspection.
- A switch should be atomic: validate entitlement and algorithm readiness, persist the new pinned selection, then acknowledge it.
- If the selected version is disabled or withdrawn, stop new runs safely and explain why. Do not silently substitute another version or algorithm.

## Independent evaluation and honest comparison

Every algorithm has its own immutable evaluation record. Compare algorithms only over compatible instruments, timeframes, event windows, data quality and execution assumptions.

Expose at least:

- sample count and evaluated date range;
- gross and net results after stated fees/slippage assumptions, where applicable;
- drawdown, volatility, win/loss distribution and downside risk;
- coverage / abstention rate and missing-data rate;
- turnover and exposure assumptions;
- out-of-sample and walk-forward results separately from in-sample development;
- algorithm version, parameters, data provenance and evaluation timestamp.

Do not rank or market an algorithm using return alone. Do not present paper results as live results, backtests as expected future returns, or a more expensive tier as a performance guarantee. Small samples and unavailable metrics must be labelled, not hidden.

## API shape (proposed)

Use the repository's existing Axum routes, Drogon gateway conventions and shared TypeScript contracts when implementing these endpoints:

- `GET /api/v1/algorithms` — catalogue plus the caller's read-only access state.
- `GET /api/v1/algorithms/{algorithm_id}` — manifest, supported capabilities and published evaluation summary.
- `GET /api/v1/me/entitlements` — effective grants and allowed capabilities for the authenticated user.
- `GET /api/v1/me/algorithm-selection` — active algorithm and pinned version.
- `PUT /api/v1/me/algorithm-selection` — validate entitlement and atomically change the active selection.
- `GET /api/v1/me/algorithms/{algorithm_id}/history` — only the caller's authorized history.
- Payment checkout and webhook routes must be added only after the payment provider and its signature-verification contract are selected.

The gateway validates request shape and applies rate limits; Axum owns identity context, entitlement checks, algorithm selection and research execution. MongoDB stores manifests, offers, grants, selections, audit events and run metadata with indexes that enforce uniqueness and user scoping.

## Data model and indexes (proposed)

- `algorithm_manifests`: unique `(algorithm_id, version)`; indexed by lifecycle status.
- `algorithm_offers`: unique `offer_id`; versioned price/currency and explicit grant semantics.
- `entitlement_grants`: indexed by `(user_id, status)`; unique provider transaction / grant idempotency key.
- `user_algorithm_selections`: unique `user_id`; stores active algorithm ID, pinned version and update timestamp.
- `algorithm_runs`: indexed by `(user_id, algorithm_id, instrument_id, timeframe, created_at)`; includes version, config fingerprint and trace reference.
- `algorithm_audit_events`: append-only user, actor, action, before/after state and timestamp.

All user-owned queries must include the authenticated user scope. Unique indexes and transactional updates should prevent duplicate grants and partially applied switches.

## Rollout plan

1. Preserve the existing deterministic engine as the first registered algorithm without changing its current signal math.
2. Define shared manifest, version, entitlement and selection contracts in Rust and shared TypeScript.
3. Add a server-side registry and read-only catalogue endpoint.
4. Add entitlement storage/resolution and denial tests before connecting payments.
5. Add atomic per-user algorithm selection and persist pinned versions.
6. Route the current engine through the algorithm interface as the first implementation.
7. Add the next algorithm as an independent implementation with isolated state and tests.
8. Add UI for algorithm discovery, access level, selection, version and transparent evaluation.
9. Integrate verified payment checkout/webhooks only after selecting a provider and confirming pricing/access semantics.
10. Run parity, isolation, authorization, replay, switching, revocation, refund and cross-platform contract tests.

## Non-goals and safety boundary

This foundation does not enable live broker orders, promise returns, invent algorithm performance, choose prices, or select a payment provider. A BUY/SELL output remains research information, not an instruction to place an order. Any future paper or live execution path must be separately authorized, risk-gated and evaluated.

## Implemented foundation (2026-11-08)

The branch now contains:

- `backend/rust-axum/src/algorithm_catalogue.rs`: validated read-only manifest catalogue for the existing vector baseline, exposed at the internal Axum route `GET /internal/algorithms`.
- `backend/rust-axum/src/algorithm_entitlements.rs`: pure fail-closed entitlement rules. Access requires a matching authenticated-user identity supplied by a trusted caller, an active grant, a positive verification timestamp, an effective start time, a non-expired grant, the requested algorithm ID and every required capability. Pending, unverified, expired, refunded, revoked and chargeback grants deny access.
- Unit tests cover successful authorization, cross-user isolation, unverified/pending payments, expiry, future activation, missing capabilities, revoked/refunded/chargeback states and invalid identity/clock inputs.

- `frontend/packages/core/src/qoinApi.ts`: shared TypeScript contracts for manifests, catalogue responses, entitlement projections and pinned selections. These are types only; no unavailable protected API is called by the client.

The entitlement module is currently a domain rule library, not a live authorization endpoint. It is deliberately not wired to caller-supplied user IDs, because this branch does not yet expose a verified Axum identity extractor or middleware. It is also not connected to payment webhooks or persisted grants. Do not use it to unlock paid access until the trusted identity, grant persistence, payment reconciliation and endpoint authorization layers are implemented and integration-tested.

The new module and its registration have been committed, but GitHub Actions has not yet reported a run for these connector-created commits. The Rust tests have not yet been executed in this environment; compilation and test status remain unverified.


## Decision transparency payload (implemented 2026-11-08)

Every newly persisted vector-baseline decision now carries an inspectable calculation ledger in addition to the existing output:

- **Raw inputs**: order-flow imbalance, funding rate, spread, EWMA absolute return, event intensity and liquidation count used by that invocation.
- **Feature trace**: each feature's raw observation, named transformation, scale, normalized value, clamped value, bullish/bearish coefficient and its weighted term after attention.
- **Attention**: the complete six-by-six attention probability matrix and attended feature vector. Rows are normalized softmax weights; this is a deterministic hand-specified interaction, not a trained transformer or causal explanation.
- **Vector result**: bullish and bearish components, magnitude, angle in radians and degrees, and the configured angular boundary.
- **Gate diagnostics**: explicit booleans for spread availability/limit, event evidence, magnitude threshold, angular tests and directional comparison, plus the exact configured limits.
- **Parameter snapshot**: the complete strategy weights, scales and thresholds used for that decision, alongside the strategy version.

The existing decision equations and action gates are unchanged by this observability addition. The explanation is persisted with the decision and therefore is also returned by the existing latest-vector and vector-history read paths and streamed with market updates.

Interpretation note: weighted feature terms are the signed coefficient multiplied by the attended feature. The final bullish/bearish components apply the existing non-negative floor after summing those terms. Attention weights show the model's deterministic interaction calculation; they must not be interpreted as causal attribution or learned feature importance. Missing funding is represented as a zero input in the calculation while the raw input record preserves null. Missing spread is represented by the strategy's existing conservative fallback for the feature calculation, while the liquidity gate remains failed.

This is a full calculation trace for the current deterministic baseline, not a claim that every future algorithm will use the same mathematics. Each future algorithm must publish its own versioned explanation schema and all intermediate operations relevant to its implementation.


## User-facing transparency surfaces (implemented 2026-11-08)

Both web and React Native instrument detail views render the baseline's full calculation trace:

- triggering event identity, venue, event type and timestamp, plus the accumulated state event count;
- a horizontally inspectable feature ledger with raw observation, transform name, scale, normalized value, clamped value and bullish/bearish post-attention weighted terms;
- every decision gate as an explicit pass/fail result and the configured thresholds;
- exact raw inputs (including nulls), liquidation side, the full parameter snapshot and the complete serialized decision payload;
- the six-by-six attention matrix and the vector trajectory already present in the instrument workspace.

The UI uses the strategy snapshot attached to the decision for contribution and threshold displays. It does not silently substitute a newer active strategy when inspecting an older decision. This is important for reproducibility: an old decision must be interpreted using the exact parameters that produced it.


## Historical event replay (implemented 2026-11-08)

A read-only replay endpoint is now available through `GET /api/v1/backtest` (Axum internal route `/internal/backtest`). It queries persisted raw observations for one symbol, an optional venue and an optional timestamp range, orders them by event timestamp then event ID, reconstructs the live engine's event-state updates, and calls the selected baseline strategy on each event in sequence. Replay is venue-scoped: if multiple providers have observations for the same symbol, callers must select one venue rather than blending different price feeds. The endpoint detects event-limit overflow and rejects the replay instead of silently returning a truncated sample. It never submits orders.

The simulation currently models long-only, all-available-cash entries on BUY and full exits on SELL. Fee and slippage assumptions are explicit query parameters (basis points). For an open long position, each persisted perpetual-funding event settles a cashflow using the event mark price: positive rates are paid by longs and negative rates are received. The response reports fills, decision counts, net equity/P&L, drawdown, fees, slippage, net funding cashflow and settlement count, the largest gap between adjacent raw observations, the number of gaps longer than five minutes, strategy version and assumptions. These are raw-event gap diagnostics, not a claim that candle intervals or market sessions are continuously covered. It rejects invalid data/cost assumptions and caps a replay at 50,000 observations.

Important limitations: this replays only raw events already present in MongoDB. It does not backfill Binance/Finnhub history or guarantee continuous coverage. Funding cashflows are only settled when a funding event includes its own valid mark price; the simulator never substitutes a stale trade or book price. Missing funding observations or mark prices are not inferred or interpolated. The model does not include short positions, leverage, borrow costs, liquidation, partial fills, latency or market impact. Fills require a current event price or valid bid/ask midpoint; a stale prior trade price is not reused as a new execution price. Open positions are marked to the latest available trade or book midpoint. Slippage assumptions must be below 10,000 basis points. The strategy is the currently active baseline snapshot at replay start; the response identifies its exact version. These results are research simulations, not live or expected performance. A dedicated historical provider/backfill pipeline, immutable strategy selection per historical timestamp, and richer execution simulator remain separate work.


## Market coverage tiers (preview design)

Market coverage is a separate product dimension from algorithm performance. A tier may restrict which market-data providers, venues, asset classes, instrument groups, timeframes, historical depth, active instruments and simultaneous stream subscriptions a user can access. It must not silently change the coefficients, normalization, decision gates or version of an algorithm. If a strategy itself is different, it must have a distinct algorithm ID/version and independent evaluation record.

The Axum catalogue now exposes three **preview-only** market-access package examples. These are product-design defaults, not active offers, prices, paid entitlements or enforced limits:

| Preview package | Providers / markets | Asset coverage | Instrument cap | Historical depth | Stream cap |
|---|---|---|---:|---:|---:|
| Starter | Binance public / USD-M | Major crypto perpetuals | 5 | 7 days | 5 |
| Plus | Binance public + optional Finnhub | Crypto, large-cap equities, major FX | 25 | 90 days | 15 |
| Pro | Binance public + optional Finnhub | Crypto, equities, FX, indices, commodities (energy, metals, agriculture) | 100 | 365 days | 50 |

Timeframe availability is also tiered. Starter begins at 5m; Plus and Pro include 1m. Finnhub coverage requires a configured key, provider entitlements and actual instrument support. Listing a provider or asset class in a preview tier does not guarantee a quote, historical series or licensed availability for every instrument.

**Broker distinction:** the current product is research-only and does not submit live orders. All preview tiers therefore expose an empty `execution_brokers` list. Binance and Finnhub in this catalogue are market-data providers/venues, not broker integrations. A future broker capability must be separately integrated, licensed, risk-controlled and tested before it can appear in an active tier.

The response advertises `market_access_tiers_enabled: false`; the existing entitlement rules are not yet connected to authenticated API authorization or payment reconciliation. The tier definitions do not currently enforce API, stream, instrument or history limits. Prices and commercial terms remain intentionally unspecified. A later enforcement layer must resolve a verified user grant and apply these limits server-side on every protected request; hiding instruments in the frontend is not access control.
