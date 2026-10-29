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
    weight_decay: float = 1e-4,
    variance_weight: float = 0.05,
    covariance_weight: float = 0.01,
    max_grad_norm: float = 1.0,
    patience: int = 4,
    latent_dim: int = 64,
    hidden_dim: int = 128,
    seed: int = 7,
) -> dict[str, object]:
    if not 0.5 <= train_fraction < 1.0:
        raise ValueError("train_fraction must be in [0.5, 1.0)")
    if epochs <= 0 or batch_size <= 0 or learning_rate <= 0 or patience <= 0:
        raise ValueError("epochs, batch_size, learning_rate, and patience must be positive")
    if weight_decay < 0 or variance_weight < 0 or covariance_weight < 0 or max_grad_norm <= 0:
        raise ValueError("weight_decay and loss weights must be non-negative; max_grad_norm must be positive")
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
        weight_decay=weight_decay,
    )
    scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(
        optimizer, mode="min", factor=0.5, patience=max(1, patience // 2),
        min_lr=learning_rate * 1e-3,
    )
    best_validation = float("inf")
    epochs_without_improvement = 0
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
            loss = train_batch(
                model, optimizer, context, future, horizon,
                variance_weight=variance_weight,
                covariance_weight=covariance_weight,
                max_grad_norm=max_grad_norm,
            )
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
                validation_loss += float(latent_prediction_loss(
                    predicted, target,
                    variance_weight=variance_weight,
                    covariance_weight=covariance_weight,
                )) * len(batch)
        validation_loss /= len(val_samples)
        epoch_record = {
            "epoch": float(epoch + 1),
            "train_loss": training_loss / training_count,
            "validation_loss": validation_loss,
        }
        history.append(epoch_record)
        scheduler.step(validation_loss)
        if validation_loss < best_validation:
            best_validation = validation_loss
            epochs_without_improvement = 0
            best_state = {key: value.detach().cpu().clone() for key, value in model.state_dict().items()}
        else:
            epochs_without_improvement += 1
        if epochs_without_improvement >= patience:
            break

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
            "optimizer": "AdamW",
            "learning_rate_initial": learning_rate,
            "weight_decay": weight_decay,
            "gradient_clip_norm": max_grad_norm,
            "loss": {
                "prediction": "smooth_l1_huber",
                "variance_weight": variance_weight,
                "covariance_weight": covariance_weight,
                "variance_floor": 1.0,
            },
            "early_stopping_patience": patience,
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
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--variance-weight", type=float, default=0.05)
    parser.add_argument("--covariance-weight", type=float, default=0.01)
    parser.add_argument("--max-grad-norm", type=float, default=1.0)
    parser.add_argument("--patience", type=int, default=4)
    parser.add_argument("--latent-dim", type=int, default=64)
    parser.add_argument("--hidden-dim", type=int, default=128)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    result = train(
        args.csv, args.output, context_steps=args.context_steps, horizon=args.horizon,
        train_fraction=args.train_fraction, epochs=args.epochs, batch_size=args.batch_size,
        learning_rate=args.learning_rate, weight_decay=args.weight_decay,
        variance_weight=args.variance_weight, covariance_weight=args.covariance_weight,
        max_grad_norm=args.max_grad_norm, patience=args.patience,
        latent_dim=args.latent_dim, hidden_dim=args.hidden_dim, seed=args.seed,
    )
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
