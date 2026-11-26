//! Versioned edge-strategy map loaded from MongoDB and used by the Axum decision path.
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
    pub feature_scales: BTreeMap<String, f64>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureTrace {
    pub name: String,
    pub raw_value: f64,
    pub transform: String,
    pub scale: f64,
    pub normalized_value: f64,
    pub clamped_value: f64,
    pub bullish_weight: f64,
    pub bearish_weight: f64,
    pub bullish_weighted_term: f64,
    pub bearish_weighted_term: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionDiagnostics {
    pub spread_present: bool,
    pub spread_within_limit: bool,
    pub event_intensity_positive: bool,
    pub magnitude_passed: bool,
    pub buy_angle_passed: bool,
    pub sell_angle_passed: bool,
    pub bullish_exceeds_bearish: bool,
    pub bearish_exceeds_bullish: bool,
    pub spread_limit_bps: f64,
    pub magnitude_limit: f64,
    pub angle_limit_degrees: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionInputEvent {
    pub event_id: String,
    pub venue: String,
    pub event_type: String,
    pub event_ts_ms: i64,
    pub state_event_count: usize,
    pub state_last_event_ts_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeDecision {
    pub symbol: String,
    pub strategy_key: String,
    pub strategy_version: String,
    /// Exact raw observations passed into this strategy evaluation.
    pub input_event: Option<DecisionInputEvent>,
    pub liquidation_side: Option<String>,
    pub raw_inputs: BTreeMap<String, Option<f64>>,
    /// Full feature-by-feature normalization and weighted-term ledger.
    pub feature_trace: Vec<FeatureTrace>,
    /// Exact parameter snapshot used for this decision.
    pub strategy_parameters: EdgeStrategy,
    pub features: BTreeMap<String, f64>,
    pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>,
    pub decision_vector: [f64; 2],
    pub magnitude: f64,
    pub angle_radians: f64,
    pub angular_threshold_radians: f64,
    pub action: &'static str,
    pub reason: String,
    pub diagnostics: DecisionDiagnostics,
    pub angle_degrees: f64,
    pub research_only: bool,
}

pub const FEATURE_NAMES: [&str; 6] = [
    "order_flow_imbalance", "funding_pressure", "liquidity_stress",
    "volatility_shock", "event_intensity", "liquidation_pressure",
];

impl EdgeStrategy {
    pub fn baseline() -> Self {
        let bullish = [0.70, -0.15, -0.10, -0.08, 0.0, 0.10];
        let bearish = [-0.70, 0.15, -0.10, -0.08, 0.0, -0.10];
        let mut bullish_weights = BTreeMap::new();
        let mut bearish_weights = BTreeMap::new();
        let mut feature_scales = BTreeMap::new();
        for (i, name) in FEATURE_NAMES.iter().enumerate() {
            bullish_weights.insert((*name).to_owned(), bullish[i]);
            bearish_weights.insert((*name).to_owned(), bearish[i]);
            feature_scales.insert((*name).to_owned(), [1.0, 100.0, 500.0, 0.01, 10.0, 10.0][i]);
        }
        Self {
            strategy_key: STRATEGY_KEY.to_owned(),
            version: "edge-map-v1".to_owned(),
            status: "research_baseline".to_owned(),
            bullish_weights, bearish_weights,
            angular_threshold_degrees: 25.0,
            magnitude_threshold: 0.35,
            max_spread_bps: 25.0,
            attention_scale: 1.0,
            feature_scales,
            updated_at_ms: chrono::Utc::now().timestamp_millis(),
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
            || !(0.0..=10.0).contains(&self.attention_scale) {
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
        funding_rate: Option<f64>,
        spread_bps: Option<f64>,
        ewma_abs_return: f64,
        event_intensity: f64,
        liquidation_events: usize,
        last_liquidation_side: Option<&str>,
    ) -> Result<EdgeDecision, String> {
        self.validate()?;
        let scale = |name: &str| *self.feature_scales.get(name).expect("validated scale");
        let raw = [
            order_flow_imbalance,
            funding_rate.unwrap_or(0.0) * scale("funding_pressure"),
            spread_bps.unwrap_or(self.max_spread_bps * 4.0) / scale("liquidity_stress"),
            ewma_abs_return / scale("volatility_shock"),
            event_intensity / scale("event_intensity"),
            (liquidation_events as f64 / scale("liquidation_pressure"))
                * match last_liquidation_side { Some("long") => -1.0, Some("short") => 1.0, _ => 0.0 },
        ];
        let source_raw = [
            order_flow_imbalance,
            funding_rate.unwrap_or(0.0),
            spread_bps.unwrap_or(self.max_spread_bps * 4.0),
            ewma_abs_return,
            event_intensity,
            liquidation_events as f64,
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
        let (action, reason) = if !spread_ok {
            ("wait", "spread missing or above the strategy map's liquidity gate".to_owned())
        } else if event_intensity <= 0.0 {
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
        let raw_inputs = [
            ("order_flow_imbalance", Some(order_flow_imbalance)),
            ("funding_rate", funding_rate),
            ("spread_bps", spread_bps),
            ("ewma_abs_return", Some(ewma_abs_return)),
            ("event_intensity", Some(event_intensity)),
            ("liquidation_events", Some(liquidation_events as f64)),
        ].into_iter().map(|(name, value)| (name.to_owned(), value)).collect();
        let feature_trace = FEATURE_NAMES.iter().enumerate().map(|(i, name)| {
            let raw_value = source_raw[i];
            let scale = match *name {
                "order_flow_imbalance" => 1.0,
                "funding_pressure" => scale("funding_pressure"),
                "liquidity_stress" => scale("liquidity_stress"),
                "volatility_shock" => scale("volatility_shock"),
                "event_intensity" => scale("event_intensity"),
                "liquidation_pressure" => scale("liquidation_pressure"),
                _ => 1.0,
            };
            let transform = match *name {
                "order_flow_imbalance" => "identity",
                "funding_pressure" => "multiply_then_clamp",
                "liquidity_stress" => "divide_then_clamp",
                "volatility_shock" => "divide_then_clamp",
                "event_intensity" => "divide_then_clamp",
                "liquidation_pressure" => "divide_apply_side_sign_then_clamp",
                _ => "identity",
            }.to_owned();
            let bullish_weight = self.bullish_weights[*name];
            let bearish_weight = self.bearish_weights[*name];
            FeatureTrace {
                name: (*name).to_owned(),
                raw_value,
                transform,
                scale,
                normalized_value: raw[i],
                clamped_value: features_vec[i],
                bullish_weight,
                bearish_weight,
                bullish_weighted_term: bullish_weight * attended[i],
                bearish_weighted_term: bearish_weight * attended[i],
            }
        }).collect();
        let diagnostics = DecisionDiagnostics {
            spread_present: spread_bps.is_some(),
            spread_within_limit: spread_ok,
            event_intensity_positive: event_intensity > 0.0,
            magnitude_passed: magnitude >= self.magnitude_threshold,
            buy_angle_passed: angle <= threshold,
            sell_angle_passed: (std::f64::consts::FRAC_PI_2 - angle).abs() <= threshold,
            bullish_exceeds_bearish: bullish > bearish,
            bearish_exceeds_bullish: bearish > bullish,
            spread_limit_bps: self.max_spread_bps,
            magnitude_limit: self.magnitude_threshold,
            angle_limit_degrees: self.angular_threshold_degrees,
        };
        let features = FEATURE_NAMES.iter().enumerate().map(|(i, name)| ((*name).to_owned(), features_vec[i])).collect();
        Ok(EdgeDecision {
            symbol: symbol.to_owned(), strategy_key: self.strategy_key.clone(),
            strategy_version: self.version.clone(), input_event: None,
            liquidation_side: last_liquidation_side.map(str::to_owned), raw_inputs, feature_trace,
            strategy_parameters: self.clone(), features, attention, attended_features: attended,
            decision_vector: [bullish, bearish], magnitude, angle_radians: angle,
            angular_threshold_radians: threshold, action, reason, diagnostics,
            angle_degrees: angle.to_degrees(), research_only: true,
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
        let d = s.decide("BTCUSDT", 0.9, None, None, 0.001, 4.0, 0, None).unwrap();
        assert_eq!(d.action, "wait");
    }

    #[test]
    fn decision_exposes_reproducible_feature_and_gate_ledger() {
        let s = EdgeStrategy::baseline();
        let d = s.decide("BTCUSDT", 0.4, Some(0.001), Some(3.0), 0.001, 4.0, 1, Some("short")).unwrap();
        assert_eq!(d.feature_trace.len(), FEATURE_NAMES.len());
        assert_eq!(d.feature_trace[0].raw_value, 0.4);
        assert_eq!(d.feature_trace[1].normalized_value, 0.1);
        assert_eq!(d.feature_trace[1].clamped_value, 0.1);
        assert_eq!(d.feature_trace[0].bullish_weight, s.bullish_weights["order_flow_imbalance"]);
        assert_eq!(d.strategy_parameters.version, s.version);
        assert!(d.diagnostics.spread_present);
        assert!(d.diagnostics.spread_within_limit);
        assert_eq!(d.angle_degrees, d.angle_radians.to_degrees());
        assert_eq!(d.liquidation_side.as_deref(), Some("short"));
    }
}
