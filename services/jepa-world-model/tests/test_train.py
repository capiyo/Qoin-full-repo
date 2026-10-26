from pathlib import Path

import torch

from qoin_jepa.train import train


def test_train_writes_checkpoint_and_metadata(tmp_path: Path) -> None:
    csv_path = tmp_path / "features.csv"
    with csv_path.open("w", encoding="utf-8") as handle:
        handle.write("timestamp,flow,spread,volatility\n")
        for i in range(80):
            handle.write(f"2026-01-{(i // 24) + 1:02d}T{(i % 24):02d}:00:00Z,{i * 0.1},{i % 7},{(i % 5) * 0.2}\n")
    output = tmp_path / "model.pt"
    result = train(
        str(csv_path), str(output), context_steps=5, horizon=1,
        train_fraction=0.75, epochs=2, batch_size=8, latent_dim=8, hidden_dim=16,
    )
    assert output.exists()
    assert result["train_windows"] > 0
    assert result["validation_windows"] > 0
    checkpoint = torch.load(output, map_location="cpu", weights_only=True)
    assert checkpoint["model_config"]["feature_names"] == ["flow", "spread", "volatility"]
    assert checkpoint["training_metadata"]["train_cutoff_index"] == 60
