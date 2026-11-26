"""Thin Python bridge to Qoin's authoritative Rust signal endpoint.

This module intentionally contains no quantitative decision logic. Research and
backtest callers submit normalized observations to Axum, which invokes the
production qoin-vector-engine crate and returns its versioned Signal.
"""
from __future__ import annotations

import json
import math
import os
from typing import Any, Mapping
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def _base_url(value: str | None) -> str:
    configured = value or os.environ.get("QOIN_AXUM_URL") or os.environ.get("QOIN_API_URL")
    return (configured or "http://127.0.0.1:8080").rstrip("/")


def evaluate_signal(
    observation: Mapping[str, Any],
    *,
    has_open_position: bool = False,
    base_url: str | None = None,
    timeout_seconds: float = 5.0,
) -> dict[str, Any]:
    """Evaluate one observation using the Rust engine served by Axum.

    The observation schema is the Rust `ForceObservation` contract:
    ofi, liquidity, volatility, funding, open_interest_change,
    liquidation_pressure, basis, spread_bps and spread_age_ms.
    """
    if not isinstance(has_open_position, bool):
        raise ValueError("has_open_position must be boolean")
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0 or timeout_seconds > 60:
        raise ValueError("timeout_seconds must be finite and in (0, 60]")
    required = (
        "ofi",
        "liquidity",
        "volatility",
        "funding",
        "open_interest_change",
        "liquidation_pressure",
        "basis",
    )
    missing = [key for key in required if key not in observation]
    if missing:
        raise ValueError(f"missing Rust observation fields: {', '.join(missing)}")
    payload_observation: dict[str, float | int | None] = {}
    for key in required:
        value = observation[key]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
            raise ValueError(f"{key} must be a finite number")
        payload_observation[key] = float(value)
    spread = observation.get("spread_bps")
    if spread is not None and (
        isinstance(spread, bool)
        or not isinstance(spread, (int, float))
        or not math.isfinite(float(spread))
        or float(spread) < 0
    ):
        raise ValueError("spread_bps must be null or a finite non-negative number")
    age = observation.get("spread_age_ms")
    if age is not None and (
        isinstance(age, bool) or not isinstance(age, int) or age < 0
    ):
        raise ValueError("spread_age_ms must be null or a non-negative integer")
    payload_observation["spread_bps"] = None if spread is None else float(spread)
    payload_observation["spread_age_ms"] = age

    payload = json.dumps(
        {"observation": payload_observation, "has_open_position": has_open_position},
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    request = Request(
        f"{_base_url(base_url)}/v1/signal",
        data=payload,
        headers={"content-type": "application/json", "accept": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=timeout_seconds) as response:
            result = json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        raise RuntimeError(f"Rust signal endpoint returned HTTP {error.code}") from None
    except (URLError, TimeoutError, OSError) as error:
        raise RuntimeError("Rust signal endpoint is unavailable") from error
    if not isinstance(result, dict) or not isinstance(result.get("config_version"), str):
        raise RuntimeError("Rust signal endpoint returned an invalid Signal response")
    return result
