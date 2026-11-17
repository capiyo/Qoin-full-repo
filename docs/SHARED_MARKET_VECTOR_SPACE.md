# Shared market vector space

**Status:** Implemented schema and version adapters; research/paper-only. No profitability or production-readiness claim.

## One market state, two independent algorithms

`qoin-market-space` is a version-independent Rust crate consumed by both Qoin algorithm crates. It defines schema v1 as a fixed 53-coordinate state vector. The vector is the common input contract, not a third algorithm and not a learned embedding.

Each snapshot carries a stable vector ID and schema version; venue, symbol and timeframe; an event-time cutoff and the time the snapshot became available; all 53 coordinates in a stable enum order; and an optional normalized value plus an independent data-quality score for every coordinate.

Every present coordinate must be finite and normalized to [-1, 1]. Missing values remain explicit `None` with quality zero; they are never silently imputed as zero. Calibration and normalization are the caller's responsibility and must use past-only data. A snapshot is rejected if its availability timestamp precedes its event-time cutoff.

## Coordinate families

| Family | Coordinates |
|---|---|
| Price | log return, displacement, rate of change, gap magnitude, swing position |
| Volatility | realized/implied volatility, volatility of volatility, skew, kurtosis, jump intensity |
| Trend | momentum, strength, persistence, mean reversion, exhaustion |
| Liquidity | spread, book depth, OFI, volume delta, impact, slippage, concentration |
| Cross-asset | correlation, covariance, beta, correlation breakdown, relative strength |
| Derivatives | funding, open-interest change, liquidation pressure, gamma, delta, basis |
| Events | event intensity, macro surprise, event proximity |
| Regime | trend strength, volatility level, liquidity stress, correlation stress, transition intensity, uncertainty, persistence |
| Portfolio | exposure, concentration, drawdown, tail risk, risk contribution |
| Temporal | time since structural break, feature persistence, signal decay, timeframe agreement |

Regime is therefore a set of coordinates in the same market state. It is not a separate hard-coded label or an external switch.

## Version 1 — numerical vector intelligence

`qoin-vector-engine::decide_from_market_vector` consumes the shared vector. It maps the baseline's required numerical forces from the common schema and requires regime volatility, liquidity stress, correlation stress, transition intensity and uncertainty to be present. Positive regime coordinates denote elevated risk. Their average raises the directional evidence threshold; elevated regime volatility also increases volatility dampening. Regime coordinates constrain the signal but do not create a bullish or bearish vote.

The original `decide(ForceObservation, ...)` API remains available as the legacy baseline. The new adapter fails closed when a required coordinate is unavailable.

## Version 2 — set intelligence and ledger

`EvidenceSet::from_market_vector` converts the exact same schema into explicit condition atoms using a versioned, caller-configurable binning policy: positive, negative or neutral for sufficiently reliable observed coordinates; low-quality for observed values below the quality threshold; and missing for unavailable coordinates when missingness recording is enabled.

The continuous normalized values remain in the common vector; V2's set conversion is a deliberate symbolic abstraction, not a replacement for the source vector. Evidence sets and relationship records retain vector ID, schema version, venue, symbol and timeframe. The relationship ledger therefore records which common vector snapshots produced each relationship.

The internal route `POST /internal/v2/relationships/discover-vectors` accepts a query vector and candidate vectors, converts them to evidence sets and uses the existing deterministic set-attention and append-only ledger path.

## Geometry and comparability

The shared crate provides quality-weighted cosine similarity and weighted Euclidean distance over dimensions observed in both vectors. It reports common-dimension count and coverage. Comparisons with no common dimensions or a zero norm are rejected.

These metrics are descriptive geometry, not probabilities, causal explanations or trade signals. Covariance-aware distances, learned projections and feature-family weighting require separately specified calibration and out-of-sample validation.

## Causality and evaluation requirements

- Vector IDs must identify immutable snapshots.
- `event_time_cutoff_ms` is the latest event time included; `available_at_ms` records when the snapshot could first be used.
- Backtests must use `available_at_ms` to prevent late-arriving data from leaking into earlier decisions.
- Every normalization parameter must be fit on past-only data and versioned by the caller.
- Missingness and quality are first-class inputs. Do not replace missing features with zero.
- Compare V1 and V2 on identical vector snapshots, timestamps, costs, instruments and risk budgets.
- Do not promote strategies based on similarity or attention weights alone.

The new V1 and V2 endpoints are internal research interfaces. They do not submit orders, size positions or enable real-money execution.
