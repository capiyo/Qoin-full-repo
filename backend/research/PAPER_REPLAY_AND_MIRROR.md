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
- A signal intent queues until the next event containing a valid bid and ask. An opposite signal cancels an unfilled intent. Entries pay the ask plus slippage; exits receive the bid less slippage. Fees are charged on both sides.
- The current simulator is long-only, one position at a time, no leverage, no shorting, no stop-loss or daily-loss circuit breaker, no funding/financing debit, and no partial fills. `notional_per_trade` is the starting notional. With `reinvest_profits=true` (default), subsequent entry notional scales in proportion to current cash so both gains and losses compound; false keeps the starting notional fixed. The entry is capped by available cash after the entry-fee reserve. This is a baseline simulator, not a broker-specific execution model.
- Open positions are marked to estimated liquidation value including exit slippage and fee. They are not force-closed at the end of the replay.

## Durable Mirror records

Raw events remain in `market_events`; venue-isolated state snapshots and research decisions remain in `market_states` and `trade_decisions`. Live state is keyed by `(venue, symbol)`, not symbol alone. The service restores the latest state for each venue/symbol pair on restart. Events arriving older than that pair's event-time watermark are retained in raw history for chronological replay but are not applied to forward state or paper execution.

Historical replay writes:
- `backtest_runs`: run summary, capital/cost assumptions, strategy version and performance metrics.
- `backtest_decisions`: append-only per-event decision trace, including event ID/time, venue, action, reason, full feature vector, attention matrix, attended features, decision vector, magnitude/angle, spread and simulated execution outcome.

The operating cycle now has a forward paper-session phase. Configure up to 50 explicit paper accounts in `CycleConfig.paper_accounts`, each with a symbol, exact ingestion venue, quote currency, starting capital, per-entry notional, contract multiplier, quantity increment/lot step, compounding choice, fee and slippage assumptions. Each venue/symbol account has its own isolated cash ledger; no cross-currency conversion or shared-wallet allocation is inferred. A paper-mode cycle cannot start without at least one valid account. The first observation and one-hour diagnostic phases do not create fills. Once the session phase begins, new chronological events for configured accounts pass through the active strategy and the incremental paper simulator.

Forward session records are stored in:
- `paper_sessions`: durable current account balances, open positions, pending intents and cycle ID; restored after service restart.
- `paper_session_decisions`: append-only per-event Mirror record with feature/attention diagnostics, strategy version, action/reason, quote spread, simulated execution status, cash, equity and open-position state.
- `paper_session_trades`: closed simulated trades with entry/exit timestamps and prices, gross P/L, both-side fees, net P/L and return.
- `operating_cycles`: five-minute observation / one-hour diagnostic / session schedule and aggregate score/alignment/feature/attention plus per-feature bullish/bearish coefficient-contribution diagnostics. The coefficient values remain locked; this measures their contribution but does not auto-retune them.

Configure/start/stop/read the cycle through `/internal/operating-cycle`, `/internal/operating-cycle/start`, and `/internal/operating-cycle/stop`. Read live paper account state through `GET /internal/paper-session`, and the latest 1–200 Mirror decisions and closed trades through `GET /internal/paper-session/history?limit=50` (optionally filter by `symbol`). Historical backtest endpoints remain `GET /internal/backtests?symbol=BTCUSDT` and `GET /internal/backtests/{run_id}`. These records survive service restarts.

## Interpretation and limits

A run with zero market events is a data-availability result, not evidence of zero strategy returns. Validate event coverage, timestamps, spread/depth quality, venue consistency, and historical gaps before interpreting performance. Current replay uses the currently active strategy map across the entire selected period; it is not a historical strategy-version walk-forward. It does not yet model stop-losses, short positions, margin, financing/funding payments, partial fills, queue position, market impact, or exchange outages. Do not treat the simulator as enforcing your stated $2/day risk ceiling: a daily-loss circuit breaker, stop-loss and broker-specific risk sizing are not implemented yet. Do not use its output as a live-trading authorization or profitability claim.

The API is internal research infrastructure. Keep it on a trusted network; do not expose the internal Axum service publicly.


## Local client configuration

The flow service defaults to loopback at `127.0.0.1:8081`, and the shared frontend client defaults to `http://127.0.0.1:8081`. Set `VITE_QOIN_FLOW_API_URL` for the web build and `EXPO_PUBLIC_QOIN_FLOW_API_URL` for Expo/mobile when the service is hosted elsewhere. For a physical device on a trusted development LAN, bind the service to a LAN-reachable interface with `QOIN_AXUM_ADDR` and point the app to that machine's LAN address. Do not expose this internal service to the public internet.


The forward paper simulator is deliberately conservative and remains long-only, one position at a time per configured venue/symbol account. It waits for a valid two-sided quote, fills entries at ask plus slippage and exits at bid less slippage, rounds quantity down to the configured venue step, applies the configured linear contract multiplier to notional/PnL, and charges configured fees on both sides. It does not model shorts, leverage, margin, stop-losses, funding/financing, partial fills, queue position, market impact, exchange outages or FX conversion. Open positions are marked to estimated liquidation value; they are not forcibly closed when a cycle ends. The one-hour diagnostic phase reports feature/attention/score distributions but does not automatically retune coefficients. No broker orders are submitted and live execution remains disabled.
