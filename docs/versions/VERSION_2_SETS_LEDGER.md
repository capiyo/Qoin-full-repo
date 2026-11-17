# Version 2 — Set theory, measure and relation ledger

**Status:** initial read-only finite-set / temporal-relation projection implemented; durable append-only ledger, generalized measure layer and state transitions remain research work.

## Purpose

Version 2 extends, rather than replaces, the deterministic Version 1 vector engine. V1 remains the active calculation and decision baseline. V2 explores whether market context is better explained as structured collections of observations and explicit relations than as a single vector score.

The frontend must not ask ordinary users to inspect mathematical sets. The engine performs the construction and presents the result as a human-readable ledger: what was observed, which sets were formed, which relations held, what measure was assigned, and why a transition occurred.

## Mathematical objects

Let a bounded research window be represented by a finite observation universe

\[
\Omega_t = \{o_i : t-W < \tau(o_i) \leq t\},
\]

where each observation retains its instrument, venue, source event ID, event time, availability time, and data-quality metadata. The event-time and availability-time distinction prevents future information from leaking into a historical decision.

A measurable space is formed as \((\Omega_t,\mathcal F_t,\mu_t)\). For the finite runtime representation, \(\mathcal F_t\) is an explicitly defined collection of subsets of \(\Omega_t\), closed under the operations required by the selected analysis. The measure \(\mu_t\) must state its semantics and units; examples include normalized event mass, traded-volume mass, elapsed-time mass, or a documented confidence weighting. These measures are not interchangeable and must never be silently combined.

Feature predicates construct typed subsets, for example:

- \(S_{\text{buy-flow}}\): observations satisfying the configured positive order-flow condition.
- \(S_{\text{liquidity-stress}}\): observations satisfying the spread/depth stress condition.
- \(S_{\text{volatility-shock}}\): observations satisfying the volatility regime condition.
- \(S_{\text{liquidation}}\): observations carrying validated liquidation evidence.

Thresholds, units, data availability and predicate versions are part of the set definition. Missing data is represented as unknown, not silently coerced to false or zero.

Relations are explicit typed subsets of a product space, \(R_k \subseteq A\times B\). Examples include temporal succession, same-venue corroboration, cross-venue agreement, contradiction, containment, and conditional co-occurrence. A relation must carry its directionality, time window, source constraints, and evaluation rule. Mere co-occurrence is not causation.

A transition maps a prior state to a new state only when its declared preconditions are satisfied. Transition rules must be deterministic for a fixed input snapshot and configuration version. The V2 layer must be able to abstain when evidence is incomplete, stale, contradictory, or outside the declared domain.

## Relation ledger

The ledger is an append-only sequence of immutable records. A record should include:

- stable record ID and schema version;
- instrument, venue scope, event-time interval and ingestion-time watermark;
- source event IDs and hashes/references to the raw evidence;
- set identifiers, predicate versions and membership outcomes;
- relation identifiers, direction, evaluated endpoints and outcome;
- measure name, unit, value, normalization rule and support size;
- transition rule/version, previous state, resulting state and reason;
- links to the corresponding V1 vector decision and strategy version, where available;
- data-quality flags, missing inputs and explicit abstention reasons.

Corrections are represented by superseding records that reference the prior record; historical entries are not silently overwritten. The ledger must distinguish a source fact from a derived membership, relation, measure, and interpretation.

## V1 integration boundary

V1 continues to produce its existing feature vector, attention/contribution trace, two-component decision vector, gates and BUY/SELL/WAIT research action. V2 initially runs in shadow/research mode and records its own set and relation outputs beside the matching V1 decision. It must not alter V1 weights, thresholds, risk controls, backtest fills, or execution behavior.

Any later proposal to combine V1 and V2 outputs requires chronological out-of-sample evaluation, realistic fees/slippage/latency, sensitivity analysis, stability tests, and explicit comparison against V1-only baselines. A V2 representation or ledger is not evidence of predictive edge or profitability.

## User experience

The user-facing surface is a compact explanation, not a set-theory workbench. It should show:

1. **Evidence:** source events and data-quality status.
2. **Conditions:** the predicates that were satisfied or unknown.
3. **Relations:** corroboration, contradiction and temporal links.
4. **Measure:** named quantity, units, support and normalization.
5. **Transition:** previous state, new state and the exact rule that fired.
6. **Audit trail:** links between ledger records and the V1 decision.

Users can expand records for detail, but do not need to manipulate sets or understand sigma-algebra notation to use Qoin.

## Required validation before implementation is promoted

- causal event-time/availability-time handling and deterministic replay;
- property tests for set membership, relation composition and transition invariants;
- measure conservation/normalization checks appropriate to each measure's declared semantics;
- duplicate, late, missing, reordered and contradictory event tests;
- schema migration and append-only correction behavior;
- ledger integrity checks and reproducible reconstruction from source events;
- shadow-mode comparisons against V1 using chronological out-of-sample windows;
- no execution authority in V2 until separately specified, tested and approved.

## Explicit non-goals

V2 is not a neural embedding, a learned latent predictor, a replacement for V1, an order router, or a claim that set theory alone creates a trading advantage. The current repository implements V1 vector mechanics and an initial V2 finite-set projection over persisted raw events. The V2 endpoint is read-only and computes its output on request; it does not persist ledger entries or drive decisions. This document defines the remaining V2 research direction.
