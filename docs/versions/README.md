# Qoin version map

Qoin maintains isolated algorithm generations with a shared application, data and safety foundation.

## Version 1 — Vector baseline

Branch: [`version-1-vector-baseline`](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)

The deterministic vector engine remains the control/baseline: frontend clients, Firebase client account/profile/Archive foundation, Drogon gateway, Axum-authoritative event and state processing, MongoDB persistence, provider adapters, replay and existing risk boundaries. See [Version 1 specification](./VERSION_1.md).

## Version 2 — Set-theoretic attention

Branch: [`version-2-set-theoretic-attention`](https://github.com/capiyo/Qoin-full-repo/tree/version-2-set-theoretic-attention)

Carries the complete Version 1 application and keeps its vector engine intact. Adds a separate Rust set-theoretic attention crate with permutation-invariant incidence embeddings, explicit set algebra, deterministic temperature-scaled cosine attention and an append-only MongoDB relationship ledger. See [Version 2 specification](./VERSION_2_SET_THEORETIC_ATTENTION.md).

The legacy JEPA sidecar remains an isolated inherited research artifact; it is not the V2 decision model and does not feed V2 or V1 decisions. The V2 set-attention implementation is currently a deterministic research baseline, not a trained transformer.

## Comparison protocol

Use identical timestamped source observations, chronological splits, feature availability, venue isolation, risk budgets and realistic execution costs. Compare V1 alone, V2 set algebra alone, V2 attention, combined V2, and any V1/V2 ensemble. Report out-of-sample net returns, drawdown, turnover, exposure, calibration, regime stability and uncertainty. Correct for multiple testing and selection bias. Attention weights and set overlap are not probabilities of profitability or causal evidence.

## Safety and product boundaries

- V1 remains unchanged and is the baseline.
- V2 research output cannot mutate V1 strategy configuration or submit orders.
- Both tracks remain research/paper-only; the existing `docs/RISK_POLICY.md` is binding.
- Live trading requires all risk gates to be implemented and independently verified plus explicit written user approval.
- Web, mobile and desktop continue to use Qoin's existing Firebase-based client account/profile/Archive foundation. No Funspot backend/auth is used.
- This branch does not claim backend Firebase ID-token verification.
