# Qoin version map

Qoin has two explicitly separated architecture tracks:

## Version 1 — Vector baseline

Branch: [`version-1-vector-baseline`](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)

The existing architecture preserved as the baseline: frontend clients, Drogon gateway, Axum-authoritative state/vector decisions, MongoDB persistence, and current research/risk boundaries. See [Version 1 specification](https://github.com/capiyo/Qoin-full-repo/blob/version-1-vector-baseline/docs/versions/VERSION_1.md).

## Version 2 — Sets and relation ledger

Version 2 is the planned extension of the active Version 1 vector engine. It introduces typed sets of observations and states, explicit relations and transitions, measure-aware aggregation, and an append-only explanatory ledger. It is a research design, not a claim that the V2 runtime is complete. See the [Version 2 specification](VERSION_2_SETS_LEDGER.md).

## How to compare

Use the same timestamped events, chronological splits, feature availability, risk budgets, and realistic execution costs. Compare the V2 set/relation outputs against V1 vector decisions out of sample; do not promote on representation quality alone.

## Safety

Both versions remain research/paper-only. The existing `docs/RISK_POLICY.md` remains binding. Live trading requires every gate to be implemented and independently verified plus explicit written user approval.
