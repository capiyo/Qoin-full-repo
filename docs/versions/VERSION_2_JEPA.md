# Version 2 — JEPA-inspired market world model

**Branch:** `version-2-jepa-world-model`  
**Status:** Research architecture and initial model scaffold. Not a validated financial predictor and not approved for live trading.

## Objective

Retain all Version 1 services and explicit vector decision logic, and add a learned latent-state model that predicts how market conditions may evolve. The model is intended to learn nonlinear interactions among observable market dimensions that hand-designed weights may miss. It does not prove who caused a move, reveal all private information, or guarantee profitable forecasts.

This is JEPA-inspired, not a direct port of Meta's video JEPA/V-JEPA weights. Meta's published models are trained on visual data; Qoin requires its own causal, market-specific dataset and training/evaluation.

## Version 2 flow

1. **Market observations:** validated, timestamped market events arrive through the existing Drogon → Axum path.
2. **Causal state builder (Axum):** constructs a fixed-schema, past-only feature vector and quality metadata. Reject malformed, stale, out-of-order, or insufficient data.
3. **Version 1 vector path:** computes its explicit vector decision as usual and stores its strategy/model version.
4. **JEPA world-model service (research):** consumes causal sequences of normalized market features; an encoder maps context to a latent state and a predictor estimates future latent states. A target encoder supplies stop-gradient future-state targets during training. The model predicts representations, not reconstructed prices.
5. **Prediction assessment:** expose model version, forecast horizon, data timestamp, latent prediction, validation/calibration metadata, and uncertainty/quality status. A latent vector alone must not be interpreted as a BUY or SELL.
6. **Axum decision authority:** compares learned-model evidence with explicit vector evidence, regime, persistence, uncertainty, spread, estimated fees/slippage/funding, and risk limits. Contradiction or unreliable model output should reduce confidence or result in WAIT; JEPA never submits orders.
7. **Evaluation:** replay both versions on identical chronological events, execution assumptions, and risk budgets. Version 2 is only useful if it improves genuinely out-of-sample results over Version 1.

## Service ownership

- **Drogon:** remains an HTTP gateway; no model logic.
- **Axum:** remains authoritative for event validation, state, strategy decisions, risk gating, strategy-map versioning, and persistence.
- **Version 1 vector engine:** remains independently callable as the baseline.
- **JEPA service:** research inference/training only. It returns prediction metadata and never places orders or mutates the active Version 1 strategy map.
- **MongoDB:** persist model registry/version metadata, training dataset cutoffs, prediction records, evaluation results, and the associated Version 1 strategy version. Do not store credentials in model metadata.

A service failure, stale output, schema mismatch, or uncalibrated prediction must not silently change the baseline. Initial integration should fall back to the Version 1-only research path and mark the missing model output explicitly.

## Training and validation constraints

- Split data chronologically; never shuffle future observations into training context.
- Fit normalization parameters only on past/training windows and record their cutoff.
- Prevent label/target overlap across train/validation/test and use purged/embargoed evaluation where windows overlap.
- Train against future latent representations; do not use future information in encoder inputs.
- Include data-quality masks and missingness; never silently treat absent market data as zero.
- Compare against Version 1, persistence/no-change baselines, and simple predictive baselines.
- Evaluate by symbol, time period, volatility/liquidity regime, and horizon; include uncertainty calibration and drift checks.
- For trading evaluation, include fees, spread, slippage, funding, latency, delayed/partial fills, turnover, drawdown, and stressed costs.
- Keep model training and backtests from automatically rewriting production strategy weights.

## Implemented research deliverables in this branch

- This specification defines ownership and the Version 1/Version 2 boundary.
- `services/jepa-world-model` contains the JEPA-inspired PyTorch encoder/predictor, EMA target encoder, latent loss, shared training step, timestamp-ordered CSV parser, training-prefix-only standardizer, chronological training/validation pipeline, checkpoint metadata, latent-only inference API, and unit tests.
- A dedicated GitHub Actions workflow runs the Python test suite.
- No production market-data adapter, Axum inference integration, trained market checkpoint, validated trading edge, or production endpoint is claimed. Those require separate implementation and verification.

## Safety

Version 2 is research/paper-only. The existing `docs/RISK_POLICY.md` gates remain mandatory. No live trading, automatic strategy promotion, or order submission is introduced by this architecture.
