# Version 2 — Set-theoretic attention and relational ledger

**Status:** Research architecture. Version 1 remains the production-shaped research baseline and comparison control. Version 2 does not submit orders or promote itself automatically.

## Objective

Version 2 represents market context as explicit sets and relations, selects relevant relations through a versioned attention procedure, and records the complete derivation in an append-only provenance ledger.

The point is not to expose mathematical machinery to end users. Qoin translates the resulting evidence into readable ledgers, explanations, confidence/quality indicators and outcome history.

## Version boundary

- **Version 1 — Vector:** deterministic feature vectors, explicit coefficients, vector scoring, magnitude/direction/persistence checks and existing risk gates. It remains independently executable and reproducible.
- **Version 2 — Sets + ledger:** typed sets of observations and conditions, relations and higher-order relations between them, attention over candidate relations, and a durable ledger of inputs, membership, selected relations, transformations and outputs.
- Both versions consume the same causally valid event stream and comparable instrument/timeframe definitions. Version 2 must never silently mutate Version 1 weights or overwrite Version 1 results.

## Mathematical model

For instrument (s) and decision time (t), define a causally bounded universe:

[
U_{s,t} = {x_i : operatorname{availableAt}(x_i) le t}
]

Each element has a stable identity, source, timestamp, instrument, timeframe, schema version and quality state. Missing, stale, duplicated or invalid observations are represented explicitly; they are not silently converted to zero.

A condition set is a subset of that universe:

[
C_k(t) subseteq U_{s,t}
]

Examples include trend conditions, volatility regimes, liquidity conditions, order-flow conditions, funding/carry conditions, gap/level interactions and event conditions. Membership is determined by versioned predicates with recorded parameters.

Relations connect condition sets or individual evidence items:

[
R_j(t) subseteq C_a(t) 	imes C_b(t)
]

The model may also represent higher-order relations (hyperedges) when a useful confluence requires three or more conditions. Relations must have typed semantics: supports, contradicts, precedes, overlaps, confirms, invalidates or depends-on. A recorded relation is evidence of a defined relationship, not proof of causation.

The attention stage ranks/selects eligible relations under explicit constraints. For a candidate relation (r):

[
a_r(t) = operatorname{Attention}_{v}(r, U_{s,t}, mathcal{R}_{s,t})
]

The attention implementation, score components, thresholds, tie-breaking and selected relation set are versioned. It must be deterministic for a fixed input snapshot and version, or else record every stochastic seed and runtime detail needed for replay. Attention is not permitted to manufacture missing evidence or bypass risk constraints.

The resulting evidence structure is:

[
mathcal{G}_{s,t} = (U_{s,t}, mathcal{C}_{s,t}, mathcal{R}_{s,t}, A_{s,t})
]

where (A_{s,t}) is the selected/weighted relation set. A downstream research evaluator can compare this structure with Version 1's vector output and with realized outcomes.

## Relational ledger

Every Version 2 evaluation writes a durable, append-only ledger record. At minimum it captures:

1. **Identity:** ledger record ID, instrument, venue, timeframe, event-time cutoff and processing time.
2. **Input provenance:** source event IDs, source timestamps, provider/schema versions, data-quality flags and the exact causal snapshot boundary.
3. **Set construction:** set IDs, predicate IDs/versions, member evidence IDs, exclusions and parameter/configuration hashes.
4. **Relations:** relation IDs/types, endpoint or hyperedge members, rule/version, supporting evidence and contradiction evidence.
5. **Attention:** candidate relation IDs, score components, eligibility decisions, selected set, thresholds and deterministic tie-break result.
6. **Evaluation:** Version 1 comparison reference, Version 2 research output, uncertainty/quality status, risk-gate result and explicit WAIT/abstain reason where applicable.
7. **Reproducibility:** source commit, algorithm version, schema version, configuration hash, runtime/build metadata and any random seed.
8. **Outcome:** later observation window, realized outcome, fees/spread/slippage/funding assumptions and evaluation status. Outcome records reference the original decision; they do not rewrite it.

Corrections are new superseding records with a reason and link to the prior record. Do not mutate or silently delete historical decision evidence. The ledger should support replay by record ID and integrity checks (for example, chained record hashes); a hash chain improves tamper evidence but does not by itself prove the source data was true.

## Evaluation and promotion gates

- Use identical instruments, timestamps, event availability, chronological train/validation/test windows and risk budgets for Version 1 and Version 2.
- Enforce point-in-time membership: no set, relation, feature or label may use information unavailable at the decision cutoff.
- Test relation stability, duplicate handling, missing data, contradictory evidence, sparse regimes and out-of-order events.
- Compare against Version 1 and simple baselines; report coverage, abstention, calibration, turnover, drawdown and cost-adjusted outcomes, not just hit rate or training loss.
- Include fees, spread, slippage, funding, latency, partial fills and stressed execution assumptions in any trading-oriented evaluation.
- Require out-of-sample and walk-forward evidence plus independent review before any promotion. No automatic promotion or live order path is introduced here.

## User-facing presentation

Users should not be required to understand set notation or inspect raw relation graphs. Present:
- a chronological evidence ledger;
- the conditions that were present;
- relations that supported or contradicted the setup;
- which evidence the attention stage selected and why;
- what was excluded and why;
- the Version 1 comparison;
- the final research state and later outcome.

The ledger is the explanation surface. Graph/set exploration is an optional analyst tool, not a prerequisite for ordinary users.

## Safety

Version 2 remains research/paper-only. Axum and the existing risk policy remain authoritative. The set/ledger layer cannot place orders, alter account balances, bypass limits or turn an unvalidated relation into an executable instruction. Live trading remains disabled until every execution, risk, reconciliation, recovery and kill-switch gate is independently verified and explicitly approved.
