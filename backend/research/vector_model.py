"""Pure-Python reference for Qoin's geometric vector baseline (not live trading)."""
from __future__ import annotations
import math

FEATURE_NAMES = (
    "order_flow_imbalance", "funding_pressure", "liquidity_stress",
    "volatility_shock", "event_intensity", "liquidation_pressure",
)

def _softmax(values: list[float]) -> list[float]:
    peak = max(values)
    exps = [math.exp(value - peak) for value in values]
    total = sum(exps)
    return [value / total for value in exps]

def decide(order_flow_imbalance: float, funding_rate: float | None,
           spread_bps: float | None, ewma_abs_return: float,
           event_intensity: float, liquidation_events: int = 0,
           last_liquidation_side: str | None = None) -> dict:
    if not all(math.isfinite(v) for v in (order_flow_imbalance, ewma_abs_return, event_intensity)):
        raise ValueError("features must be finite")
    if funding_rate is not None and not math.isfinite(funding_rate):
        raise ValueError("funding_rate must be finite")
    if spread_bps is not None and (not math.isfinite(spread_bps) or spread_bps < 0):
        raise ValueError("spread_bps must be finite and non-negative")
    spread = min(max(spread_bps if spread_bps is not None else 100.0, 0.0), 500.0) / 100.0
    liquidation_sign = -1.0 if last_liquidation_side == "long" else 1.0 if last_liquidation_side == "short" else 0.0
    features = [max(-1.0, min(1.0, order_flow_imbalance)),
                max(-1.0, min(1.0, (funding_rate or 0.0) * 100.0)),
                max(0.0, min(1.0, spread / 5.0)),
                max(0.0, min(1.0, ewma_abs_return / 0.01)),
                max(0.0, min(1.0, event_intensity / 10.0)),
                max(-1.0, min(1.0, liquidation_events / 10.0)) * liquidation_sign]
    attention, attended = [], []
    for value in features:
        weights = _softmax([value * other for other in features])
        attention.append(weights)
        attended.append(max(-2.0, min(2.0, value + sum(w * x for w, x in zip(weights, features)))))
    bullish = max(0.0, .70 * attended[0] - .15 * attended[1] - .10 * attended[2] - .08 * attended[3] + .10 * attended[5])
    bearish = max(0.0, -.70 * attended[0] + .15 * attended[1] - .10 * attended[2] - .08 * attended[3] - .10 * attended[5])
    magnitude = math.hypot(bullish, bearish)
    angle = math.atan2(bearish, bullish)
    buy_angle, sell_angle = angle, math.pi / 2 - angle
    threshold, min_magnitude = math.radians(25), .35
    if spread_bps is None or spread_bps > 25:
        action, reason = "wait", "spread missing or above the conservative 25 bps baseline gate"
    elif event_intensity <= 0:
        action, reason = "wait", "no recent event-intensity evidence"
    elif magnitude < min_magnitude:
        action, reason = "wait", "decision-vector magnitude below threshold"
    elif buy_angle <= threshold and bullish > bearish:
        action, reason = "buy", "vector lies inside buy angular boundary and passes magnitude/liquidity gates"
    elif sell_angle <= threshold and bearish > bullish:
        action, reason = "sell", "vector lies inside sell angular boundary and passes magnitude/liquidity gates"
    else:
        action, reason = "wait", "vector direction is outside both angular decision boundaries"
    return {"features": features, "feature_names": list(FEATURE_NAMES), "attention": attention,
            "attended_features": attended, "decision_vector": [bullish, bearish],
            "magnitude": magnitude, "angle_radians": angle,
            "buy_angle_radians": buy_angle, "sell_angle_radians": sell_angle,
            "angular_threshold_radians": threshold, "magnitude_threshold": min_magnitude,
            "action": action, "reason": reason}
