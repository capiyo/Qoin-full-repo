# Functional multi-market vectors, confluence and portfolio mathematics

## Design boundary

Each vector point is an independently calculated observation. Feature calculations are pure functions: same inputs produce same outputs. Inputs must be timestamp-aligned and causal. FeaturePoint carries a signed normalized value, confidence, a separate non-directional risk intensity, and an availability flag. Missing inputs are unavailable; they are not imputed as neutral observations.

calculate_feature_vector assembles a canonical snapshot for crypto, forex, equities, commodities, fixed income, or other instruments. Venue adapters remain responsible for supplying correctly timestamped source data, unit conventions, and provenance. Neutral-change, volatility-reference, spread-reference, FVG-distance and basis scales are configurable per instrument/timeframe; fixed defaults are convenience baselines only. The feature library does not assume every market has funding, open interest, options, or centralized order flow.

## Individual calculations

- OFI: (V_buy - V_sell) / (V_buy + V_sell).
- Liquidation pressure: (short-liquidation volume - long-liquidation volume) / total liquidation volume; forced short closures create buy flow and forced long closures create sell flow.
- Price momentum: rolling log-price change compressed by an instrument/timeframe-specific neutral return.
- Event intensity: event count relative to an expected event rate, stored as non-directional risk intensity.
- Open-interest change: relative change from previous to current OI, compressed by delta / (abs(delta) + 0.02), stored as positioning context rather than a standalone bullish/bearish vote.
- Funding pressure: negative tanh(funding / neutralBand); positive funding is treated as crowded-long context.
- Liquidity stress: spread relative to an instrument-specific reference spread. It is stored as risk intensity, not bearish direction.
- Volatility: rolling standard deviation of log returns; stored as risk intensity, not direction.
- Fibonacci: distance to the nearest 23.6%, 38.2%, 50%, 61.8%, or 78.6% retracement of a confirmed swing, decayed by a configured tolerance. Swing pivots must be confirmed before use.
- FVG: three-candle displacement gap; subsequent bars measure mitigation and full traversal invalidates the zone. Bullish zones are positive, bearish zones negative.
- Value Area: volume-at-price bins; POC is the maximum-volume bin and VAH/VAL expand from POC toward the larger adjacent bin until the configured fraction (normally 70%) is included. Candle volume alone is not an exact volume-at-price profile.
- GEX: sum(gamma * OI * multiplier * spot^2 * 0.01 * dealerSign). Stored as gamma-regime context plus negative-gamma risk, not a standalone bullish/bearish vote. Requires a real options chain and a documented dealer-side sign assumption. It does not infer gamma flip from spot candles.
- Basis: spot/derivative price difference in basis points, scaled by an instrument-specific reference and retained as carry/dislocation context rather than an outright directional vote.

Non-directional stress dimensions do not masquerade as sell signals. They can contribute to a separate risk level and to explicitly configured interactions.

## Confluence

Let x_i in [-1,1] be a feature value, c_i in [0,1] its confidence, and w_i >= 0 its configured weight. Only available observations count.

C = sum(i in A, w_i*c_i) / sum(all i, w_i)

P0 = sum(i in A, w_i*c_i*x_i) / sum(i in A, w_i*c_i)

Pair interactions add bounded, explicit confluence terms. Each point also carries non-directional risk and signed context (for example OI buildup or positive/negative gamma regime). For interactions, z_i is the directional value when nonzero, otherwise signed context when present, otherwise risk intensity:

P = clip[-1,1](P0 + clip[-Imax,Imax](sum((i,j) in E, gamma_ij*c_ij*z_i*z_j)))

Agreement and contradiction are calculated separately from positive and negative weighted evidence. Low coverage, high contradiction, or excessive risk causes WAIT. The default coefficients cover thirteen dimensions and sum to one. They are transparent research starting values, not fitted parameters or proof of an edge. Correlated dimensions must be audited to prevent double-counting.

## Portfolio and cross-market calculations

Given aligned return rows r(t,i) and long-only weights w_i summing to one:

r(p,t) = sum(i, w_i*r(t,i))

Sigma(i,j) = Cov(r_i,r_j)

rho(i,j) = Sigma(i,j) / sqrt(Sigma(i,i)*Sigma(j,j))

The portfolio diagnostics calculate sample volatility, historical lower-tail VaR and expected shortfall, peak-to-trough drawdown, concentration HHI, and diversification ratio. Return series must be aligned across instruments; mismatched timestamps should be rejected by the caller before invoking the functions.

## Safety and validation

- No function sizes positions, submits orders, or changes the existing ForceObservation / decide API.
- These functions are library-level research calculations. The existing Axum event endpoint does not yet supply candle history, volume-at-price, options-chain, and portfolio-return inputs; wiring those data feeds and persistence is a separate integration step.
- GEX is unavailable without options-chain data. Value Area is unavailable without volume-at-price bins. Missing data lowers confluence coverage.
- Run cargo test --workspace and repository CI before treating this implementation as validated. Keep both V1 and V2 paper/research-only until independent risk validation and explicit written approval.
