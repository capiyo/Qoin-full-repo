# Multi-horizon research evaluation

## Purpose

Qoin evaluates market forces at multiple holding horizons without treating a single
market direction as universally valid. A short-horizon recovery can coexist with a
bearish daily structure. Each horizon therefore receives its own causal feature
observation, engine configuration, configuration version, and open-position flag.

## Supported horizons

| API value | Duration |
|---|---:|
| `one_minute` | 60 seconds |
| `five_minutes` | 300 seconds |
| `fifteen_minutes` | 900 seconds |
| `thirty_minutes` | 1,800 seconds |
| `one_hour` | 3,600 seconds |
| `four_hours` | 14,400 seconds |
| `one_day` | 86,400 seconds |
| `one_week` | 604,800 seconds |

## API

The gateway exposes `POST /api/v1/multi-horizon`; Drogon forwards the request to
Axum's internal `POST /internal/multi-horizon` handler. The stored-market-data
endpoint is `GET /api/v1/market-horizons` and requires both `symbol` and
`asset_class` so identically named forex, crypto, and stock instruments cannot
contaminate one another. The manually supplied single-observation endpoint is
`POST /api/v1/signal`. The request is a non-empty
array of `HorizonInput` objects. Every object includes:

- A unique horizon identifier.
- A separately computed `ForceObservation`.
- An explicit `EngineConfig` and configuration version.
- The open-position state for that horizon.

Duplicate horizons, empty input, non-finite observations, and invalid engine
configurations are rejected. Results are returned in stable horizon order.

## Consensus semantics

The response includes each independent signal and descriptive counts for BUY,
SELL, WAIT, and CLOSE. The relationship is one of:

- `aligned_buy`: all directional signals are BUY.
- `aligned_sell`: all directional signals are SELL.
- `divergent`: at least one BUY and one SELL are present.
- `no_directional_consensus`: there are no directional signals.

WAIT and CLOSE do not vote for a direction. A unanimous direction is populated
only when all directional signals agree. Consensus is not an aggregated trading
signal and must not be interpreted as permission to enter, reverse, or size a
position.

## Data and risk requirements

The caller must construct each observation from data available at or before the
decision timestamp. Do not duplicate one timeframe's observation across other
horizons and label it as independent evidence. Feature windows, normalization
statistics, and bar construction must be causal and must account for exchange
sessions, market-specific calendars, missing data, and feed staleness.

This endpoint is a deterministic research evaluator. It does not fetch market data,
perform portfolio allocation, calculate order size, submit orders, or establish
profitability. All horizons remain paper/research-only until chronological
backtesting, walk-forward and out-of-sample evaluation, realistic cost simulation,
and the existing independent risk gates have passed.


## Current observation normalization

The first market-event adapter contract normalizes the following inputs before they
reach the vector engine:

- OFI: signed aggressive trade volume divided by classified buy-plus-sell trade volume; unknown-side trades are excluded from both numerator and denominator and do not count as signed-flow evidence.
- Liquidity: mean signed bid/ask displayed-depth imbalance.
- Volatility: square root of summed squared log returns (realized volatility),
  scaled by 1% and clipped to [0, 3].
- Funding: fractional funding rate divided by 0.1% and clipped to [-3, 3].
- Open-interest change: fractional change divided by 1% and clipped to [-3, 3].
- Liquidation pressure: signed liquidation size divided by total liquidation size.
- Basis: fractional spot/perpetual basis divided by 0.1% and clipped to [-3, 3].

These are explicit initial scales, not fitted parameters or evidence of predictive
value. They must be calibrated chronologically by asset class and venue before
performance comparisons. The current market-backed endpoint uses the default
research engine configuration for every horizon; independent per-horizon
calibration remains a subsequent research task.


## Asset-class data requirements

The event contract classifies instruments as `forex`, `crypto`, or `stock`, and
the API/UI can isolate and select symbols by that class. This is not equivalent to
having a complete market universe or a connected vendor feed.

The current signal-readiness gate deliberately requires both classified trade
prints and order-book depth in a horizon. Consequently, a forex or equity instrument
whose provider supplies only candles or top-of-book quotes will remain not-ready
until an adapter supplies the required trade and depth evidence (or a separately
reviewed asset-class feature contract is implemented). Funding, OI, liquidations
and basis are optional and must remain explicitly unavailable where the market
does not publish them. Do not relabel quote changes or tick volume as true OFI.

The instrument catalogue is derived from symbols already present in
`market_events`. Complete exchange/venue discovery and provider-specific adapters
are not implemented in this branch.

## Query completeness and signal suppression

The stored-market-data endpoint currently loads at most 50,000 recent events per
symbol and asset class. When that safety cap is reached, all horizon signals are
suppressed for that response because one or more trailing windows may be missing
older records. The UI reports the condition and must not display the partial
observations as actionable signals. A future paginated/window-partitioned query
should replace this conservative cap before high-volume instruments are evaluated.

The endpoint requires an explicit asset class. Queries without it are rejected
rather than mixing events for symbols that collide across markets. The same
asset-class key is included in the event index used by the query.

## Horizon window coverage

A horizon is not considered ready merely because a burst of events meets the
minimum event count. The adapter measures the timestamp span between the first
and last usable events and divides it by the horizon duration. The initial
readiness policy requires at least 15% temporal coverage, in addition to the
minimum event count, classified trade evidence, and book-depth evidence.
Coverage is exposed as `coverage_ratio` in the API and UI.

The 15% threshold is a conservative completeness guard, not a calibrated
predictive parameter. It is a rolling wall-clock coverage proxy, not a full
exchange-session calendar. Before production use, this guard should be refined
with venue calendars, holidays, and instrument trading sessions, particularly
for equities and FX.
