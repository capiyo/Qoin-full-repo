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

Python 3.11+ is recommended. Install from this directory:

```sh
python -m venv .venv
. .venv/bin/activate
python -m pip install -e '.[dev]'
python -m pytest
```

## Train a research checkpoint

Prepare a CSV with a strictly increasing ISO-8601 timestamp column first, followed by finite numeric feature columns. Example header:

```csv
timestamp,order_flow_imbalance,spread_bps,realized_volatility,funding_rate,open_interest_change,liquidation_pressure,basis_bps
```

Rows must be deduplicated, time-aligned, and limited to observations actually available at each timestamp. Then run:

```sh
python -m qoin_jepa.train --csv /path/to/causal-market-features.csv --output artifacts/qoin-jepa.pt --context-steps 32 --horizon 1 --epochs 10
```

The scaler is fit only on the chronological training prefix. The checkpoint records feature names, scaler values, split cutoff, training history, and model configuration. The validation suffix is not used to fit normalization. Training windows may be shuffled within the training partition; the chronological partition itself is never shuffled.

A lower latent loss is not proof of predictive utility or trading edge. Evaluate against persistence and Version 1 on untouched chronological periods with costs, regime slices, calibration, and realistic execution assumptions. Do not commit real market data if it contains credentials or private account information.

## Loopback-only inference

After training a checkpoint, start the local research endpoint:

```sh
python -m qoin_jepa.serve --checkpoint artifacts/qoin-jepa.pt --host 127.0.0.1 --port 8765
```

- `GET /health` reports model metadata.
- `POST /predict` accepts JSON with `feature_names` (must exactly match the checkpoint feature order), `rows` (the most recent `context_steps` feature rows), and optional integer `horizon`.
- The endpoint binds only to loopback, caps request bodies, rejects invalid input, and returns a latent representation only. It is not authenticated for public access, is not connected to Axum, and must not be exposed outside the local machine.
- No decision or order endpoint exists. Axum remains the only authority for decisions and risk gates.

## Integration boundary

Axum remains authoritative for state, vector decisions, risk checks, and persistence. Do not let this service place orders or directly activate strategy-map changes. Until an authenticated, versioned inference contract and failure policy are implemented and tested, the service is not connected to the trading path.
