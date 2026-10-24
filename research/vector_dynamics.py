"""Research-only causal vector geometry helpers; not connected to order execution."""
from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Mapping


@dataclass(frozen=True)
class VectorObservation:
    timestamp_ms: int
    forces: Mapping[str, float]

    def __post_init__(self) -> None:
        if self.timestamp_ms <= 0:
            raise ValueError("timestamp_ms must be positive")
        if not self.forces:
            raise ValueError("forces cannot be empty")
        if any(not math.isfinite(float(value)) for value in self.forces.values()):
            raise ValueError("force values must be finite")


def derive_kinematics(
    older: VectorObservation,
    previous: VectorObservation,
    current: VectorObservation,
) -> dict[str, dict[str, float]]:
    """Calculate per-force position, velocity and acceleration from past/current samples only."""
    if not older.timestamp_ms < previous.timestamp_ms < current.timestamp_ms:
        raise ValueError("observations must be strictly chronological")
    keys = set(older.forces)
    if set(previous.forces) != keys or set(current.forces) != keys:
        raise ValueError("all observations must contain the same force names")
    dt_old = (previous.timestamp_ms - older.timestamp_ms) / 1000.0
    dt_now = (current.timestamp_ms - previous.timestamp_ms) / 1000.0
    dt_acc = (dt_old + dt_now) / 2.0
    result: dict[str, dict[str, float]] = {}
    for name in sorted(keys):
        p0 = float(older.forces[name])
        p1 = float(previous.forces[name])
        p2 = float(current.forces[name])
        v_old = (p1 - p0) / dt_old
        velocity = (p2 - p1) / dt_now
        acceleration = (velocity - v_old) / dt_acc
        result[name] = {"position": p2, "velocity": velocity, "acceleration": acceleration}
    return result


def pairwise_interactions(forces: Mapping[str, float], regularization: float = 0.25) -> dict[str, float]:
    """Return bounded pairwise products; regularization dampens large interactions."""
    if not math.isfinite(regularization) or regularization < 0:
        raise ValueError("regularization must be finite and non-negative")
    items = sorted(forces.items())
    if any(not math.isfinite(float(value)) for _, value in items):
        raise ValueError("force values must be finite")
    result: dict[str, float] = {}
    for index, (left_name, left_value) in enumerate(items):
        for right_name, right_value in items[index + 1:]:
            product = float(left_value) * float(right_value)
            result[f"{left_name}*{right_name}"] = product / (1.0 + regularization * abs(product))
    return result


def pareto_top_k(
    forces: Mapping[str, float],
    reliability: Mapping[str, float],
    k: int,
) -> list[str]:
    """Choose reliable, high-magnitude non-dominated forces with deterministic tie breaks."""
    if k < 1:
        raise ValueError("k must be at least 1")
    if set(forces) != set(reliability):
        raise ValueError("forces and reliability must have identical keys")
    pairs = []
    for name, raw_value in forces.items():
        value, trust = float(raw_value), float(reliability[name])
        if not math.isfinite(value) or not math.isfinite(trust) or not 0.0 <= trust <= 1.0:
            raise ValueError("force values must be finite and reliability must be in [0, 1]")
        pairs.append((name, abs(value), trust, abs(value) * trust))
    frontier = []
    for candidate in pairs:
        _, magnitude, trust, _ = candidate
        dominated = any(
            other[1] >= magnitude and other[2] >= trust
            and (other[1] > magnitude or other[2] > trust)
            for other in pairs
        )
        if not dominated:
            frontier.append(candidate)
    frontier.sort(key=lambda item: (-item[3], -item[1], -item[2], item[0]))
    return [item[0] for item in frontier[:k]]
