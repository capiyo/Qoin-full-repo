# Qoin version map

Qoin maintains two research tracks. Neither track is a claim of profitability, and neither may bypass the platform's risk policy.

## Version 1 — Deterministic vector baseline

Branch: [`version-1-vector-baseline`](https://github.com/capiyo/Qoin-full-repo/tree/version-1-vector-baseline)

The baseline uses causal market observations, deterministic feature construction and vector-based scoring. It is the reference implementation against which later research is compared. Preserve reproducibility by versioning inputs, feature definitions, coefficients, timestamps and outcomes.

## Version 2 — Set-theoretic attention and relational ledger

Branch: [`version-2-set-theoretic-attention`](https://github.com/capiyo/Qoin-full-repo/tree/version-2-set-theoretic-attention)

Version 2 explores explicit sets of market conditions, relations among those conditions, attention/selection over the relations, and an append-only ledger that records the evidence and transformations behind each research decision. The vector baseline remains the Version 1 reference; Version 2 must not silently replace its outputs or become authoritative without comparative validation.

The ledger is for traceability and reproducibility. It should record source timestamps, instrument/timeframe identity, feature and relation versions, selected evidence, decision provenance and evaluation outcomes. It must not imply that a relation is causal merely because it is recorded.

## Evaluation protocol

Compare versions on the same instruments, timestamped events, chronological splits, feature availability, risk budgets and realistic execution costs. Prevent look-ahead and survivorship leakage. Use walk-forward and out-of-sample evaluation, include fees/spread/slippage/funding and delayed fills, and report uncertainty and drawdowns alongside returns. Do not promote a version based on training loss or in-sample performance alone.

## Safety

Both versions remain research/paper-only. [`docs/RISK_POLICY.md`](../RISK_POLICY.md) remains binding. Live trading requires every execution, risk, reconciliation, recovery and kill-switch gate to be implemented and independently verified, plus explicit written approval.
