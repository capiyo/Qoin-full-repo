"""Versioned, read-only inference for a trained JEPA research checkpoint."""
from __future__ import annotations

import math
from pathlib import Path
from typing import Sequence

import torch

from .data import Standardizer
from .model import MarketJEPA


class MarketJEPAInference:
    """Load a checkpoint and predict a future latent state; never emits orders."""

    def __init__(self, checkpoint_path: str | Path) -> None:
        checkpoint = torch.load(checkpoint_path, map_location="cpu", weights_only=True)
        if checkpoint.get("format_version") != 1:
            raise ValueError("unsupported JEPA checkpoint format")
        config = checkpoint["model_config"]
        self.feature_names = tuple(config["feature_names"])
        self.context_steps = int(config["context_steps"])
        self.horizon = int(config["max_horizon"])
        self.normalizer = Standardizer(
            tuple(float(x) for x in checkpoint["normalizer"]["means"]),
            tuple(float(x) for x in checkpoint["normalizer"]["scales"]),
        )
        if len(self.feature_names) != len(self.normalizer.means):
            raise ValueError("checkpoint feature names and normalizer dimensions disagree")
        if any(not math.isfinite(x) for x in (*self.normalizer.means, *self.normalizer.scales)):
            raise ValueError("checkpoint normalizer contains non-finite values")
        if any(x <= 0 for x in self.normalizer.scales):
            raise ValueError("checkpoint normalizer scales must be positive")
        self.model = MarketJEPA(
            feature_dim=int(config["feature_dim"]),
            latent_dim=int(config["latent_dim"]),
            hidden_dim=int(config["hidden_dim"]),
            max_horizon=int(config["max_horizon"]),
        )
        self.model.load_state_dict(checkpoint["model_state_dict"], strict=True)
        self.model.eval()
        self.model_version = str(checkpoint.get("training_metadata", {}).get("model_version", "unversioned"))

    @torch.no_grad()
    def predict(self, rows: Sequence[Sequence[float]], horizon: int = 1) -> dict[str, object]:
        """Predict a latent state from the most recent context rows.

        Input rows must follow checkpoint.feature_names order and represent only
        observations available at prediction time. No trading signal is returned.
        """
        if len(rows) != self.context_steps:
            raise ValueError(f"expected exactly {self.context_steps} context rows")
        normalized_rows = []
        for index, row in enumerate(rows):
            if len(row) != len(self.feature_names):
                raise ValueError(f"row {index} has wrong feature width")
            values = tuple(float(value) for value in row)
            if not all(math.isfinite(value) for value in values):
                raise ValueError(f"row {index} contains non-finite values")
            normalized_rows.append(values)
        normalized = self.normalizer.transform(normalized_rows).unsqueeze(0)
        latent, _ = self.model(normalized, future=None, horizon=horizon)
        return {
            "model_version": self.model_version,
            "horizon_steps": horizon,
            "feature_names": list(self.feature_names),
            "predicted_latent_state": latent[0].cpu().tolist(),
            "decision": None,
            "note": "Latent prediction only; not a BUY/SELL signal and not an uncertainty estimate.",
        }
