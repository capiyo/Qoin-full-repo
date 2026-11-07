# Qoin paper replay and Mirror trace

## What it does

The flow-engine service exposes a deterministic, long-only paper replay over persisted `market_events`. It replays events in ascending `ts_ms` order and calls the same active `EdgeStrategy::decide` implementation used by the ingestion path. The active strategy version is recorded with each run.

The replay is venue-isolated. A symbol may be listed on multiple brokers or venues; never combine their books into one synthetic stream. Supply the exact `venue` value used by ingestion.

## API

`POST /internal/backtest`

Example request:

```json
{
  "symbol": "BTCUSDT",
  "venue": "your-exact-ingestion-venue",
  "quote_currency": "USDT",
  "start_ts_ms": 1794000000000,
  "end_ts_ms": 1794086400000,
  "warmup_hours": 24,
  "initial_cash": 20000,
  "notional_per_trade": 2000,
  "reinvest_profits": true,
  "fee_bps": 10,
  "slippage_bps": 5,
  "max_events": 100000
}
```

- `start_ts_ms` and `end_ts_ms` are Unix milliseconds.
- `initial_cash` and `notional_per_trade` are denominated in `quote_currency`. The simulator does not convert KSh to USD/USDT or model contract multipliers; convert the account balance to the instrument quote currency before running, and do not enter KSh 20,000 as USDT 20,000.
- The warmup window (1–168 hours) updates causal feature state before the measurement window. Warmup events do not create signals or trades.
- The replay is capped at 100,000 fetched events per request. Narrow the time window if the cap is reached.
- A fill is possible only on an event containing a valid bid and ask. Entries pay the ask plus slippage; exits receive the bid less slippage. Fees are charged on both sides.
- The current simulator is long-only, one position at a time, no leverage, no shorting, no stop-loss, no funding/financing debit, and no partial fills. `notional_per_trade` is the starting notional. With `reinvest_profits=true` (default), subsequent entry notional scales in proportion to current cash so both gains and losses compound; false keeps the starting notional fixed. The entry is capped by available cash after the entry-fee reserve. This is a baseline simulator, not a broker-specific execution model.
- Open positions are marked to estimated liquidation value including exit slippage and fee. They are not force-closed at the end of the replay.

## Durable Mirror records

Raw events remain in `market_events`; ingestion state snapshots and decisions remain in `market_states` and `trade_decisions`.

Each replay writes:
- `backtest_runs`: run summary, capital/cost assumptions, strategy version and performance metrics.
- `backtest_decisions`: append-only per-event decision trace, including event ID/time, venue, action, reason, full feature vector, attention matrix, attended features, decision vector, magnitude/angle, spread and simulated execution outcome.
- `operating_cycles`: the current five-minute observation / one-hour diagnostic / session state, including counters and score/alignment aggregates. The flow API restores this record on restart and updates it as new ingested events are evaluated.

Retrieve the latest 50 runs with `GET /internal/backtests?symbol=BTCUSDT`, or a specific run and its trace with `GET /internal/backtests/{run_id}`. Configure/start/stop/read the persistent operating cycle through `/internal/operating-cycle`, `/internal/operating-cycle/start`, and `/internal/operating-cycle/stop`. These collections survive service restarts.

## Interpretation and limits

A run with zero market events is a data-availability result, not evidence of zero strategy returns. Validate event coverage, timestamps, spread/depth quality, venue consistency, and historical gaps before interpreting performance. Current replay uses the currently active strategy map across the entire selected period; it is not a historical strategy-version walk-forward. It does not yet model stop-losses, short positions, margin, financing/funding payments, partial fills, queue position, market impact, or exchange outages. Do not use its output as a live-trading authorization or profitability claim.

The API is internal research infrastructure. Keep it on a trusted network; do not expose the internal Axum service publicly.


## Local client configuration

The flow service defaults to loopback at `127.0.0.1:8081`, and the shared frontend client defaults to `http://127.0.0.1:8081`. Set `VITE_QOIN_FLOW_API_URL` for the web build and `EXPO_PUBLIC_QOIN_FLOW_API_URL` for Expo/mobile when the service is hosted elsewhere. For a physical device on a trusted development LAN, bind the service to a LAN-reachable interface with `QOIN_AXUM_ADDR` and point the app to that machine's LAN address. Do not expose this internal service to the public internet.


The operating-cycle session currently records and monitors incoming research decisions; it does not yet generate forward simulated fills or send broker orders. The historical replay endpoint is the current paper-fill simulator. The one-hour diagnostic phase reports score/alignment and decision distributions but does not automatically retune coefficients.
