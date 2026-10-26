from pathlib import Path

import pytest
import torch

from qoin_jepa.data import Standardizer, make_windows, read_market_csv


def test_standardizer_uses_training_rows_only() -> None:
    scaler = Standardizer.fit([(1.0, 10.0), (3.0, 14.0)])
    transformed = scaler.transform([(2.0, 12.0), (1000.0, 1000.0)])
    assert torch.allclose(transformed[0], torch.tensor([0.0, 0.0]))
    assert transformed[1, 0] > 100.0


def test_csv_requires_strictly_increasing_timestamps(tmp_path: Path) -> None:
    path = tmp_path / "market.csv"
    path.write_text(
        "timestamp,flow,spread\n2026-01-02T00:00:00Z,1,2\n"
        "2026-01-01T00:00:00Z,2,3\n2026-01-03T00:00:00Z,3,4\n",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="strictly increasing"):
        read_market_csv(path)


def test_csv_rejects_non_finite_values(tmp_path: Path) -> None:
    path = tmp_path / "market.csv"
    path.write_text(
        "timestamp,flow\n2026-01-01T00:00:00Z,1\n"
        "2026-01-02T00:00:00Z,nan\n2026-01-03T00:00:00Z,3\n",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="non-finite"):
        read_market_csv(path)


def test_windows_do_not_include_future_target_in_context() -> None:
    rows = torch.arange(20, dtype=torch.float32).reshape(10, 2)
    samples = make_windows(rows, context_steps=3, horizon=2, stop_index=8)
    context, future = samples[0]
    assert context.shape == (3, 2)
    assert torch.equal(context, rows[:3])
    assert torch.equal(future, rows[4])
    assert all(int(target[0]) < int(rows[8, 0]) for _, target in samples)
