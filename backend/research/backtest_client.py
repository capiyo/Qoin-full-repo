"""Python client for the authoritative Rust chronological backtest endpoint.

No Python signal or fill engine is implemented here. The Rust Axum service
replays the persisted event log and applies the production qoin-vector-engine
and paper execution assumptions.
"""
from __future__ import annotations

import json
import math
import os
from urllib.parse import urlencode
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError
from typing import Any


def run_rust_backtest(
    *,
    symbol: str,
    venue: str | None = None,
    from_ts_ms: int = 0,
    to_ts_ms: int,
    limit: int = 50_000,
    initial_capital: float = 10_000.0,
    fee_bps: float = 10.0,
    slippage_bps: float = 5.0,
    latency_ms: int = 0,
    max_participation_rate: float = 0.1,
    market_impact_bps: float = 5.0,
    base_url: str | None = None,
    timeout_seconds: float = 30.0,
) -> dict[str, Any]:
    """Run the Rust backtest over persisted chronological events."""
    if not symbol.strip():
        raise ValueError("symbol is required")
    if from_ts_ms < 0 or to_ts_ms <= 0 or from_ts_ms > to_ts_ms:
        raise ValueError("invalid replay time range")
    if not 1 <= limit <= 50_000:
        raise ValueError("limit must be between 1 and 50000")
    if not math.isfinite(initial_capital) or initial_capital <= 0:
        raise ValueError("initial_capital must be finite and positive")
    if not math.isfinite(fee_bps) or fee_bps < 0:
        raise ValueError("fee_bps must be finite and non-negative")
    if not math.isfinite(slippage_bps) or not 0 <= slippage_bps < 10_000:
        raise ValueError("slippage_bps must be in [0, 10000)")
    if not 0 <= latency_ms <= 60_000:
        raise ValueError("latency_ms must be between 0 and 60000")
    if not math.isfinite(max_participation_rate) or not 0 < max_participation_rate <= 1:
        raise ValueError("max_participation_rate must be in (0, 1]")
    if not math.isfinite(market_impact_bps) or not 0 <= market_impact_bps <= 1000:
        raise ValueError("market_impact_bps must be in [0, 1000]")
    if not math.isfinite(timeout_seconds) or not 0 < timeout_seconds <= 120:
        raise ValueError("timeout_seconds must be in (0, 120]")

    params: dict[str, str | int | float] = {
        "symbol": symbol.strip(),
        "from_ts_ms": from_ts_ms,
        "to_ts_ms": to_ts_ms,
        "limit": limit,
        "initial_capital": initial_capital,
        "fee_bps": fee_bps,
        "slippage_bps": slippage_bps,
        "latency_ms": latency_ms,
        "max_participation_rate": max_participation_rate,
        "market_impact_bps": market_impact_bps,
    }
    if venue and venue.strip():
        params["venue"] = venue.strip()
    root = (base_url or os.environ.get("QOIN_AXUM_INTERNAL_URL") or "http://127.0.0.1:8081").rstrip("/")
    request = Request(
        f"{root}/internal/backtest?{urlencode(params)}",
        headers={"accept": "application/json"},
        method="GET",
    )
    try:
        with urlopen(request, timeout=timeout_seconds) as response:
            result = json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        raise RuntimeError(f"Rust backtest endpoint returned HTTP {error.code}") from None
    except (URLError, TimeoutError, OSError):
        raise RuntimeError("Rust backtest endpoint is unavailable") from None
    if not isinstance(result, dict) or result.get("symbol") != symbol.strip():
        raise RuntimeError("Rust backtest endpoint returned an invalid replay result")
    return result
