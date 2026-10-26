from pathlib import Path

import pytest
import torch

from qoin_jepa.inference import MarketJEPAInference
from qoin_jepa.train import train


def test_inference_loads_checkpoint_and_returns_latent_only(tmp_path: Path) -> None:
    csv_path = tmp_path / "features.csv"
    with csv_path.open("w", encoding="utf-8") as handle:
        handle.write("timestamp,flow,spread\n")
        for i in range(70):
            handle.write(f"2026-02-{(i // 24) + 1:02d}T{(i % 24):02d}:00:00Z,{i * 0.2},{i % 3}\n")
    checkpoint = tmp_path / "model.pt"
    train(
        str(csv_path), str(checkpoint), context_steps=5, horizon=1,
        train_fraction=0.75, epochs=1, batch_size=8, latent_dim=8, hidden_dim=16,
    )
    predictor = MarketJEPAInference(checkpoint)
    result = predictor.predict([[float(i), float(i % 3)] for i in range(5)])

    assert len(result["predicted_latent_state"]) == 8
    assert result["decision"] is None
    assert result["feature_names"] == ["flow", "spread"]


def test_inference_rejects_non_finite_values(tmp_path: Path) -> None:
    csv_path = tmp_path / "features.csv"
    with csv_path.open("w", encoding="utf-8") as handle:
        handle.write("timestamp,flow\n")
        for i in range(40):
            handle.write(f"2026-03-{(i // 24) + 1:02d}T{(i % 24):02d}:00:00Z,{i}\n")
    checkpoint = tmp_path / "model.pt"
    train(
        str(csv_path), str(checkpoint), context_steps=4, horizon=1,
        train_fraction=0.75, epochs=1, batch_size=8, latent_dim=4, hidden_dim=8,
    )
    predictor = MarketJEPAInference(checkpoint)
    with pytest.raises(ValueError, match="non-finite"):
        predictor.predict([[float("nan")]] * 4)
