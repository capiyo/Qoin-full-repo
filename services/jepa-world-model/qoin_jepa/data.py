"""Leakage-aware CSV loading and chronological windows for JEPA research.

CSV format: first column `timestamp` (ISO-8601 or sortable UTC string), then
one or more numeric feature columns. Rows must be strictly chronological.
Normalization is fitted on the training prefix only.
"""
from __future__ import annotations

import csv
import math
from dataclasses import dataclass
from pathlib import Path

import torch
from torch import Tensor


@dataclass(frozen=True)
class Standardizer:
    means: tuple[float, ...]
    scales: tuple[float, ...]

    @classmethod
    def fit(cls, rows: list[tuple[float, ...]]) -> "Standardizer":
        if not rows:
            raise ValueError("cannot fit normalizer without training rows")
        width = len(rows[0])
        if width == 0 or any(len(row) != width for row in rows):
            raise ValueError("training rows must have consistent nonzero feature width")
        means = tuple(sum(row[i] for row in rows) / len(rows) for i in range(width))
        variances = tuple(
            sum((row[i] - means[i]) ** 2 for row in rows) / len(rows)
            for i in range(width)
        )
        scales = tuple(math.sqrt(v) if v > 1e-12 else 1.0 for v in variances)
        return cls(means, scales)

    def transform(self, rows: list[tuple[float, ...]]) -> Tensor:
        if any(len(row) != len(self.means) for row in rows):
            raise ValueError("feature width does not match fitted normalizer")
        return torch.tensor(
            [[(x - mean) / scale for x, mean, scale in zip(row, self.means, self.scales, strict=True)]
             for row in rows],
            dtype=torch.float32,
        )


def read_market_csv(path: str | Path) -> tuple[list[str], list[tuple[float, ...]]]:
    """Read a timestamped CSV and reject malformed, nonfinite, or unsorted rows."""
    with Path(path).open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        fields = reader.fieldnames or []
        if len(fields) < 2 or fields[0].lower() != "timestamp":
            raise ValueError("CSV must start with timestamp and include numeric feature columns")
        feature_names = fields[1:]
        timestamps: list[str] = []
        rows: list[tuple[float, ...]] = []
        for line_number, record in enumerate(reader, start=2):
            timestamp = (record.get(fields[0]) or "").strip()
            if not timestamp:
                raise ValueError(f"missing timestamp at CSV line {line_number}")
            try:
                row = tuple(float(record[name]) for name in feature_names)
            except (TypeError, ValueError) as exc:
                raise ValueError(f"invalid numeric feature at CSV line {line_number}") from exc
            if not all(math.isfinite(value) for value in row):
                raise ValueError(f"non-finite feature at CSV line {line_number}")
            timestamps.append(timestamp)
            rows.append(row)
    if len(rows) < 3:
        raise ValueError("at least three observations are required")
    if any(current <= previous for previous, current in zip(timestamps, timestamps[1:])):
        raise ValueError("timestamps must be strictly increasing; sort/deduplicate upstream")
    return feature_names, rows


def make_windows(
    normalized_rows: Tensor, context_steps: int, horizon: int, stop_index: int
) -> list[tuple[Tensor, Tensor]]:
    """Build context/future pairs whose future target is before stop_index."""
    if normalized_rows.ndim != 2:
        raise ValueError("normalized_rows must have shape [time, feature]")
    if context_steps <= 0 or horizon <= 0:
        raise ValueError("context_steps and horizon must be positive")
    stop = min(stop_index, normalized_rows.shape[0])
    samples = []
    for target_index in range(context_steps + horizon - 1, stop):
        context_end = target_index - horizon + 1
        context_start = context_end - context_steps
        samples.append((
            normalized_rows[context_start:context_end],
            normalized_rows[target_index],
        ))
    return samples
