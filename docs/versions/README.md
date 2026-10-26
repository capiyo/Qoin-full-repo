# Qoin architecture versions

Qoin maintains two independently reviewable architecture tracks:

- **Version 1 — Vector baseline:** `version-1-vector-baseline`. The existing repository architecture and decision path, preserved as the comparison baseline.
- **Version 2 — JEPA world model:** `version-2-jepa-world-model`. Retains Version 1's services and hard boundaries while adding a research-only learned latent-state prediction layer.

These are Git branches, not a destructive in-place rewrite. Keep the same chronological datasets, event contracts, execution assumptions, and risk gates when comparing versions.

## Promotion rule

Version 2 must demonstrate out-of-sample incremental value over Version 1 after fees, spread, slippage, funding, latency, and realistic fill assumptions. No model is promoted based on training loss alone. Both tracks remain research/paper-only; live trading requires every gate in `docs/RISK_POLICY.md` plus explicit written user approval.
