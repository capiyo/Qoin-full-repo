# Shared market vector space

**Status:** Implemented schema and version adapters; research/paper-only. No profitability or production-readiness claim.

## One market state, two independent algorithms

`qoin-market-space` is a version-independent Rust crate consumed by both Qoin algorithm crates. It defines schema v1 as a fixed 53-coordinate state vector. The vector is the common input contract, not a third algorithm and not a learned embedding.

Each snapshot carries a stable vector ID and schema version; venue, symbol and timeframe; an event-time cutoff and the time the snapshot became available; all 53 coordinates in a stable enum order; and an optional normalized value plus an independent data-quality score for every coordinate.

Every present coordinate must be finite and normalized to [-1, 1]. Missing values remain explicit `None` with quality zero; they are never silently imputed as zero. Each coordinate also distinguishes `observed`, `missing` (applicable but unavailable) and `not_applicable` (the feature does not apply to this instrument/provider). This prevents an absent options chain, for example, from being treated as an adverse options signal. Calibration and normalization are the caller's responsibility and must use past-only data. A snapshot is rejected if its availability timestamp precedes its event-time cutoff.

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

`EvidenceSet::from_market_vector` converts the exact same schema into explicit condition atoms using a versioned, caller-configurable binning policy: positive, negative or neutral for sufficiently reliable observed coordinates; low-quality for observed values below the quality threshold; missing for applicable-but-unavailable coordinates; and not-applicable for dimensions outside the instrument/provider's scope. The latter two can be independently included or excluded by the evidence policy.

The continuous normalized values remain in the common vector; V2's set conversion is a deliberate symbolic abstraction, not a replacement for the source vector. Evidence sets and relationship records retain the complete source vector snapshot, including coordinate status, normalized value and quality, plus vector ID, schema version, venue, symbol and timeframe. The append-only relationship ledger therefore remains replayable without depending on a mutable external vector cache.

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


## Current integration boundary

The raw-event ingestion path now constructs and persists immutable shared market vectors from the features supported by the current trade, book, funding, open-interest and liquidation event schema. It uses past-only robust calibration, feature-specific freshness and quality, and distinct source-event timestamps so a book/funding update does not get counted as a new price observation. The vector is included in the ingestion response and `market.update` websocket message.

Every snapshot contains all 53 coordinates. Current per-symbol ingestion explicitly marks option-chain-only dimensions and portfolio-level dimensions as `not_applicable`; spot venues also mark basis as `not_applicable`. Perpetual basis remains `missing` until a paired mark/index feed is connected. Cross-asset correlation/stress and regime uncertainty remain `missing` until aligned multi-instrument producers and a validated regime-uncertainty estimator are connected. They are not replaced with neutral zeros.

The actual V1 vector engine is now invoked for each ingested vector. Its complete decision and exact Shapley attribution are persisted with the decision record and returned in `v1_shared_market_vector`. If a required applicable force or regime coordinate is unavailable, V1 emits `status: not_ready` with the exact feature/status/quality gaps and abstains. A structurally not-applicable basis coordinate is neutral and excluded from the coverage denominator. The legacy `POST /v1/signal` ForceObservation interface and existing EdgeStrategy diagnostic path remain available for compatibility; they are not a substitute for the V1 shared-vector report.

V2 can now discover relationships directly from stored vector history through `POST /internal/v2/relationships/discover-history?venue=...&symbol=...&limit=...`. The endpoint selects the latest immutable snapshot and earlier candidate snapshots, converts them into evidence sets, computes deterministic set attention and appends provenance-complete relationship records to the ledger. Repeating discovery for the same vector window is idempotent. The caller-supplied `POST /internal/v2/relationships/discover-vectors` interface remains available for controlled research comparisons.

The Drogon gateway now forwards read-only client routes for `GET /api/v1/market-vector`, `GET /api/v1/market-vector/history`, `GET /api/v1/variables/progress`, and `GET /api/v1/v2/relationships`. The V1 attribution report is also available through the existing `GET /api/v1/vector` decision response and the `market.update` websocket message. V2 history discovery and ledger mutation remain internal-only until server-side identity verification, authorization and write-rate controls are in place.

The ingestion feature producer is still not a complete 53-coordinate market-data system. Cross-asset alignment, macro-event surprise/proximity, option-chain Greeks/implied volatility, paired derivative basis, portfolio exposures/risk, and a validated multivariate regime uncertainty/change-point pipeline require additional licensed feeds or explicit portfolio/temporal inputs. These coordinates remain missing or not-applicable according to scope. Consequently, the automatically invoked V1 engine will correctly abstain on current event streams until all required applicable dimensions are genuinely observed. All endpoints remain research-only and do not submit orders or enable real-money execution.


## Shared variable hub and progression

The schema is the shared vocabulary; the **variable hub** is the lifecycle manager that makes it operational. A hub instance is scoped to one `venue + symbol + timeframe`. It is not a global pool that mixes unrelated instruments. Cross-asset and multi-timeframe producers align their inputs first, then publish their resulting coordinates into the relevant instrument hub.

### The progression

```text
Canonical events / external aligned feeds / portfolio state
                         |
                         v
                INPUT VARIABLES
          observed raw values + provenance
                         |
                         v
             PAST-ONLY NORMALIZATION
       center/scale/profile version + fit cutoff
                         |
                         v
                DERIVED VARIABLES
          dependency graph + captured revisions
                         |
                         v
                 REGIME VARIABLES
      structural state computed from ordinary features
                         |
                         v
          PORTFOLIO / TEMPORAL VARIABLES
                         |
                         v
              IMMUTABLE MARKET VECTOR
             schema v1 + quality + timestamps
                    /           \
                   v             v
                  V1             V2
          numerical forces   evidence sets,
          and risk overlay   attention + ledger
                    \           /
                     v         v
                  SHARED RISK GATE
```

The dependency graph is a directed acyclic graph, not an arbitrary collection of formulas. For example:

- `LogReturn -> RealizedVolatility -> VolatilityOfVolatility`
- `LogReturn -> Momentum -> TrendStrength -> RegimeTrendStrength`
- `SpreadBps + OrderBookDepth + OrderFlowImbalance -> MarketImpact -> SlippageEstimate`
- `Correlation + Covariance + Beta -> CorrelationBreakdown -> RegimeCorrelationStress`
- `RealizedVolatility + ReturnKurtosis + JumpIntensity -> RegimeVolatilityLevel`
- `RegimeTrendStrength + RegimeVolatilityLevel + RegimeLiquidityStress + RegimeCorrelationStress + RegimeTransitionIntensity -> RegimePersistence`

The Rust hub stores each variable's raw value, normalized value, quality, event/availability times, producer and calculation version, normalization profile, revision, and the exact revisions of its dependencies. If an upstream input changes, downstream values become **stale** automatically and cannot be published until recalculated.

### Lifecycle states

| State | Meaning | Next transition |
|---|---|---|
| Awaiting input | Input producer has not supplied the variable | Observed |
| Blocked | A derived variable lacks one or more ready dependencies | Dependencies become ready |
| Observed | Raw input received with provenance | Normalized |
| Derived | Derived raw value calculated from fresh dependencies | Normalized |
| Normalized | Past-only calibration mapped the value to [-1, 1] | Published |
| Published | This exact revision was included in an immutable vector snapshot | New revision or stale |
| Stale | An upstream dependency changed after this value was calculated | Recompute |
| Not applicable | Dimension is structurally outside the instrument/provider scope | Remains explicit in the vector |

A variable is never treated as zero merely because it is missing. A variable cannot advance by skipping a dependency. A derived value records the dependency revisions used to calculate it. A normalizer must carry a fit cutoff no later than the observation's event time. The hub rejects future calibration and out-of-order derived availability.

### Ownership and sharing

- The hub owns the canonical feature definitions, dependency graph, provenance, revision counters, freshness checks, normalization metadata and snapshot publication.
- Feature producers own the mathematics for their variables: event aggregation, rolling statistics, order-book impact, cross-asset alignment, change-point/regime estimation, portfolio risk and temporal persistence.
- V1 owns its numerical decision logic; V2 owns set conversion, attention and relationship discovery. Neither owns or silently recalculates the shared input features.
- The risk gate consumes the published state and constrains both algorithms. It does not rewrite the market vector.
- The V2 ledger keeps the exact source snapshots used in its relationship records; V1 results should likewise reference the immutable vector ID and schema version.

### Important boundaries

The hub now implements the shared dependency registry, lifecycle tracking, revision invalidation, causal normalization checks, topological graph validation and immutable vector publication. It does not invent feature values or claim that every producer is already connected to exchange ingestion. Provider-specific and statistical feature calculators still need to be implemented and wired to call the hub. A feature that is not ready remains blocked/missing rather than fabricated.


## Contribution percentages: vectors, sets and measures

Contribution attribution is a shared mathematical service, not a V1-only or V2-only feature. The same report contract can be attached to an intermediate variable, a V1 score, a V2 evidence relation, or the final continuous decision score.

### What the percentages mean

- **Variable contribution:** the signed effect of one input on a particular derived variable or score. The report shows positive/negative direction, absolute contribution share, quality and coverage.
- **Set contribution:** the share of measured evidence mass allocated to each evidence set. Atomic measure prevents overlap from being counted more than once.
- **Final outcome contribution:** how much each variable, set relation, regime factor or risk overlay contributed to the final continuous score. Additive models use their exact signed effects; nonlinear interactions use exact Shapley attribution from coalition evaluations.

For an additive score, each effect is (e_i), the net effect is (sum_i e_i), and the displayed contribution percentage is (100|e_i| / sum_j |e_j|). Percentages therefore sum to 100% when at least one effect is non-zero, while signed effects still sum to the net score change. Positive and negative contributors are shown separately; opposing forces are not hidden by netting them first.

For vector projection, the effect is coefficient × normalized coordinate × data quality. Missing applicable coordinates reduce coverage; not-applicable coordinates are excluded from the applicable-weight denominator. Quality is visible in the report and explicitly participates in the vector effect, rather than being silently mistaken for a missing value.

### Set theory and measure

V2's sets can overlap. Simply adding each set's measure would double-count shared atoms. The atomic measure service calculates the measure of the union and allocates each atom's mass equally across the sets containing it. For this union-coverage game, that is the Shapley allocation: all set allocations add back to the union measure, and their percentages add to 100%.

This is a measure of evidence coverage, not automatically a probability that a trade succeeds. A probability requires a separately calibrated statistical model and out-of-sample calibration tests.

### Nonlinear final outcomes

When vector, set, regime and risk factors interact nonlinearly, fixed weights are not a truthful explanation. Qoin can evaluate the final score for each coalition of components and calculate exact Shapley values. The current exact implementation is bounded to 12 components per attribution calculation. Larger models should use a documented sampled/permutation approximation with error bounds rather than pretending exactness.

### Percentage display rules

1. Show signed effect and direction alongside percentage share.
2. Show coverage and data quality beside percentages.
3. Show variable-level attribution first, family/set-level aggregation second, and final-outcome attribution last.
4. Keep interaction effects attributable; do not double-count the same evidence in vector and set summaries.
5. Percentages describe contribution to the reported score or measure, not causal certainty, confidence, or win probability.
6. A final BUY/SELL/WAIT label is a thresholded decision. Attribute the underlying continuous score and risk-adjusted score, and show the threshold separately.

The shared Rust crate supplies vector-projection attribution, overlap-aware atomic set-measure attribution and exact Shapley attribution. V1's shared-vector signal endpoint now returns coordinate-level and family-level Shapley attribution for its risk-adjusted decision margin. V2's vector relationship discovery endpoint returns each persisted relationship with its pairwise set-measure allocation. The TypeScript package contains matching report contracts; frontend rendering is the next phase.


### Per-variable dependency attribution

A derived variable can be recorded with a reconciled attribution report. Each contribution must map to one of that variable's direct dependencies (multiple distinct pathways may map to the same dependency), include a unique pathway key, signed effect and optional quality, and the effects must sum to the derived raw value relative to the stated baseline. Absolute-effect percentages must reconcile to 100% (or 0% when every effect is zero). The hub stores this breakdown beside the variable's revision and dependency revisions and exposes it through variable progress. If an upstream dependency changes, the variable and its attached attribution become stale together.

Feature calculators must provide defensible dependency effects or coalition-based Shapley allocations. The hub does not invent percentages from equal weights. Raw input variables may have no dependency attribution; their own source/provenance and quality remain visible.


## Backend attribution contract (V1 and V2)

V1's shared-vector decision quality-weights its seven force coordinates and five regime coordinates before evaluation. The returned decision includes an exact 12-component Shapley report over the *risk-adjusted decision margin* (absolute directional score minus the regime-adjusted threshold), plus canonical aggregation by the ten feature families. Each coalition re-runs clipping, Pareto top-k selection, the OFI × open-interest interaction, volatility dampening and the regime threshold. Nonlinear interactions are allocated rather than hidden in a linear approximation. Percentages are shares of absolute Shapley effects; signed effects and positive/negative directions remain visible. Coverage is the mean quality of the twelve required inputs.

V2 relationship records now carry a pairwise atomic set-measure report. Every evidence atom has unit mass; an atom shared by both sets is split across memberships, so the union is counted once. The report is validated again when a relationship ledger event is appended or read. The vector-discovery endpoint returns the relationship records, including each relationship's set measure and attention weight, instead of only returning aggregate write counts.

These are research explanations of model score/evidence structure, not causal probabilities, win probabilities, or expected returns. V1 remains the only directional signal engine; V2 discovers and logs relationships and does not submit orders.


## Backend ingestion and variable-hub API

The Axum event pipeline now maintains isolated state per **venue + symbol**, preventing two exchanges that use the same symbol string from contaminating one another. It creates immutable `event-window` market-vector snapshots from the canonical trade, book, funding, open-interest and liquidation events. Raw rolling features include returns, displacement, rate of change, swing position, realized volatility, volatility-of-volatility, skew, kurtosis, jump intensity, momentum, trend strength/persistence, mean reversion, spread, depth, order-flow imbalance, volume delta, impact/slippage estimates, funding pressure, open-interest change, liquidation pressure, event intensity and selected regime/temporal coordinates.

Each raw feature is normalized using a rolling median and MAD computed from **prior observations only**. The current sample is appended only after its normalization is calculated. At least eight prior samples and a non-degenerate historical scale are required; otherwise the coordinate remains Missing. Unsupported dimensions are not fabricated. In particular, cross-asset correlation stress, regime uncertainty, basis, options Greeks/IV, macro surprise and portfolio variables remain Missing until their specialist producers supply them. V1 continues to fail closed when its required coordinates are unavailable.

Available routes:

- `GET /internal/market-vector?symbol=BTCUSDT&venue=binance_usdm` — latest immutable vector
- `GET /internal/market-vector/history?symbol=BTCUSDT&venue=binance_usdm&limit=120` — vector history
- `POST /internal/v1/market-vector/signal` — V1 decision plus exact Shapley/family attribution
- `POST /internal/v2/relationships/discover-vectors` — V2 evidence-set discovery and append-only ledger records

The persistent variable hub is exposed separately for specialist feature producers and derived calculators:

- `POST /internal/variables/observe` — register a raw input and provenance
- `POST /internal/variables/normalize` — apply a past-only normalization profile
- `POST /internal/variables/derive` — record a derived variable after all dependencies are current; optional attribution is validated
- `POST /internal/variables/not-applicable` — explicitly mark a structurally inapplicable dimension
- `GET /internal/variables/progress?venue=...&symbol=...&timeframe=...` — return all 53 lifecycle states and dependency blockers
- `POST /internal/variables/publish` — freeze fresh normalized variables into a versioned MarketVector

Hub snapshots and published vectors are persisted in MongoDB. Derived values retain dependency revisions, and upstream changes make dependent values Stale. The auto-generated event-window vector pipeline and the specialist variable-hub path are intentionally separate producer paths that converge on the same MarketVector schema.

All routes above are internal research interfaces. This does not activate real-money order submission. Live execution remains behind the existing execution token and kill switch, and no deployment or merge is performed by this work.


The backend also applies a wall-clock guard at event ingestion and at V1/V2 decision, variable-hub publication, and ledger-write boundaries. Future-dated event, availability, observation, and ledger timestamps are rejected; historical replay remains valid. This complements past-only normalization and stale-feature expiry, so an otherwise well-formed vector cannot introduce data that was not yet available.
