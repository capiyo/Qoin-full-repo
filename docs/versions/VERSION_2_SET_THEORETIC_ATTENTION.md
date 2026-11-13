# Version 2 — Set-theoretic attention and relationship ledger

**Branch:** `version-2-set-theoretic-attention`  
**Starting point:** the complete Qoin development baseline at `69f5107b8b90b9b6cd6d2432b1597d079acc9d3c`.  
**Status:** isolated research implementation; no claim of predictive edge.

## Version boundary

Version 2 carries the complete Version 1 application and deterministic vector engine forward unchanged as its comparison baseline. The React web, React Native and Tauri desktop clients, shared contracts, Firebase client authentication/profile/Archive foundation, Axum, Drogon, MongoDB, provider adapters, replay and risk boundaries are inherited from the starting commit.

The V1 vector engine remains available and authoritative for existing decisions. The new `qoin-set-theoretic-attention` crate is separate. V2 research output is not wired to order submission, does not mutate the V1 strategy map, and cannot bypass risk controls.

## V2 research pipeline

1. Construct timestamped evidence sets from causal, source-attributed market observations.
2. Build a stable sorted vocabulary and a permutation-invariant binary incidence embedding.
3. Compute set algebra (intersection, union, difference, symmetric difference, subset and disjointness).
4. Apply deterministic scaled-dot-product attention over set embeddings. This is a transparent, untrained research baseline, not a trained transformer.
5. Emit relationship records with full source/target evidence snapshots, domains, individual observation timestamps, relation type, internally consistent Jaccard/coverage metrics, attention weight, attention temperature and model version.
6. Append discovery, outcome and invalidation events to the relationship ledger. Historical events are immutable; corrections and later outcomes are additional events.

## Relationship ledger

The ledger is an append-only event stream. A relationship is not considered predictive merely because it has a high attention weight or set overlap. Outcome events must be evaluated on a declared forward horizon and preserve realized return, maximum adverse excursion and maximum favorable excursion. Relationship validity and usefulness must be assessed out of sample, with transaction costs, regime segmentation and multiple-testing controls.

The first implementation exposes the ledger only through Axum's internal research API and persists events in MongoDB. Each record preserves both evidence-set snapshots and the attention temperature so its set algebra and attention input can be reconstructed. It uses a unique event ID for idempotency and a time index for chronological inspection. No Firebase ID-token verification is claimed or added; these are internal service routes, not user-facing Archive/profile routes.

## Internal Axum research API

- `POST /internal/v2/relationships/discover` accepts a query evidence set, 1-500 candidate sets, a model version, attention temperature and recording timestamp. Axum builds one stable vocabulary, embeds the sets, calculates normalized attention weights, derives set-relation metrics, and appends one discovery event per candidate.
- `GET /internal/v2/relationships?limit=100` reads the newest ledger events (limit is clamped to 1-500).
- `POST /internal/v2/relationships` appends a validated discovery, outcome or invalidation event. Event IDs are unique; duplicate IDs are rejected as conflicts.
- Discovery event IDs are deterministic hashes of the complete relationship snapshot and recording timestamp, making identical retries idempotent. Requests are bounded to 500 candidate sets, 512 atoms per set and 10,000 total atoms. A partial database outage can leave earlier records from a multi-candidate request persisted; the API reports the persisted count and does not rewrite those events.
- These routes are Axum-internal and are not exposed through the public Drogon gateway. They are not Firebase-authenticated user endpoints.

Example discovery payload:

```json
{
  "query": {
    "id": "gold-5m-trend",
    "domain": "XAUUSD/5m",
    "observed_at_ms": 1800000000000,
    "atoms": ["trend:higher_high", "liquidity:sweep"]
  },
  "candidates": [
    {
      "id": "gold-5m-volatility",
      "domain": "XAUUSD/5m",
      "observed_at_ms": 1800000000000,
      "atoms": ["trend:higher_high", "liquidity:sweep", "volatility:expansion"]
    }
  ],
  "model_version": "set-attention-v2.0.0",
  "temperature": 1.0,
  "recorded_at_ms": 1800000000100
}
```

## Validation plan

- Unit tests for set algebra, permutation invariance, embedding dimension checks, attention normalization, and ledger validation.
- Chronological walk-forward evaluation against V1 using identical market observations, feature availability, risk budgets and conservative execution costs.
- Ablations: V1 only; V2 set algebra only; V2 attention only; combined set algebra + attention; and V1/V2 ensemble.
- Report net returns, drawdown, turnover, exposure, calibration, stability by regime and confidence intervals. Correct for selection bias and multiple hypothesis testing.
- V2 remains research-only until independently validated. It never enables live orders.

## Explicit limitations

- The incidence embedding is a deterministic set representation, not a learned semantic embedding.
- Attention weights are normalized similarity scores, not probabilities of profitable outcomes or causal explanations.
- The ledger records evidence and subsequent outcomes; it does not automatically train or promote a model.
- No independent V2 predictive performance has yet been established.
- The existing Firebase account/profile/Archive implementation is inherited unchanged; V2 does not use Funspot backend/auth.
