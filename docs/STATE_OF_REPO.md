# State of repository

The repository uses React web and React Native clients, a Rust/Axum authoritative
event and vector engine, a C++/Drogon HTTP gateway, and MongoDB persistence. The
vector engine exposes independent multi-horizon research evaluation and a
market-event aggregation path.

## Multi-horizon implementation

- Eight horizons are supported: 1m, 5m, 15m, 30m, 1h, 4h, 1d and 1w.
- Market events are tagged as `forex`, `crypto` or `stock`. The stored-market
  horizon endpoint requires an explicit asset class so identically named
  instruments cannot contaminate one another.
- Each horizon is aggregated from its own trailing event window using only
  events at or before the decision timestamp.
- A horizon requires the configured minimum event count, classified buy/sell
  trade evidence, order-book depth evidence, and at least 15% temporal coverage
  of its target window before it can emit a signal.
- `coverage_ratio` and `data_quality` are exposed to the web and mobile
  research screens. Data quality is a feed-completeness diagnostic, not a
  probability of signal correctness.
- Unknown-side trade volume is excluded from both the signed OFI numerator and
  denominator; it does not dilute classified buy/sell flow.
- The event query is capped at 50,000 records. If the cap is reached, all
  horizon signals are suppressed because older portions of the window may be
  truncated.
- Missing funding, open interest, liquidations and basis are reported as
  unavailable and treated as neutral by the current baseline. Dedicated basis
  events are supported.
- The gateway exposes `POST /api/v1/signal` for one normalized observation,
  `POST /api/v1/multi-horizon` for separately configured horizon inputs, and
  `GET /api/v1/market-horizons` for stored market-event aggregation.
- Web and mobile research screens select an asset class and instrument and
  display readiness, window coverage, data quality, signals and horizon
  divergence/agreement.

## Validation status

Completed GitHub Actions validations on recent commits include:

- Rust workspace tests: 19 tests passed, 0 failed, including horizon divergence,
  empty/duplicate horizon rejection, causal window filtering, temporal coverage,
  unknown-side OFI handling and realized-volatility calculation.
- Axum + Drogon + MongoDB integration: passed, including service restart,
  strategy persistence, event ingestion, instrument discovery, asset-class
  forwarding, horizon readiness/coverage, the single-signal endpoint and the
  multi-horizon divergence endpoint.
- Drogon gateway build: passed.
- React web production build: passed.
- React Native strict TypeScript check: passed.
- Python reference tests and market-event JSON schema validation: passed.

The latest code validation completed successfully on commit
`655c677df0cd0ae0ff4412a103a86f2b8470a435`. All six relevant GitHub Actions
checks passed, including the Axum/Drogon/MongoDB end-to-end run. Subsequent
changes to this file are documentation-only.

## Limitations

- Market-data adapters for complete forex, crypto and stock universes are not
  wired to this endpoint. The API aggregates events already ingested; it does
  not fetch exchange/vendor data itself.
- Current normalization and the default engine configuration are research
  baselines, not past-only rolling calibration across venues, asset classes or
  horizons. The stored-market endpoint currently uses the same default engine
  configuration for each ready horizon; independently calibrated profiles
  remain future research work.
- The 15% coverage threshold is a conservative rolling wall-clock proxy, not a
  full venue/session calendar. Production use needs exchange calendars,
  holidays and instrument-specific sessions.
- No event-driven backtester, walk-forward performance report, realistic
  fee/slippage model, shadow deployment, or profitability evidence exists yet.
- No live-order route is registered. Authentication/authorization, a
  production risk engine, protective stops, position/order reconciliation,
  kill switch and crash recovery remain incomplete.
- Market data must be venue-normalized and symbol-mapped before cross-asset
  comparisons are meaningful.

Keep this system research/paper-only. Do not claim production readiness or
profitability.
