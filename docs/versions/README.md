# Qoin version map

Qoin has two explicitly separated architecture tracks:

## Version 1 — Vector baseline

Branch: [`version-1-vector-baseline`](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)

The existing architecture preserved as the baseline: frontend clients, Drogon gateway, Axum-authoritative state/vector decisions, MongoDB persistence, and current research/risk boundaries. See [Version 1 specification](https://github.com/capiyo/Qoin-full-repo/blob/version-1-vector-baseline/docs/versions/VERSION_1.md).

## Version 2 — JEPA world model

Branch: [`version-2-jepa-world-model`](https://github.com/capiyo/Qoin-full-repo/tree/version-2-jepa-world-model)

Retains the Version 1 architecture and adds a JEPA-inspired PyTorch encoder/predictor plus optional loopback-only Axum telemetry. Latent predictions are persisted separately as research observations; they do not replace the vector engine, produce direct orders, or claim a validated trading edge. See the [Version 2 architecture specification](https://github.com/capiyo/Qoin-full-repo/blob/version-2-jepa-world-model/docs/versions/VERSION_2_JEPA.md) and [model scaffold](https://github.com/capiyo/Qoin-full-repo/tree/version-2-jepa-world-model/services/jepa-world-model).

## How to compare

Use the same timestamped events, chronological splits, feature availability, risk budgets, and realistic execution costs. Compare Version 2 with Version 1 out of sample; do not promote based on training loss alone.

## Safety

Both versions remain research/paper-only. The existing `docs/RISK_POLICY.md` remains binding. Live trading requires every gate to be implemented and independently verified plus explicit written user approval.
