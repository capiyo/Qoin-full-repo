# Qoin JEPA World Model (Version 2)

Research-only PyTorch scaffold for predicting future latent representations from sequences of causal market-feature vectors.

This is a compact **JEPA-inspired market model**, not Meta's V-JEPA implementation, not pretrained financial weights, and not a proven trading signal. It is intentionally separate from the authoritative Rust/Axum decision engine.

## Model contract

- Input: `context` shaped `[batch, time, feature_dim]`, built from timestamped observations available at prediction time.
- Context encoder: maps each feature vector to latent embeddings.
- Predictor: predicts future latent embeddings from context and a learned horizon embedding.
- Target encoder: supplies future-state embeddings during training; its parameters are updated with an exponential moving average and its outputs are detached from gradient updates.
- Objective: latent prediction loss only; no raw-price reconstruction and no BUY/SELL output.
- Output: predicted future latent state. A downstream, separately validated head or decision layer is required before it can influence any market decision.

## Local research setup

Python 3.11+ is recommended.

```sh
python -m venv .venv
. .venv/bin/activate
python -m pip install -e '.[dev]'
python -m pytest
```

Use chronological data windows, past-only normalization, and leakage-safe validation. The model cannot be evaluated meaningfully on synthetic examples alone. Do not commit data containing secrets or exchange credentials.

## Integration boundary

Axum remains authoritative for state, vector decisions, risk checks, and persistence. Do not let this service place orders or directly activate strategy-map changes. Until an authenticated, versioned inference contract and failure policy are implemented and tested, the service is not connected to the trading path.
