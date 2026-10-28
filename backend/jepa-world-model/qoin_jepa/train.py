"""Train the JEPA research model on chronological CSV market features."""
from __future__ import annotations

import argparse
import json
import random
from pathlib import Path

import torch

from .data import Standardizer, make_windows, read_market_csv
from .model import MarketJEPA, latent_prediction_loss
from .training import train_batch


def train(
    csv_path: str,
    output_path: str,
    *,
    context_steps: int = 32,
    horizon: int = 1,
    train_fraction: float = 0.8,
    epochs: int = 10,
    batch_size: int = 64,
    learning_rate: float = 1e-3,
    latent_dim: int = 64,
    hidden_dim: int = 128,
    seed: int = 7,
) -> dict[str, object]:
    if not 0.5 <= train_fraction < 1.0:
        raise ValueError("train_fraction must be in [0.5, 1.0)")
    if epochs <= 0 or batch_size <= 0 or learning_rate <= 0:
        raise ValueError("epochs, batch_size, and learning_rate must be positive")
    random.seed(seed)
    torch.manual_seed(seed)

    feature_names, raw_rows = read_market_csv(csv_path)
    split = int(len(raw_rows) * train_fraction)
    if split <= context_steps + horizon or len(raw_rows) - split < horizon:
        raise ValueError("not enough rows for requested context, horizon, and chronological split")

    # Fit scaler strictly on the training prefix. Validation rows never influence it.
    scaler = Standardizer.fit(raw_rows[:split])
    normalized = scaler.transform(raw_rows)
    train_samples = make_windows(normalized, context_steps, horizon, split)
    # Validation context may include historical training observations; targets must be
    # strictly inside the validation suffix.
    val_samples = []
    for target_index in range(split, len(raw_rows)):
        context_end = target_index - horizon + 1
        context_start = context_end - context_steps
        if context_start < 0:
            continue
        val_samples.append((normalized[context_start:context_end], normalized[target_index]))
    if not train_samples or not val_samples:
        raise ValueError("split produced no training or validation windows")

    model = MarketJEPA(
        feature_dim=len(feature_names),
        latent_dim=latent_dim,
        hidden_dim=hidden_dim,
        max_horizon=max(horizon, 1),
    )
    optimizer = torch.optim.AdamW(
        [parameter for parameter in model.parameters() if parameter.requires_grad],
        lr=learning_rate,
    )
    best_validation = float("inf")
    best_state: dict[str, torch.Tensor] | None = None
    history: list[dict[str, float]] = []

    for epoch in range(epochs):
        model.train()
        order = list(range(len(train_samples)))
        random.shuffle(order)
        training_loss = 0.0
        training_count = 0
        for offset in range(0, len(order), batch_size):
            batch = [train_samples[i] for i in order[offset:offset + batch_size]]
            context = torch.stack([sample[0] for sample in batch])
            future = torch.stack([sample[1] for sample in batch])
            loss = train_batch(model, optimizer, context, future, horizon)
            training_loss += float(loss) * len(batch)
            training_count += len(batch)

        model.eval()
        validation_loss = 0.0
        with torch.no_grad():
            for offset in range(0, len(val_samples), batch_size):
                batch = val_samples[offset:offset + batch_size]
                context = torch.stack([sample[0] for sample in batch])
                future = torch.stack([sample[1] for sample in batch])
                predicted, target = model(context, future, horizon)
                assert target is not None
                validation_loss += float(latent_prediction_loss(predicted, target)) * len(batch)
        validation_loss /= len(val_samples)
        epoch_record = {
            "epoch": float(epoch + 1),
            "train_loss": training_loss / training_count,
            "validation_loss": validation_loss,
        }
        history.append(epoch_record)
        if validation_loss < best_validation:
            best_validation = validation_loss
            best_state = {key: value.detach().cpu().clone() for key, value in model.state_dict().items()}

    if best_state is None:
        raise RuntimeError("training did not produce a checkpoint")
    destination = Path(output_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    torch.save({
        "format_version": 1,
        "model_state_dict": best_state,
        "model_config": {
            "feature_names": feature_names,
            "feature_dim": len(feature_names),
            "latent_dim": latent_dim,
            "hidden_dim": hidden_dim,
            "max_horizon": max(horizon, 1),
            "context_steps": context_steps,
        },
        "normalizer": {"means": scaler.means, "scales": scaler.scales},
        "training_metadata": {
            "model_version": "qoin-jepa-latent-v1",
            "csv_path_name": Path(csv_path).name,
            "row_count": len(raw_rows),
            "train_cutoff_index": split,
            "train_fraction": train_fraction,
            "horizon": horizon,
            "seed": seed,
            "history": history,
            "best_validation_loss": best_validation,
        },
    }, destination)
    return {
        "checkpoint": str(destination),
        "feature_names": feature_names,
        "train_windows": len(train_samples),
        "validation_windows": len(val_samples),
        "best_validation_loss": best_validation,
        "history": history,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--csv", required=True, help="Timestamp-first numeric feature CSV")
    parser.add_argument("--output", default="artifacts/qoin-jepa.pt")
    parser.add_argument("--context-steps", type=int, default=32)
    parser.add_argument("--horizon", type=int, default=1)
    parser.add_argument("--train-fraction", type=float, default=0.8)
    parser.add_argument("--epochs", type=int, default=10)
    parser.add_argument("--batch-size", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=1e-3)
    parser.add_argument("--latent-dim", type=int, default=64)
    parser.add_argument("--hidden-dim", type=int, default=128)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    result = train(
        args.csv, args.output, context_steps=args.context_steps, horizon=args.horizon,
        train_fraction=args.train_fraction, epochs=args.epochs, batch_size=args.batch_size,
        learning_rate=args.learning_rate, latent_dim=args.latent_dim,
        hidden_dim=args.hidden_dim, seed=args.seed,
    )
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
