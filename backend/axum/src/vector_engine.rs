use serde::Serialize;

pub const FEATURE_NAMES: [&str; 6] = [
    "order_flow_imbalance",
    "funding_pressure",
    "liquidity_stress",
    "volatility_shock",
    "event_intensity",
    "liquidation_pressure",
];

#[derive(Debug, Clone, Serialize)]
pub struct VectorDecision {
    pub symbol: String,
    pub model_version: String,
    pub features: Vec<f64>,
    pub feature_names: Vec<&'static str>,
    pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>,
    pub decision_vector: [f64; 2],
    pub magnitude: f64,
    pub angle_radians: f64,
    pub buy_angle_radians: f64,
    pub sell_angle_radians: f64,
    pub angular_threshold_radians: f64,
    pub magnitude_threshold: f64,
    pub action: &'static str,
    pub reason: String,
    pub caveats: Vec<String>,
}

fn softmax(values: &[f64]) -> Vec<f64> {
    let max_value = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = values.iter().map(|v| (v - max_value).exp()).collect();
    let sum: f64 = exps.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return vec![1.0 / values.len() as f64; values.len()];
    }
    exps.into_iter().map(|v| v / sum).collect()
}

/// Deterministic, state-dependent scalar self-attention baseline. This is
/// explicitly a research model, not a trained transformer.
pub fn decide(
    symbol: &str,
    order_flow_imbalance: f64,
    funding_rate: Option<f64>,
    spread_bps: Option<f64>,
    ewma_abs_return: f64,
    event_intensity: f64,
    liquidation_events: usize,
    last_liquidation_side: Option<&str>,
) -> VectorDecision {
    let spread = spread_bps.unwrap_or(100.0).clamp(0.0, 500.0) / 100.0;
    let features = vec![
        order_flow_imbalance.clamp(-1.0, 1.0),
        (funding_rate.unwrap_or(0.0) * 100.0).clamp(-1.0, 1.0),
        (spread / 5.0).clamp(0.0, 1.0),
        (ewma_abs_return / 0.01).clamp(0.0, 1.0),
        (event_intensity / 10.0).clamp(0.0, 1.0),
        (liquidation_events as f64 / 10.0).clamp(0.0, 1.0)
            * match last_liquidation_side { Some("long") => -1.0, Some("short") => 1.0, _ => 0.0 },
    ];
    let n = features.len();
    let mut attention = vec![vec![0.0; n]; n];
    let mut attended = vec![0.0; n];
    for i in 0..n {
        let scores: Vec<f64> = (0..n).map(|j| features[i] * features[j]).collect();
        attention[i] = softmax(&scores);
        attended[i] = (features[i] + (0..n).map(|j| attention[i][j] * features[j]).sum::<f64>()).clamp(-2.0, 2.0);
    }
    let bullish = (0.70 * attended[0] - 0.15 * attended[1] - 0.10 * attended[2]
        - 0.08 * attended[3] + 0.10 * attended[5]).max(0.0);
    let bearish = (-0.70 * attended[0] + 0.15 * attended[1] - 0.10 * attended[2]
        - 0.08 * attended[3] - 0.10 * attended[5]).max(0.0);
    let magnitude = (bullish * bullish + bearish * bearish).sqrt();
    let angle = bearish.atan2(bullish);
    let buy_angle = angle;
    let sell_angle = std::f64::consts::FRAC_PI_2 - angle;
    let angular_threshold = 25.0_f64.to_radians();
    let magnitude_threshold = 0.35;
    let quality_ok = spread_bps.map(|s| s.is_finite() && s <= 25.0).unwrap_or(false);
    let (action, reason) = if !quality_ok {
        ("wait", "spread missing or above the conservative 25 bps baseline gate".to_string())
    } else if event_intensity <= 0.0 {
        ("wait", "no recent event-intensity evidence".to_string())
    } else if magnitude < magnitude_threshold {
        ("wait", "decision-vector magnitude below threshold".to_string())
    } else if buy_angle <= angular_threshold && bullish > bearish {
        ("buy", "vector lies inside buy angular boundary and passes magnitude/liquidity gates".to_string())
    } else if sell_angle <= angular_threshold && bearish > bullish {
        ("sell", "vector lies inside sell angular boundary and passes magnitude/liquidity gates".to_string())
    } else {
        ("wait", "vector direction is outside both angular decision boundaries".to_string())
    };
    VectorDecision {
        symbol: symbol.to_string(), model_version: "qoin-vector-baseline-v1".to_string(),
        features, feature_names: FEATURE_NAMES.to_vec(), attention, attended_features: attended,
        decision_vector: [bullish, bearish], magnitude, angle_radians: angle,
        buy_angle_radians: buy_angle, sell_angle_radians: sell_angle,
        angular_threshold_radians: angular_threshold, magnitude_threshold, action, reason,
        caveats: vec![
            "Attention is deterministic and hand-specified; it is not a trained model or causal attribution.".into(),
            "Feature scaling and directional coefficients require chronological walk-forward calibration.".into(),
            "A sell signal is not an instruction to open a short; position and risk policy must decide close/reduce/open.".into(),
            "This endpoint emits research signals only and does not submit orders.".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attention_rows_sum_to_one() {
        let d = decide("BTCUSDT", 0.4, Some(0.001), Some(3.0), 0.001, 4.0, 0, None);
        for row in d.attention {
            assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9);
            assert!(row.iter().all(|w| *w >= 0.0 && w.is_finite()));
        }
    }
    #[test]
    fn missing_spread_abstains() {
        let d = decide("BTCUSDT", 0.9, None, None, 0.001, 4.0, 0, None);
        assert_eq!(d.action, "wait");
    }
    #[test]
    fn decision_is_reproducible() {
        let a = decide("BTCUSDT", 0.3, Some(0.001), Some(2.0), 0.002, 3.0, 1, Some("short"));
        let b = decide("BTCUSDT", 0.3, Some(0.001), Some(2.0), 0.002, 3.0, 1, Some("short"));
        assert_eq!(a.decision_vector, b.decision_vector);
        assert_eq!(a.attention, b.attention);
    }
}
