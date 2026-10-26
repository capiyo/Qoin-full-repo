# Qoin architecture versions

Qoin maintains two independently reviewable architecture tracks:

- **Version 1 — Vector baseline:** `version-1-vector-baseline`. The existing architecture, preserved as the reference baseline.
- **Version 2 — JEPA world model:** `version-2-jepa-world-model`. Version 1's frontend, Drogon gateway, Axum decision authority, vector engine, MongoDB persistence, and risk boundaries remain in place. Version 2 adds a JEPA-inspired learned latent-state prediction service.

These are Git branches, not a destructive in-place rewrite. Version 2 predictions are additional evidence, never direct orders and never a bypass around the Version 1 decision/risk path.

## Promotion rule

Version 2 must show reproducible, out-of-sample incremental value over Version 1 after fees, spread, slippage, funding, latency, and realistic fill assumptions. Training loss alone is not evidence of trading edge. Both tracks remain research/paper-only; live trading requires every gate in `docs/RISK_POLICY.md` plus explicit written user approval.

## Reference material

The JEPA design is inspired by latent-state prediction in Meta's JEPA/V-JEPA research, but Qoin's market-feature model is a separate research implementation—not a port of Meta's video model and not a pretrained financial model.
