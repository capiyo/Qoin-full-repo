# Version 2 — Set theory, measure and relation ledger

**Status:** finite-set projection, append-only MongoDB observation/relation records, and authenticated resumable historical backfill implemented; automated scheduled integrity monitoring, correction/supersession writes, generalized measure algebra and state transitions remain research work.

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

The current V2 read projection implements two finite measures over each returned set: the counting measure (member cardinality) and the sum of observed trade sizes. Trade-size values retain instrument-native quantity units and are not comparable across different instruments or contracts without a separately defined conversion. Missing sizes are counted explicitly; they are not imputed as zero. The runtime does not yet expose a general-purpose user-defined measure algebra.

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

### Durable backfill operation

New validated events append their observation and eligible temporal-relation records during ingestion. Existing raw events are migrated with the bounded, resumable gateway operation:

- `GET /api/v1/set-ledger/backfill?symbol=BTCUSDT&batch_size=100`
- Supply the operator token in the `x-qoin-execution-token` header. The request is rejected when the token is missing or invalid.
- The response returns `next_cursor` (`after_ts_ms`, `after_event_id`, `after_venue`) and `has_more`. Repeat with those cursor values until `has_more` is false.
- Batch size is bounded to 1–250. The source scan is ordered by event time, event ID and venue. A compound index supports this keyset order.
- Re-running a page is safe: observation and relation record IDs are deterministic and MongoDB rejects duplicate IDs without rewriting prior records.
- The response reports source-event count, persisted observation count and a lower bound on remaining observations. Invalid legacy rows are counted and skipped, not silently coerced.

An authenticated audit is available at `GET /api/v1/set-ledger/integrity?symbol=BTCUSDT&limit=1000`. It compares persisted observation hashes with canonical hashes recomputed from raw source events, checks that relation endpoints resolve to observation records, and reports whether both bounded scans are complete. A partial scan is never reported as a full integrity pass.

The operation is intentionally explicit rather than an automatic startup migration. It does not alter source events, V1 state, decisions or orders. Backfill completion for a symbol is indicated only when the cursor is exhausted and the persisted observation count catches up with validated source events; relation coverage can still be incomplete where the 5,000-neighbour bound was exceeded, which is recorded on each derived record.
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
- schema migration and append-only correction behavior (superseding records are specified but correction writes are not yet exposed);
- ledger integrity checks and reproducible reconstruction from source events;
- shadow-mode comparisons against V1 using chronological out-of-sample windows;
- no execution authority in V2 until separately specified, tested and approved.

## Explicit non-goals

V2 is not a neural embedding, a learned latent predictor, a replacement for V1, an order router, or a claim that set theory alone creates a trading advantage. The current repository implements V1 vector mechanics and an initial V2 finite-set projection over persisted raw events. New validated events append immutable observation records and temporal-relation edge records to the MongoDB `set_relation_ledger` collection. Deterministic record IDs make duplicate delivery idempotent; the ingestion path retries ledger persistence when a raw event is redelivered after an interrupted append. The read endpoint returns both the derived finite-set projection and persisted ledger records. Existing raw-event history can be migrated with the authenticated, resumable, bounded backfill endpoint. Backfill completion is operator-driven and must be checked per symbol; no correction/supersession API is exposed yet. Integrity auditing is an on-demand bounded check, not scheduled monitoring. V2 remains read-only and never drives V1 decisions or orders. This document defines the remaining V2 research direction.
