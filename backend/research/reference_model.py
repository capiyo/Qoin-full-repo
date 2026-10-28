"""Small deterministic reference helpers for Qoin feature tests."""
from math import isfinite
import math

def signed_flow(trades):
    """trades: iterable of (side, size); unknown side contributes no signed flow."""
    total = 0.0
    for side, size in trades:
        if not isfinite(size) or size < 0:
            raise ValueError("size must be finite and non-negative")
        if side == "buy":
            total += size
        elif side == "sell":
            total -= size
        elif side != "unknown":
            raise ValueError("side must be buy, sell, or unknown")
    return total

def queue_imbalance(bid_size, ask_size):
    if any(not isfinite(x) or x < 0 for x in (bid_size, ask_size)):
        raise ValueError("depth must be finite and non-negative")
    denominator = bid_size + ask_size
    return 0.0 if denominator == 0 else (bid_size - ask_size) / denominator

def decay_intensity(intensity, elapsed_seconds, tau=30.0):
    if intensity < 0 or elapsed_seconds < 0 or tau <= 0:
        raise ValueError("invalid intensity, elapsed time, or tau")
    return intensity * math.exp(-elapsed_seconds / tau)
