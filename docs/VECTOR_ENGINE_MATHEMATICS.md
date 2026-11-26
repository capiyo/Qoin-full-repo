# Qoin V1 vector engine — mathematical contract

Updated: 2026-11-26  
Implementation: `backend/rust-axum/crates/qoin-vector-engine/src/lib.rs`  
Authority: Rust `qoin-vector-engine`; Python research delegates to Axum and does not reimplement decisions.

## Scope and preservation

V1 remains the existing deterministic seven-force engine. This document makes the existing Attend, Pulse and pressure-blade calculations explicit and exposes them as diagnostics. It does not add an independent oscillator, phase-coupled pulse system, game-payoff/equilibrium model, or alternative indicator engine. Those were research proposals, not accepted V1 production mathematics.

The seven fixed feature coordinates are:

[
x = [x_{mathrm{ofi}},x_{mathrm{liquidity}},x_{mathrm{volatility}},
x_{mathrm{funding}},x_{mathrm{openInterest}},x_{mathrm{liquidation}},x_{mathrm{basis}}].
]

Each raw observation must be finite. The market-data normalizer supplies causal values; the engine does not use future observations.

## Attend

Let (c) be the configured input clip and:

[
z_i = operatorname{clip}(x_i,-c,c).
]

The Attend matrix is row-normalized softmax of pairwise feature products:

[
A_{ij} =
rac{exp(z_i z_j - m_i)}
{sum_{k=1}^{7}exp(z_i z_k - m_i)},
qquad
m_i = max_k(z_i z_k).
]

Each attended feature is:

[
h_i = z_i + sum_{j=1}^{7} A_{ij}z_j.
]

The implementation exposes `Signal.attend` as the explicit Attend matrix and retains `Signal.attention` as a backward-compatible alias. `attended_features` contains (h_i). Every Attend row sums to one within floating-point tolerance.

A deterministic top-(k) selection ranks (|h_i|) descending and breaks ties by the fixed feature index. Unselected coordinates are set to zero before pressure calculation. (k) must be in ([1,7]).

## Pressure blades and Pulse

The V1 directional template is fixed:

[
w=[0.30,0.18,0,-0.10,0,0.12,-0.08].
]

Each feature's signed pressure-blade contribution is:

[
B_i = w_i h_i.
]

The interaction contribution is:

[
I = operatorname{clip}(gamma h_{mathrm{ofi}}|h_{mathrm{openInterest}}|,-0.25,0.25),
]

where (gamma) is the configured interaction coefficient and must satisfy (|gamma|leq0.25).

Pulse is the positive/negative decomposition of those existing signed contributions:

[
P_{mathrm{bull}} = sum_i max(B_i,0)+max(I,0),
]
[
P_{mathrm{bear}} = sum_i max(-B_i,0)+max(-I,0).
]

The net pressure before volatility damping is:

[
P_{mathrm{net}} = P_{mathrm{bull}}-P_{mathrm{bear}}
= sum_i B_i+I.
]

The serialized `Signal.pulse` is `[P_bull, P_bear]`. Each `FeatureTrace` records its Attend row, bullish/bearish Pulse contribution and signed pressure-blade contribution. The interaction term is retained separately so it is not double-counted as a feature blade.

## Score and decision

The volatility dampener is:

[
D = rac{1}{1+lambda |h_{mathrm{volatility}}|},
]

where (lambdageq0) is configured. The score is:

[
s = operatorname{clip}(P_{mathrm{net}}D,-s_{max},s_{max}).
]

The existing magnitude, alignment, angular, spread and spread-freshness gates then select BUY, SELL, CLOSE or WAIT. Missing, stale or over-limit spread data always abstains. Pulse/Attend fields are diagnostic exposures of the current calculation; they do not change the score or decision.

## Validated domains and determinism

- Every raw force and configured floating-point coefficient must be finite.
- `input_clip > 0`; `score_clip` is in ((0,1]); `top_k` is 1–7.
- `min_alignment` is in [-1,1]; `max_angle_radians` is in [0,π].
- `volatility_dampener >= 0`; `abs(interaction_coefficient) <= 0.25).
- `max_spread_bps > 0`; `max_spread_age_ms > 0`.
- Missing spread, stale spread and wide spread produce WAIT.
- Softmax subtracts the row maximum for numerical stability; a non-finite/zero sum falls back to a uniform row.
- Fixed feature ordering, deterministic top-(k) tie breaks and a versioned config make replay reproducible.
- The engine rejects NaN and infinity. Axum's market event layer handles duplicate/out-of-order event identity before a new observation reaches the engine.

## Provenance

Every `FeatureTrace` records the feature name, raw value, transform, scale, normalized and clipped values, top-(k) selection, attended value, directional weights, bullish/bearish weighted terms, signed pressure-blade contribution, interaction contribution and Attend row. `Signal` records raw inputs, normalized features, config version, rationale, decision vector, Pulse totals, Attend matrix and diagnostics.

Risk validation, capital allocation and order authorization remain separate downstream Rust responsibilities. A BUY/SELL signal is never an order authorization.
