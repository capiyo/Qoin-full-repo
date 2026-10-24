//! Versioned strategy map and deterministic decision logic shared by all services.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STRATEGY_KEY: &str = "active";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStrategy {
    pub strategy_key: String,
    pub version: String,
    pub status: String,
    /// Feature -> signed directional coefficient. Keys are stable API contract names.
    pub bullish_weights: BTreeMap<String, f64>,
    pub bearish_weights: BTreeMap<String, f64>,
    pub angular_threshold_degrees: f64,
    pub magnitude_threshold: f64,
    pub max_spread_bps: f64,
    pub attention_scale: f64,
    /// Walk-forward calibrated edge conversion. None means entries fail the cost gate.
    #[serde(default)] pub expected_edge_bps_per_magnitude: Option<f64>,
    /// Round-trip fees in basis points. None means entries fail the cost gate.
    #[serde(default)] pub fee_bps: Option<f64>,
    /// Conservative slippage estimate in basis points. None means entries fail the cost gate.
    #[serde(default)] pub slippage_bps: Option<f64>,
    #[serde(default)] pub cost_multiplier: Option<f64>,
    pub feature_scales: BTreeMap<String, f64>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeDecision {
    pub symbol: String,
    pub strategy_key: String,
    pub strategy_version: String,
    pub features: BTreeMap<String, f64>,
    pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>,
    pub decision_vector: [f64; 2],
    pub magnitude: f64,
    pub angle_radians: f64,
    pub angular_threshold_radians: f64,
    pub action: &'static str,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_veto: Option<String>,
    pub research_only: bool,
}

pub const FEATURE_NAMES: [&str; 7] = [
    "order_flow_imbalance", "book_imbalance", "funding_pressure", "liquidity_stress",
    "volatility_shock", "event_intensity", "liquidation_pressure",
];

impl EdgeStrategy {
    pub fn baseline() -> Self {
        let bullish = [0.70, 0.08, -0.15, -0.10, -0.08, 0.0, 0.10];
        let bearish = [-0.70, -0.08, 0.15, -0.10, -0.08, 0.0, -0.10];
        let mut bullish_weights = BTreeMap::new();
        let mut bearish_weights = BTreeMap::new();
        let mut feature_scales = BTreeMap::new();
        for (i, name) in FEATURE_NAMES.iter().enumerate() {
            bullish_weights.insert((*name).to_owned(), bullish[i]);
            bearish_weights.insert((*name).to_owned(), bearish[i]);
            feature_scales.insert((*name).to_owned(), [1.0, 1.0, 100.0, 500.0, 0.01, 10.0, 10.0][i]);
        }
        Self {
            strategy_key: STRATEGY_KEY.to_owned(),
            version: "edge-map-v3".to_owned(),
            status: "research_baseline".to_owned(),
            bullish_weights, bearish_weights,
            angular_threshold_degrees: 25.0,
            magnitude_threshold: 0.35,
            max_spread_bps: 25.0,
            attention_scale: 6.0,
            expected_edge_bps_per_magnitude: None,
            fee_bps: None,
            slippage_bps: None,
            cost_multiplier: Some(2.0),
            feature_scales,
            updated_at_ms: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.strategy_key != STRATEGY_KEY || self.version.trim().is_empty() {
            return Err("strategy_key must be 'active' and version must be non-empty".into());
        }
        if !self.angular_threshold_degrees.is_finite()
            || !(0.0..=90.0).contains(&self.angular_threshold_degrees)
            || !self.magnitude_threshold.is_finite()
            || self.magnitude_threshold <= 0.0
            || !self.max_spread_bps.is_finite()
            || self.max_spread_bps <= 0.0
            || !self.attention_scale.is_finite()
            || !(0.0..=10.0).contains(&self.attention_scale)
            || self.expected_edge_bps_per_magnitude.map(|v| !v.is_finite() || v <= 0.0).unwrap_or(false)
            || self.fee_bps.map(|v| !v.is_finite() || v < 0.0).unwrap_or(false)
            || self.slippage_bps.map(|v| !v.is_finite() || v < 0.0).unwrap_or(false)
            || self.cost_multiplier.map(|v| !v.is_finite() || v <= 0.0).unwrap_or(false) {
            return Err("invalid strategy thresholds or attention_scale".into());
        }
        for name in FEATURE_NAMES {
            let b = self.bullish_weights.get(name).ok_or_else(|| format!("missing bullish weight: {name}"))?;
            let s = self.bearish_weights.get(name).ok_or_else(|| format!("missing bearish weight: {name}"))?;
            let scale = self.feature_scales.get(name).ok_or_else(|| format!("missing feature scale: {name}"))?;
            if !b.is_finite() || !s.is_finite() || b.abs() > 5.0 || s.abs() > 5.0
                || !scale.is_finite() || *scale <= 0.0 {
                return Err(format!("invalid weight or scale for {name}"));
            }
        }
        Ok(())
    }

    pub fn decide(
        &self,
        symbol: &str,
        order_flow_imbalance: f64,
        book_imbalance: Option<f64>,
        funding_rate: Option<f64>,
        spread_bps: Option<f64>,
        ewma_abs_return: f64,
        event_intensity: f64,
        liquidation_events: f64,
        last_liquidation_side: Option<&str>,
    ) -> Result<EdgeDecision, String> {
        self.validate()?;
        let scale = |name: &str| *self.feature_scales.get(name).expect("validated scale");
        let raw = [
            order_flow_imbalance,
            book_imbalance.unwrap_or(0.0) / scale("book_imbalance"),
            funding_rate.unwrap_or(0.0) * scale("funding_pressure"),
            spread_bps.unwrap_or(self.max_spread_bps * 4.0) / scale("liquidity_stress"),
            ewma_abs_return / scale("volatility_shock"),
            event_intensity / scale("event_intensity"),
            (liquidation_events / scale("liquidation_pressure"))
                * match last_liquidation_side { Some("long") => -1.0, Some("short") => 1.0, _ => 0.0 },
        ];
        if raw.iter().any(|v| !v.is_finite()) { return Err("non-finite feature input".into()); }
        let features_vec: Vec<f64> = raw.iter().enumerate().map(|(i, v)| {
            let name = FEATURE_NAMES[i];
            let value = if i == 2 { *v } else { *v };
            let cap = if i == 2 { 1.0 } else { 1.0 };
            let _ = name;
            value.clamp(-cap, cap)
        }).collect();
        let n = features_vec.len();
        let mut attention = vec![vec![0.0; n]; n];
        let mut attended = vec![0.0; n];
        for i in 0..n {
            let scores: Vec<f64> = (0..n).map(|j| features_vec[i] * features_vec[j] * self.attention_scale).collect();
            let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let exps: Vec<f64> = scores.iter().map(|v| (v - max_score).exp()).collect();
            let total: f64 = exps.iter().sum();
            attention[i] = if total.is_finite() && total > 0.0 {
                exps.iter().map(|v| v / total).collect()
            } else { vec![1.0 / n as f64; n] };
            attended[i] = (features_vec[i] + (0..n).map(|j| attention[i][j] * features_vec[j]).sum::<f64>()).clamp(-2.0, 2.0);
        }
        let weighted = |weights: &BTreeMap<String, f64>| -> f64 {
            FEATURE_NAMES.iter().enumerate().map(|(i, name)| weights[*name] * attended[i]).sum::<f64>().max(0.0)
        };
        let bullish = weighted(&self.bullish_weights);
        let bearish = weighted(&self.bearish_weights);
        let magnitude = (bullish * bullish + bearish * bearish).sqrt();
        let angle = bearish.atan2(bullish);
        let threshold = self.angular_threshold_degrees.to_radians();
        let spread_ok = spread_bps.map(|v| v.is_finite() && v >= 0.0 && v <= self.max_spread_bps).unwrap_or(false);
        let cost_ok = match (self.expected_edge_bps_per_magnitude, self.fee_bps, spread_bps, self.slippage_bps, self.cost_multiplier) {
            (Some(edge_per_magnitude), Some(fees), Some(spread), Some(slippage), Some(k))
                if edge_per_magnitude.is_finite() && fees.is_finite() && spread.is_finite() && slippage.is_finite() && k.is_finite() =>
                    magnitude * edge_per_magnitude > k * (fees + spread + slippage),
            _ => false,
        };
        let (action, reason) = if !spread_ok {
            ("wait", "spread missing or above the strategy map's liquidity gate".to_owned())
        } else if !cost_ok {
            ("wait", "cost gate failed: calibrated expected edge, fees, and slippage inputs are required".to_owned())
        } else if event_intensity <= 0.05 {
            ("wait", "no current event-intensity evidence".to_owned())
        } else if magnitude < self.magnitude_threshold {
            ("wait", "decision-vector magnitude below strategy threshold".to_owned())
        } else if angle <= threshold && bullish > bearish {
            ("buy", "vector is inside the mapped buy angle and passes magnitude/liquidity gates".to_owned())
        } else if (std::f64::consts::FRAC_PI_2 - angle).abs() <= threshold && bearish > bullish {
            ("sell", "vector is inside the mapped sell angle and passes magnitude/liquidity gates".to_owned())
        } else {
            ("wait", "vector is outside both mapped angular boundaries".to_owned())
        };
        let features = FEATURE_NAMES.iter().enumerate().map(|(i, name)| ((*name).to_owned(), features_vec[i])).collect();
        Ok(EdgeDecision {
            symbol: symbol.to_owned(), strategy_key: self.strategy_key.clone(),
            strategy_version: self.version.clone(), features, attention, attended_features: attended,
            decision_vector: [bullish, bearish], magnitude, angle_radians: angle,
            angular_threshold_radians: threshold, action, reason, risk_veto: None, research_only: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_map_is_complete_and_valid() { assert!(EdgeStrategy::baseline().validate().is_ok()); }
    #[test]
    fn missing_feature_weight_is_rejected() {
        let mut strategy = EdgeStrategy::baseline();
        strategy.bullish_weights.remove("order_flow_imbalance");
        assert!(strategy.validate().is_err());
    }
    #[test]
    fn missing_spread_abstains() {
        let s = EdgeStrategy::baseline();
        let d = s.decide("BTCUSDT", 0.9, None, None, 0.001, 4.0, 0.0, None).unwrap();
        assert_eq!(d.action, "wait");
    }
    #[test]
    fn missing_calibrated_cost_inputs_fail_closed() {
        let s = EdgeStrategy::baseline();
        let d = s.decide("BTCUSDT", 0.9, Some(0.0001), Some(2.0), 0.001, 4.0, 0.0, None).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("cost gate failed"));
    }

    #[test]
    fn attention_temperature_changes_attention_distribution() {
        let mut cold = EdgeStrategy::baseline();
        cold.attention_scale = 0.0;
        let mut focused = EdgeStrategy::baseline();
        focused.attention_scale = 6.0;
        for strategy in [&mut cold, &mut focused] {
            strategy.expected_edge_bps_per_magnitude = Some(100.0);
            strategy.fee_bps = Some(1.0);
            strategy.slippage_bps = Some(1.0);
            strategy.cost_multiplier = Some(2.0);
        }
        let a = cold.decide("BTCUSDT", 0.8, Some(0.0001), Some(2.0), 0.001, 4.0, 0.0, None).unwrap();
        let b = focused.decide("BTCUSDT", 0.8, Some(0.0001), Some(2.0), 0.001, 4.0, 0.0, None).unwrap();
        assert_ne!(a.attention, b.attention);
        for row in &b.attention {
            assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        }
    }

}
