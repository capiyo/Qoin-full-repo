# Qoin: A Dynamic Market-Force Vector Framework
## Observational foundations, mathematical specification, and philosophy of decision-making

**Document class:** Research and engineering design paper  
**Version:** 1.0  
**Date:** 24 October 2026  
**Status:** Conceptual specification plus an explicitly limited research baseline. Not investment advice, not evidence of profitability, and not a production trading specification.

---

## Abstract

Qoin models a market as a changing field of observable forces rather than as a single price forecast. Order-flow imbalance, depth and spread, volatility, funding, open-interest change, liquidation pressure, event intensity, and basis can each be represented as a time-varying feature or vector. A decision engine filters noisy observations, normalizes features using only information available at the decision time, measures directional alignment and magnitude, limits attention to reliable high-impact forces, and may abstain when evidence is weak or market quality is poor.

The central design principle is **current evidence first, historical data as disciplined context**. Historical observations are needed for causal scaling, filtering, and stability estimation, but old outcomes should not dominate the current state. Each position episode has temporary adaptive state; when the position is fully closed, that state is reset. Partial fills remain in the same episode. A reversal is modeled as a close followed by a new open. Trade outcomes are stored for offline research, not allowed to rewrite live weights immediately.

This paper distinguishes three things throughout: (1) the philosophy and research hypotheses, (2) the proposed mathematical specification, and (3) what is actually present in the current repository snapshot. The current code is a hand-specified research baseline, not a trained model. The advanced features described here are targets unless specifically marked implemented. No claim of profitable performance is made.

## 1. The question Qoin is trying to answer

A price chart compresses many processes into one observable series. A market move may be associated with aggressive buying, thin ask-side liquidity, a funding imbalance, a volatility regime shift, or forced liquidations. These processes can reinforce or offset one another. Qoin asks:

> Given the market evidence observable now, which forces are aligned, how reliable are they, how quickly are they changing, and is the combined evidence strong enough to justify action after costs and risk?

The engine is not meant to declare that a market has one true hidden direction. It constructs a conditional estimate from imperfect measurements and should explicitly represent uncertainty through abstention gates, reliability weights, and risk constraints.

A directional label is not itself an order. A signal such as BUY is only a candidate action. A separate risk and execution layer must decide whether an order is allowed, what size is appropriate, how it is protected, and whether exchange state agrees with internal state. In the current branch, the live order adapter is intentionally not wired into the decision path.

## 2. Observation ontology: market forces as measurable proxies

A “force” here is an engineering metaphor for a measured influence on short-horizon price formation, not a claim that market price literally obeys a classical force law. Each force is a proxy constructed from data.

| Symbol | Force | Possible observation | Interpretation and caveat |
|---|---|---|---|
| (f_1) | Order-flow imbalance (OFI) | Signed aggressive volume or order-book change | Measures directional pressure; feed definitions differ by venue |
| (f_2) | Liquidity stress | Spread, depth asymmetry, depth withdrawal | Thin liquidity can amplify movement, but does not determine direction by itself |
| (f_3) | Volatility shock | Causally filtered absolute returns or realized variance | Captures risk and regime change; high volatility is not inherently bullish or bearish |
| (f_4) | Funding pressure | Funding rate relative to a past-only baseline | Crowding/carry proxy; can remain extreme without immediate reversal |
| (f_5) | Open-interest change | Change in open interest over a defined interval | Helps distinguish position build-up from position reduction, but is ambiguous without price/flow context |
| (f_6) | Liquidation pressure | Liquidation events, side, size, intensity | Forced flow can cascade; venue coverage and reporting latency matter |
| (f_7) | Event intensity | Arrival rate of trades, book changes, liquidations, or other events | Captures activity clustering; activity alone has no directional sign |
| (f_8) | Basis / cross-venue divergence | Futures-spot or venue price difference | Can reflect carry, inventory, latency, or market dislocation rather than a free arbitrage |

The feature universe should remain modular. A feature is admitted only when its definition, timestamps, units, source quality, and missing-data behavior are explicit. Features that are highly correlated should not be counted as independent confirmations merely because they have different names.

### 2.1 Order-flow imbalance

A simple signed trade-flow proxy over a window (W_t) is:

[
OFI_t = rac{sum_{i in W_t} s_i q_i}{epsilon + sum_{i in W_t} q_i},
]

where (q_i ge 0) is trade size, (s_i=+1) for buyer-initiated flow, (s_i=-1) for seller-initiated flow, and (epsilon>0) avoids division by zero. This normalized form lies approximately in ([-1,1]). It is only as reliable as aggressor-side classification and event ordering.

For order-book changes, OFI can instead combine changes in best bid and ask queues. The exact definition must be consistent between backtest and live ingestion; otherwise apparent performance may be a data-contract artifact.

### 2.2 Liquidity and spread

The relative spread is:

[
spread_t^{bps}=10^4rac{ask_t-bid_t}{(ask_t+bid_t)/2}.
]

Spread is primarily a *quality and cost* variable. A wide spread can justify WAIT even if directional evidence is strong. Depth imbalance can be represented as:

[
I_t=rac{D^{bid}_t-D^{ask}_t}{D^{bid}_t+D^{ask}_t+epsilon}.
]

Depth is ephemeral. A displayed queue can cancel before execution, and a snapshot may be stale. Therefore depth-derived features require timestamps, freshness checks, and conservative treatment of missing or crossed books.

### 2.3 Volatility and event intensity

A causal EWMA of squared returns can be defined by:

[
v_t = lambda v_{t-1} + (1-lambda)r_t^2,quad 0<lambda<1.
]

The scale parameter must be estimated only from the past. For event arrivals (N(t)), a simple intensity estimate is a count per time window. A Hawkes-style research candidate models self-exciting event clusters:

[
lambda(t)=mu+sum_{t_i<t}alpha e^{-eta(t-t_i)}.
]

This formula says that past events can temporarily raise the expected rate of future events. It does not by itself predict direction. The parameters require causal estimation and stability tests, and should not be confused with the current baseline's simpler decaying event score.

### 2.4 Funding, open interest, liquidations, and basis

Funding and open interest are context variables, not standalone buy/sell instructions. A rising price with rising open interest can mean new positions are entering, but cannot uniquely identify which side is informed. Liquidation flow can push prices further in the direction of forced execution, while also indicating that a move is late or fragile. Basis can widen because of carry and inventory constraints, not just mispricing. Each feature therefore enters as conditional evidence and interaction, not a deterministic rule.

## 3. Causal time, data contracts, and observation quality

Every event needs at least a source, symbol, event type, source event identifier, event timestamp, ingestion timestamp, and units. The system must distinguish **event time** from **arrival time**. If a feed arrives late, a backtest may not use it before the live system could have known it. Replaying sorted-by-event-time data without modeling delivery latency can create look-ahead bias.

For a decision at time (t), admissible data are those available by (t). A feature normalizer, filter, or model parameter must not use future samples. A rolling mean and variance should use observations strictly before the decision sample (or a documented causal update order). Missing or stale data should reduce confidence or cause abstention; it should not silently become a neutral value unless that behavior is an explicit, tested design choice.

Recommended event checks:
1. Schema and numeric validity (finite values, nonnegative sizes, valid sides).
2. Source/symbol allowlist and event ID deduplication.
3. Timestamp monotonicity policy and bounded late-event handling.
4. Book sanity: bid/ask ordering, spread bounds, depth consistency.
5. Freshness thresholds and heartbeat monitoring.
6. Per-source health and explicit missingness indicators.
7. Persistence of raw events before any derived conclusion is treated as durable.

Current repository status: the Axum service contains event ingestion, event deduplication, in-memory state updates, and MongoDB writes. Full restart replay, complete event-time reconciliation, and all of the guards above are not yet demonstrated. Treat these as unfinished engineering requirements.

## 4. From observations to normalized forces

Raw features use different units. A funding rate, an OFI ratio, a spread in basis points, and a liquidation count cannot be summed directly.

For each feature (x_{j,t}), define a causal location and scale from a historical window ending before (t):

[
z_{j,t}=rac{x_{j,t}-m_{j,t^-}}{s_{j,t^-}+epsilon}.
]

Here (m_{j,t^-}) and (s_{j,t^-}) may be robust median/MAD estimates, rolling moments, or exponentially weighted statistics, provided they are fitted causally. Clip extreme values with a documented bound:

[
	ilde z_{j,t}=max(-c_j,min(c_j,z_{j,t})).
]

Robust scaling limits the ability of one malformed tick or extreme event to dominate the combined vector. However, clipping can erase genuine tail information. A better design may retain a separate tail flag or regime indicator rather than merely clipping all extremes.

**Past-only normalization is a target requirement; the current baseline uses fixed hand-chosen scales for several features.** Those fixed scales are transparent but may fail across symbols and regimes. A versioned configuration must specify each scale, clipping bound, window, missing-value rule, and calibration timestamp.

## 5. Filtering before derivatives

Numerical derivatives amplify noise. If raw feature observations contain measurement noise, the difference (x_t-x_{t-1}) can be dominated by that noise; a second difference is even less stable. Therefore, velocity and acceleration should be computed only after causal filtering.

A basic exponential filter is:

[
ar x_t=alpha x_t+(1-alpha)ar x_{t-1},quad 0<alphale1.
]

With timestamps measured in seconds, define:

[
Delta t_1=t_{k-1}-t_{k-2},quad Delta t_2=t_k-t_{k-1},
]
[
v_k=rac{ar x_k-ar x_{k-1}}{Delta t_2},
quad
v_{k-1}=rac{ar x_{k-1}-ar x_{k-2}}{Delta t_1},
]
[
a_k=rac{v_k-v_{k-1}}{(Delta t_1+Delta t_2)/2}.
]

The dimensions matter: if the force is dimensionless, velocity has units per second and acceleration per second squared. A unit test must assert these conventions. Unequal intervals must use actual elapsed time, not an assumed fixed bar interval.

A Kalman filter is a research candidate when a defensible state/measurement model exists. A particle filter may be justified for nonlinear or multimodal state estimates, but complexity is not evidence of predictive value. Curvature (a higher derivative or vector turning-rate proxy) should remain disabled until it adds robust out-of-sample value after costs.

## 6. A dynamic vector representation

Let the normalized force vector at time (t) be:

[
mathbf f_t=[	ilde z_{1,t},	ilde z_{2,t},ldots,	ilde z_{d,t}]^T.
]

There are two related but distinct interpretations:
- **Feature-space vector:** each component is a force feature. The vector is a compact state representation.
- **Directional decision vector:** a low-dimensional vector, often (mathbf d_t=[B_t,S_t]^T), where (B_t) and (S_t) summarize bullish and bearish evidence.

Do not conflate the feature vector with physical position or price velocity. A geometric visualization is useful for reasoning, but its axes and transformations must be explicitly defined. The vector's magnitude is a score in a chosen feature space, not automatically a probability or expected return.

### 6.1 Attention over forces

An attention mechanism can learn or calculate which features are relevant to one another. In a standard scaled dot-product form:

[
Q=FW_Q,quad K=FW_K,quad V=FW_V,
]
[
A=operatorname{softmax}left(rac{QK^T}{sqrt{d_k}}ight),quad H=AV.
]

For a compact deterministic baseline, a simpler scalar interaction can be used:

[
a_{ij}=rac{exp(q_i k_j/	au)}{sum_{ell=1}^dexp(q_i k_ell/	au)},
quad
h_i=f_i+sum_j a_{ij}v_j.
]

The temperature (	au) controls concentration. Small (	au) creates sharp attention; large (	au) spreads weight. In the present research baseline, the attention calculation is deterministic and hand-specified, not trained from outcomes. It should be interpreted as an interaction heuristic, not as learned understanding.

Attention weights are not causal explanations and are not guaranteed to indicate feature importance. Validate them through ablation, permutation, and stability analysis.

### 6.2 Pairwise interactions and regularization

Two forces can reinforce or offset one another. A simple interaction term is:

[
g_{ij}=f_i f_j.
]

Unregularized products can explode when inputs are extreme. A bounded alternative is:

[
	ilde g_{ij}=rac{f_i f_j}{1+ho|f_i f_j|},quad hoge0.
]

A linear score with interactions might be:

[
s_t=mathbf w^Tmathbf f_t+
sum_{i<j}gamma_{ij}	ilde g_{ij}.
]

The coefficients (gamma_{ij}) must be regularized and constrained. For (d) features there are (d(d-1)/2) pairwise interactions; the parameter count grows quadratically. Too many interactions create an overfitting pathway. Start with a small pre-registered subset, shrink coefficients, measure effective degrees of freedom, and compare against a linear baseline.

### 6.3 Pareto selection of reliable forces

Qoin's Pareto principle means prioritizing a small set of high-impact, reliable forces rather than averaging every available signal equally. For each force, define magnitude (m_i=|f_i|) and reliability (r_iin[0,1]). Force (i) is dominated if another force (j) has both (m_jge m_i) and (r_jge r_i), with at least one strict inequality.

The non-dominated frontier is then ranked by a declared utility such as (u_i=m_i r_i), and at most (k) forces are retained. This is a heuristic for attention management, not a universal law that exactly 20% of features always explain 80% of returns. The Pareto pattern must be measured empirically. Selection stability and feature redundancy should be evaluated over walk-forward windows.

## 7. Direction, angle, magnitude, and abstention

Let bullish and bearish evidence be nonnegative scores:

[
B_t=max(0,mathbf c_B^Tmathbf h_t),quad
S_t=max(0,mathbf c_S^Tmathbf h_t).
]

Define the directional vector:

[
mathbf d_t=(B_t,S_t),quad
M_t=|mathbf d_t|_2=sqrt{B_t^2+S_t^2}.
]

One angular representation is:

[
	heta_t=operatorname{atan2}(S_t,B_t).
]

When (B_t) dominates, (	heta_t) is near zero; when (S_t) dominates, it is near (pi/2). A buy candidate may require (	heta_tle	heta_B), and a sell candidate may require (pi/2-	heta_tle	heta_S), with minimum magnitude (M_tge M_{min}). The angular thresholds and magnitude threshold must be calibrated out-of-sample, not optimized repeatedly against the final test period.

A useful decision gate is conjunctive:

- Data quality and freshness pass.
- Spread and estimated transaction costs are acceptable.
- Directional angle lies inside the appropriate boundary.
- Magnitude exceeds a minimum threshold.
- Signal is stable enough over a specified interval.
- Independent risk gate approves exposure and protective orders.
- Position/order state allows this action without duplication.

If any required condition fails, WAIT. Abstention is a first-class decision, not a model failure. A system that trades less but avoids low-quality states may outperform a system that always predicts a direction, but that proposition requires testing after costs.

### 7.1 Hysteresis and stability

A signal close to a boundary can flip repeatedly due to noise. Hysteresis uses different entry and exit thresholds, or requires a candidate to persist for a minimum number of observations. Stability can be measured as the fraction of recent samples agreeing in direction, the variance of angle, or the rate of sign changes. These gates add delay; they must be tested against latency and missed-move costs.

### 7.2 Signal is not probability

A score, angle, or attention distribution is not a calibrated probability of profit. If probability is needed for position sizing, estimate and calibrate it on strictly out-of-sample data and assess reliability diagrams, Brier score, and calibration drift. Never interpret softmax weights as odds that a trade wins.

## 8. Trade episodes and temporary learning state

A core design choice is **fresh adaptive state per trade episode**. The goal is to avoid allowing the result of yesterday's trade to contaminate the next trade through an unbounded, path-dependent live weight update.

An episode state can be written as:

[
mathcal E_n={w^{(n)}_t, 	ext{observations}, 	ext{decisions}, 	ext{fills}, 	ext{risk state}}.
]

At the start of episode (n), initialize temporary weights from a fixed, versioned prior (w_0), or from a tightly constrained prior established offline. Update only from information available during that episode. At full close, freeze the episode record, calculate outcome metrics, and reset the temporary adaptive state for the next episode.

Lifecycle rules:
- Partial fills belong to the same episode and update filled quantity and average entry price.
- A partial close does not finish the episode if exposure remains.
- A full close finalizes the episode only after exchange reconciliation.
- A reversal must be represented as closing the old exposure and opening a separate new episode.
- A rejected or canceled order is not a completed trade.
- Fees, funding, realized PnL, slippage, and remaining exposure are recorded explicitly.

The reset is not a claim that all past information should be discarded. Long-lived causal filters and normalization state may persist under a separate, versioned policy. What resets is the *trade-specific adaptive state*. The model should preserve useful historical statistics without letting one outcome directly rewrite the next trade's weights.

### 8.1 Offline learning, not emotional online updates

A completed trade outcome is a noisy sample. Updating live weights aggressively after a win or loss creates high variance, feedback loops, and selection bias. Store outcomes for offline calibration. Compare fresh reset, carry-over, and informed-prior variants using identical walk-forward splits. Only promote a new prior after independent evaluation, review, and a versioned release. No single trade should change production parameters automatically.

## 9. Risk is a separate control plane

A directional model cannot be the sole authority over money. The risk gate must be independently implemented and capable of rejecting every signal.

For equity (E), allowed risk fraction (r), and stop distance (Delta P), a simplified quantity bound is:

[
qle rac{rE}{Delta Pcdot V},
]

where (V) converts price movement into account-currency PnL per unit. Actual sizing must account for contract multipliers, fees, slippage, funding, lot size, minimum notional, and liquidation constraints. A reasonable research starting policy is 0.25% equity risk per trade and leverage capped at 2x, but these are not proof of safety and must not be increased automatically.

Required independent controls include:
- Maximum notional, leverage, open positions, and correlated exposure.
- Exchange-side protective stop, with confirmation it exists.
- Daily and weekly loss limits and drawdown circuit breaker.
- Kill switch that blocks new orders and follows a documented flatten/halt policy.
- Stale-feed and abnormal-spread rejection.
- Slippage bounds, rate limits, order timeout/cancel logic, and duplicate-order idempotency.
- Partial-fill handling and reconciliation after timeout or process restart.
- Secrets management, withdrawal-disabled API keys, IP restrictions, and no secret logging.
- Manual approval before any real-money execution is enabled.

The current repository is not ready for unattended live trading. The existing gated exchange-adapter source is isolated from the decision path, and no live order route is registered. Keep it that way until the risk and execution lifecycle has been implemented and independently tested.

## 10. Backtesting as a falsification process

A backtest is not a marketing chart. It is a test designed to make the strategy fail if its hypotheses are false.

The backtester must replay timestamped events in causal order and use the same feature, filter, vector, and decision implementation as paper/live modes. Execution simulation must include fees, spread crossing, slippage, funding, latency, partial fills, market impact assumptions, and an additional +50% cost stress.

Minimum comparisons:
1. No-trade / cash baseline.
2. Buy-and-hold over the same period.
3. Simple linear or logistic baseline.
4. Qoin without attention or interactions.
5. Qoin with the proposed components ablated one at a time.
6. Fresh episode reset versus carry-over versus informed prior.
7. Signal without stability gate versus with stability gate.

Minimum metrics:
- Net expectancy per trade and per unit of risk.
- Profit factor and trade count.
- Sharpe and Sortino, with the sampling assumptions stated.
- Maximum drawdown and time under water.
- Exposure, turnover, holding time, and tail losses.
- Results by symbol, volatility regime, liquidity regime, and direction.
- Bootstrap confidence intervals and sensitivity to costs.
- Calibration and signal coverage, including how often WAIT occurs.

Use chronological walk-forward evaluation. Purge overlapping labels and apply embargo where labels or holding periods can leak across folds. Track every configuration tried. Repeated parameter search inflates apparent Sharpe; use a deflated-Sharpe or other multiple-testing adjustment. The final test set should be touched once after choices are frozen. All transformations must be fit on training history only.

A strategy is not validated merely because a parameter set has a positive historical return. Evidence should be stable across periods and regimes, after costs, and should outperform simpler baselines with uncertainty reported.

## 11. Philosophy: humility, attention, and the right to wait

### 11.1 The market as an open, adaptive system

Markets are not closed physical systems with fixed laws. Participants adapt to each other, liquidity changes, venues differ, and the act of trading changes the environment through impact. The “force field” is therefore a model of relationships among observations, not a literal ontology of price. Its value is pragmatic: it can organize evidence and generate falsifiable hypotheses.

### 11.2 Present-centered does not mean memoryless

A present-centered model prioritizes current evidence, but a single observation cannot define context. Filtering, volatility estimates, and causal baselines require memory. The design question is not “past or present?” but “which past information remains useful, how is it allowed to enter, and can it leak future information?” Persistent statistics should be slow, explicit, and versioned; trade-specific adaptation should be temporary and resettable.

### 11.3 Pareto as discipline, not dogma

The Pareto principle motivates attention to a small number of dominant, reliable forces. It does not guarantee an 80/20 law in financial data. The frontier may change with regime, and a quiet force can become important during a crisis. Therefore, top-k selection should be dynamic, uncertainty-aware, and evaluated for stability. Excluded features should remain logged so that selection failures can be diagnosed.

### 11.4 Geometry as a language for relationships

Angles express directional alignment; magnitude expresses combined strength under a chosen scaling; velocity expresses change; acceleration expresses change in that rate. These concepts are useful only if units, normalization, and time intervals are coherent. Geometric elegance does not validate a model. Every geometric construct must earn its place through ablation and out-of-sample evidence.

### 11.5 WAIT is an affirmative decision

WAIT means the available evidence does not justify risking capital under current costs and constraints. It can be the rational output when forces conflict, data quality degrades, the market is too expensive to enter, or a model is outside its validated regime. A system that treats inaction as a legitimate state is less likely to confuse activity with intelligence.

### 11.6 Survival precedes optimization

The first objective is not maximum backtest return; it is avoiding ruin, uncontrolled exposure, operational ambiguity, and hidden failure. Optimization comes after reliable data, reproducible decisions, honest validation, and an independent risk layer. If a model cannot explain what it observed, what it ignored, which version produced the signal, and why an order was allowed, it is not ready to control real funds.

## 12. Implementation map: concept versus current code

| Component | Research target | Current snapshot status |
|---|---|---|
| Event ingestion and dedupe | Validated, idempotent, timestamp-aware pipeline | Basic Axum ingestion and dedupe exist |
| State persistence | Durable state plus deterministic restart replay | Events and snapshots are written; full replay is unfinished |
| Normalization | Past-only robust rolling normalization | Current baseline uses hand-chosen fixed scales |
| Filtering and derivatives | Causal filters before velocity/acceleration | Research-only Python helper exists; not wired to Rust signal |
| Attention | Versioned, tested interaction mechanism | Deterministic hand-specified baseline; not trained |
| Pareto top-k | Reliability-aware frontier and stable selection | Research-only helper exists; not wired to Rust signal |
| Pairwise interactions | Regularized interactions selected by validation | Research-only helper exists; not wired to Rust signal |
| Config versioning | Runtime-loaded config hash on every decision | Example JSON exists; runtime does not load it yet |
| Episode lifecycle | Fill-aware reset and persisted outcomes | Not fully implemented |
| Backtesting | Event-driven, same production signal code | Not yet demonstrated |
| Risk/execution | Independent gate, stops, reconciliation, breakers | Incomplete; live order path intentionally not wired |
| Performance evidence | Walk-forward, cost stress, uncertainty, baselines | No valid profitability result established |

This distinction is essential. The paper is a mathematical and philosophical specification plus a map for future implementation. It does not mean every equation is already in the runtime.

## 13. Recommended development sequence

**Phase A — correctness and reproducibility**
1. Pin Rust toolchain and dependencies; retain a lockfile.
2. Validate event contracts and timestamp policies.
3. Make state replay deterministic and test restart behavior.
4. Introduce a runtime-loaded, versioned configuration with a content hash.
5. Persist every decision with raw feature values, normalized values, gates, reason, and config version.

**Phase B — causal signal research**
1. Add past-only robust normalization and causal filters.
2. Add feature reliability and missingness handling.
3. Compare linear, attention, interaction, and Pareto variants by ablation.
4. Keep derivatives and curvature disabled until evidence supports them.
5. Add stability/hysteresis gates and measure the latency trade-off.

**Phase C — evaluation**
1. Implement event-driven replay using the exact signal code.
2. Add realistic execution costs and +50% stress.
3. Add chronological walk-forward splits, purge/embargo, bootstrap intervals, and multiple-testing correction.
4. Log every experiment and compare against no-trade, linear/logistic, and buy-and-hold.
5. Reject the model if improvements are not robust after costs.

**Phase D — operational safety**
1. Build a separate risk gate and order lifecycle.
2. Implement protective stops, idempotency, partial fills, reconciliation, and recovery.
3. Add circuit breakers and a tested kill switch.
4. Run paper and shadow modes; then testnet only after independent review.
5. Keep live execution disabled until explicit human approval.

## 14. Conclusion

Qoin's thesis is that market decisions can be made more disciplined by representing current market evidence as interacting forces, prioritizing reliable contributors, measuring direction and strength geometrically, and allowing the system to abstain. Its design combines present-centered attention with carefully controlled memory, fresh trade-specific adaptive state, and offline learning.

The framework succeeds only if its parts survive falsification. Attention is not explanation; vector magnitude is not probability; a profitable backtest is not proof; and a signal is not permission to trade. Causal data, versioned parameters, realistic costs, simple baselines, risk separation, and honest uncertainty are not secondary engineering chores. They are the conditions under which the idea can be evaluated at all.

**Current status:** research foundation. No verified profitability. No authorization for real-money execution.

---

## Appendix A — Example decision contract

A production decision record should include:
- event-time cutoff and source freshness;
- symbol, venue, and event sequence watermark;
- raw and normalized feature values;
- feature reliability and missingness flags;
- filter state and normalization version;
- attention, interaction, and Pareto-selection outputs;
- bullish score, bearish score, magnitude, angle, and stability;
- every gate outcome and a human-readable abstention reason;
- model/config version and content hash;
- risk decision, if requested from the separate risk service;
- idempotency key and lifecycle correlation IDs.

## Appendix B — Core equations at a glance

1. Normalized force: (z_{j,t}=(x_{j,t}-m_{j,t^-})/(s_{j,t^-}+epsilon)).
2. Causal EWMA: (ar x_t=alpha x_t+(1-alpha)ar x_{t-1}).
3. Velocity: (v_t=(ar x_t-ar x_{t-1})/Delta t).
4. Acceleration: (a_t=(v_t-v_{t-1})/Delta t_{mid}).
5. Attention: (A=operatorname{softmax}(QK^T/sqrt{d_k})).
6. Regularized interaction: (	ilde g_{ij}=f_if_j/(1+ho|f_if_j|)).
7. Directional magnitude: (M=sqrt{B^2+S^2}).
8. Angular gate: BUY if (	hetale	heta_B); SELL if (pi/2-	hetale	heta_S), subject to all other gates.
9. Risk-sized quantity: (qle rE/(Delta Pcdot V)), with contract-specific adjustments.
10. Pareto dominance: force (j) dominates (i) when (m_jge m_i) and (r_jge r_i), with at least one strict inequality.
