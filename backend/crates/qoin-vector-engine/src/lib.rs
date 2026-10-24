//! Deterministic, research-stage market-force vector decision engine.
//!
//! This crate emits signals only. It does not size positions, place orders, or
//! claim profitability. Inputs must be causal and normalized using past-only
//! statistics by the caller. All coefficients are explicit baseline parameters,
//! not trained weights and not evidence of an edge.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ForceObservation {
    pub ofi: f64,
    pub liquidity: f64,
    pub volatility: f64,
    pub funding: f64,
    pub open_interest_change: f64,
    pub liquidation_pressure: f64,
    pub basis: f64,
}

impl ForceObservation {
    fn values(self) -> [f64; 7] {
        [self.ofi, self.liquidity, self.volatility, self.funding,
         self.open_interest_change, self.liquidation_pressure, self.basis]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Buy,
    Sell,
    Close,
    Wait,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Minimum absolute directional score to consider a trade.
    pub signal_threshold: f64,
    /// Lower exit threshold; must be strictly below the entry threshold.
    pub exit_threshold: f64,
    /// Maximum allowed absolute input after normalization/clipping.
    pub input_clip: f64,
    /// Maximum absolute output score.
    pub score_clip: f64,
    /// Number of strongest absolute forces retained by Pareto selection.
    pub top_k: usize,
    /// Minimum cosine alignment between observed and expected force vectors.
    pub min_alignment: f64,
    /// Maximum angular deviation in radians for a directional entry.
    pub max_angle_radians: f64,
    /// Volatility dampening coefficient.
    pub volatility_dampener: f64,
    /// Small regularized OFI × OI interaction coefficient.
    pub interaction_coefficient: f64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            signal_threshold: 0.65,
            exit_threshold: 0.25,
            input_clip: 3.0,
            score_clip: 1.0,
            top_k: 5,
            min_alignment: 0.65,
            max_angle_radians: 0.80,
            volatility_dampener: 0.15,
            interaction_coefficient: 0.025,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    pub decision: Decision,
    pub score: f64,
    pub alignment: f64,
    pub angle_radians: f64,
    pub active_force_count: usize,
    pub config_version: String,
    pub rationale: String,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum EngineError {
    #[error("all observations must be finite")]
    NonFiniteInput,
    #[error("invalid engine configuration")]
    InvalidConfig,
}

fn valid_config(config: &EngineConfig) -> bool {
    config.signal_threshold.is_finite()
        && config.exit_threshold.is_finite()
        && config.exit_threshold > 0.0
        && config.exit_threshold < config.signal_threshold
        && config.input_clip.is_finite()
        && config.score_clip.is_finite()
        && config.min_alignment.is_finite()
        && config.max_angle_radians.is_finite()
        && config.volatility_dampener.is_finite()
        && config.interaction_coefficient.is_finite()
        && config.signal_threshold > 0.0
        && config.input_clip > 0.0
        && config.score_clip > 0.0
        && config.score_clip <= 1.0
        && (1..=7).contains(&config.top_k)
        && (-1.0..=1.0).contains(&config.min_alignment)
        && (0.0..=std::f64::consts::PI).contains(&config.max_angle_radians)
        && config.volatility_dampener >= 0.0
        && config.interaction_coefficient.abs() <= 0.25
}

pub fn decide(
    input: ForceObservation,
    config: &EngineConfig,
    config_version: &str,
    has_open_position: bool,
) -> Result<Signal, EngineError> {
    let raw = input.values();
    if raw.iter().any(|v| !v.is_finite()) {
        return Err(EngineError::NonFiniteInput);
    }
    if !valid_config(config) {
        return Err(EngineError::InvalidConfig);
    }

    // Expected directional template. Volatility is context, not a direction.
    let weights = [0.30_f64, 0.18, 0.0, -0.10, 0.12, 0.12, -0.08];
    let clip = |v: f64| v.clamp(-config.input_clip, config.input_clip);
    let mut values = raw.map(clip);

    // Pareto-style top-k attention: retain the strongest k observed magnitudes.
    // This is a bounded heuristic, not a learned Pareto frontier.
    let mut rank: Vec<(usize, f64)> = values.iter().enumerate()
        .map(|(i, v)| (i, v.abs())).collect();
    rank.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut selected = [false; 7];
    for (index, magnitude) in rank.into_iter().take(config.top_k) {
        if magnitude > 0.0 { selected[index] = true; }
    }
    for i in 0..7 {
        if !selected[i] { values[i] = 0.0; }
    }

    let active_force_count = selected.iter().filter(|v| **v).count();
    let mut dot = 0.0;
    let mut observed_norm_sq = 0.0;
    let mut template_norm_sq = 0.0;
    let mut directional = 0.0;
    for i in 0..7 {
        dot += values[i] * weights[i];
        observed_norm_sq += values[i] * values[i];
        template_norm_sq += weights[i] * weights[i];
        directional += values[i] * weights[i];
    }
    // A small interaction captures flow/positioning reinforcement but is capped.
    let interaction = (config.interaction_coefficient * values[0] * values[4])
        .clamp(-0.25, 0.25);
    directional += interaction;

    let observed_norm = observed_norm_sq.sqrt();
    let template_norm = template_norm_sq.sqrt();
    let alignment = if observed_norm <= f64::EPSILON || template_norm <= f64::EPSILON {
        0.0
    } else {
        (dot / (observed_norm * template_norm)).clamp(-1.0, 1.0)
    };
    let angle_radians = alignment.acos();
    let dampener = 1.0 / (1.0 + values[2].abs() * config.volatility_dampener);
    let score = (directional * dampener).clamp(-config.score_clip, config.score_clip);

    // Angular and magnitude gates must both pass before a new directional entry.
    let buy_aligned = alignment >= config.min_alignment
        && angle_radians <= config.max_angle_radians;
    let sell_aligned = alignment <= -config.min_alignment
        && (std::f64::consts::PI - angle_radians) <= config.max_angle_radians;
    let decision = if score >= config.signal_threshold && buy_aligned {
        Decision::Buy
    } else if score <= -config.signal_threshold && sell_aligned {
        Decision::Sell
    } else if has_open_position && score.abs() < config.exit_threshold {
        Decision::Close
    } else {
        Decision::Wait
    };
    let rationale = if has_open_position && score.abs() < config.exit_threshold {
        "CLOSE: score crossed the lower exit threshold; entry and exit use hysteresis".to_owned()
    } else if !buy_aligned && !sell_aligned {
        "WAIT: force vector failed directional angular convergence; research baseline only".to_owned()
    } else {
        "research baseline; top-k force attention; signal only; independent risk gate required".to_owned()
    };

    Ok(Signal {
        decision,
        score,
        alignment,
        angle_radians,
        active_force_count,
        config_version: config_version.to_owned(),
        rationale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ForceObservation {
        ForceObservation {
            ofi: 2.0, liquidity: 1.0, volatility: 0.2, funding: -0.5,
            open_interest_change: 1.0, liquidation_pressure: 0.5, basis: 0.0,
        }
    }

    #[test]
    fn strong_positive_force_produces_buy() {
        assert_eq!(decide(sample(), &EngineConfig::default(), "baseline-v2", false).unwrap().decision, Decision::Buy);
    }

    #[test]
    fn weak_signal_without_position_waits() {
        let input = ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0, open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(result.decision, Decision::Wait);
        assert_eq!(result.active_force_count, 0);
    }

    #[test]
    fn non_finite_values_are_rejected() {
        let mut input = sample();
        input.ofi = f64::NAN;
        assert_eq!(decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap_err(), EngineError::NonFiniteInput);
    }

    #[test]
    fn strong_negative_force_produces_sell() {
        let input = ForceObservation {
            ofi: -2.0, liquidity: -1.0, volatility: 0.2, funding: 0.5,
            open_interest_change: -1.0, liquidation_pressure: -0.5, basis: 0.0,
        };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(result.decision, Decision::Sell);
    }

    #[test]
    fn invalid_top_k_is_rejected() {
        let mut config = EngineConfig::default();
        config.top_k = 0;
        assert_eq!(decide(sample(), &config, "baseline-v2", false).unwrap_err(), EngineError::InvalidConfig);
    }

    #[test]
    fn open_position_does_not_close_at_every_sub_entry_threshold_tick() {
        let input = ForceObservation { ofi: 1.2, liquidity: 0.0, volatility: 0.0, funding: 0.0, open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 };
        let result = decide(input, &EngineConfig::default(), "hysteresis-test", true).unwrap();
        assert_eq!(result.decision, Decision::Wait);
    }

    #[test]
    fn open_position_closes_only_below_exit_threshold() {
        let input = ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0, open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 };
        let result = decide(input, &EngineConfig::default(), "hysteresis-test", true).unwrap();
        assert_eq!(result.decision, Decision::Close);
    }

    #[test]
    fn exit_threshold_must_be_below_entry_threshold() {
        let mut config = EngineConfig::default();
        config.exit_threshold = config.signal_threshold;
        assert_eq!(decide(sample(), &config, "bad-hysteresis", false).unwrap_err(), EngineError::InvalidConfig);
    }

    #[test]
    fn opposing_force_vector_does_not_trigger_entry() {
        let input = ForceObservation { ofi: -2.5, liquidity: -2.0, volatility: 0.1, funding: 1.5, open_interest_change: -2.0, liquidation_pressure: -1.5, basis: 1.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_ne!(result.decision, Decision::Buy);
    }
}


// The persisted strategy-map decision implementation lives in this crate so
// services cannot fork or shadow the canonical decision logic.
// Versioned edge-strategy map loaded from MongoDB and used by the Axum decision path.

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
    #[serde(default = "default_min_persistence_ticks")]
    pub min_persistence_ticks: usize,
    #[serde(default = "default_cost_multiplier")]
    pub cost_multiplier: f64,
    #[serde(default = "default_max_state_age_ms")]
    pub max_state_age_ms: i64,
    #[serde(default = "default_min_state_quality")]
    pub min_state_quality: f64,
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
    pub candidate_action: &'static str,
    pub action: &'static str,
    pub reason: String,
    pub research_only: bool,
}

pub const EDGE_FEATURE_NAMES: [&str; 6] = [
    "order_flow_imbalance", "funding_pressure", "liquidity_stress",
    "volatility_shock", "event_intensity", "liquidation_pressure",
];

fn default_min_persistence_ticks() -> usize { 3 }
fn default_cost_multiplier() -> f64 { 1.5 }
fn default_max_state_age_ms() -> i64 { 5_000 }
fn default_min_state_quality() -> f64 { 0.2 }

#[derive(Debug, Clone, Default)]
pub struct DecisionContext {
    pub expected_edge_bps: Option<f64>,
    pub fee_bps: Option<f64>,
    pub slippage_bps: Option<f64>,
    pub state_age_ms: Option<i64>,
    pub state_quality: Option<f64>,
    pub persistence_ticks: usize,
    pub feed_sequence_valid: bool,
    pub regime_allowed: bool,
}

impl EdgeStrategy {
    pub fn baseline() -> Self {
        let bullish = [0.70, -0.15, -0.10, -0.08, 0.0, 0.10];
        let bearish = [-0.70, 0.15, -0.10, -0.08, 0.0, -0.10];
        let mut bullish_weights = BTreeMap::new();
        let mut bearish_weights = BTreeMap::new();
        let mut feature_scales = BTreeMap::new();
        for (i, name) in EDGE_FEATURE_NAMES.iter().enumerate() {
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
            attention_scale: 6.0,
            feature_scales,
            updated_at_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0),
            min_persistence_ticks: default_min_persistence_ticks(),
            cost_multiplier: default_cost_multiplier(),
            max_state_age_ms: default_max_state_age_ms(),
            min_state_quality: default_min_state_quality(),
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
            || self.min_persistence_ticks == 0
            || !self.cost_multiplier.is_finite() || self.cost_multiplier < 1.0
            || self.max_state_age_ms <= 0
            || !self.min_state_quality.is_finite() || !(0.0..=1.0).contains(&self.min_state_quality) {
            return Err("invalid strategy thresholds or attention_scale".into());
        }
        for name in EDGE_FEATURE_NAMES {
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
        liquidation_events: f64,
        last_liquidation_side: Option<&str>,
    ) -> Result<EdgeDecision, String> {
        self.decide_with_context(symbol, order_flow_imbalance, funding_rate, spread_bps,
            ewma_abs_return, event_intensity, liquidation_events, last_liquidation_side,
            &DecisionContext::default())
    }

    pub fn decide_with_context(
        &self,
        symbol: &str,
        order_flow_imbalance: f64,
        funding_rate: Option<f64>,
        spread_bps: Option<f64>,
        ewma_abs_return: f64,
        event_intensity: f64,
        liquidation_events: f64,
        last_liquidation_side: Option<&str>,
        context: &DecisionContext,
    ) -> Result<EdgeDecision, String> {
        self.validate()?;
        let scale = |name: &str| *self.feature_scales.get(name).expect("validated scale");
        let raw = [
            order_flow_imbalance,
            funding_rate.unwrap_or(0.0) * scale("funding_pressure"),
            spread_bps.unwrap_or(self.max_spread_bps * 4.0) / scale("liquidity_stress"),
            ewma_abs_return / scale("volatility_shock"),
            event_intensity / scale("event_intensity"),
            (liquidation_events / scale("liquidation_pressure"))
                * match last_liquidation_side { Some("long") => -1.0, Some("short") => 1.0, _ => 0.0 },
        ];
        if raw.iter().any(|v| !v.is_finite()) { return Err("non-finite feature input".into()); }
        let features_vec: Vec<f64> = raw.iter().enumerate().map(|(i, v)| {
            let name = EDGE_FEATURE_NAMES[i];
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
            EDGE_FEATURE_NAMES.iter().enumerate().map(|(i, name)| weights[*name] * attended[i]).sum::<f64>().max(0.0)
        };
        let bullish = weighted(&self.bullish_weights);
        let bearish = weighted(&self.bearish_weights);
        let magnitude = (bullish * bullish + bearish * bearish).sqrt();
        let angle = bearish.atan2(bullish);
        let threshold = self.angular_threshold_degrees.to_radians();
        let spread_ok = spread_bps.map(|v| v.is_finite() && v >= 0.0 && v <= self.max_spread_bps).unwrap_or(false);
        let candidate_action = if magnitude >= self.magnitude_threshold && angle <= threshold && bullish > bearish {
            "buy"
        } else if magnitude >= self.magnitude_threshold && (std::f64::consts::FRAC_PI_2 - angle).abs() <= threshold && bearish > bullish {
            "sell"
        } else { "wait" };
        let expected_edge = context.expected_edge_bps.filter(|v| v.is_finite() && *v > 0.0);
        let fees = context.fee_bps.filter(|v| v.is_finite() && *v >= 0.0);
        let slippage = context.slippage_bps.filter(|v| v.is_finite() && *v >= 0.0);
        let cost_gate = match (expected_edge, fees, spread_bps.filter(|v| v.is_finite() && *v >= 0.0), slippage) {
            (Some(edge), Some(fee), Some(spread), Some(slip)) => edge > self.cost_multiplier * (fee + spread + slip),
            _ => false,
        };
        let age_ok = context.state_age_ms.map(|age| age >= 0 && age <= self.max_state_age_ms).unwrap_or(false);
        let quality_ok = context.state_quality.map(|q| q.is_finite() && q >= self.min_state_quality && q <= 1.0).unwrap_or(false);
        let (action, reason) = if !spread_ok {
            ("wait", "spread missing or above the strategy map's liquidity gate".to_owned())
        } else if event_intensity < 0.1 {
            ("wait", "decayed event intensity is below the recent-evidence floor".to_owned())
        } else if !age_ok {
            ("wait", "state is stale or event-to-arrival latency is unavailable".to_owned())
        } else if !quality_ok {
            ("wait", "state quality is below the configured floor".to_owned())
        } else if !context.feed_sequence_valid {
            ("wait", "feed sequence continuity is unknown or invalid".to_owned())
        } else if context.persistence_ticks < self.min_persistence_ticks {
            ("wait", "candidate has not persisted for the required number of ticks".to_owned())
        } else if !context.regime_allowed {
            ("wait", "regime gate is not explicitly satisfied".to_owned())
        } else if !cost_gate {
            ("wait", "expected edge does not exceed configured multiple of fees, spread, and slippage".to_owned())
        } else if candidate_action == "buy" {
            ("buy", "all entry gates passed for the bullish vector candidate".to_owned())
        } else if candidate_action == "sell" {
            ("sell", "all entry gates passed for the bearish vector candidate".to_owned())
        } else {
            ("wait", "vector is outside both mapped angular boundaries".to_owned())
        };
        let features = EDGE_FEATURE_NAMES.iter().enumerate().map(|(i, name)| ((*name).to_owned(), features_vec[i])).collect();
        Ok(EdgeDecision {
            symbol: symbol.to_owned(), strategy_key: self.strategy_key.clone(),
            strategy_version: self.version.clone(), features, attention, attended_features: attended,
            decision_vector: [bullish, bearish], magnitude, angle_radians: angle,
            angular_threshold_radians: threshold, candidate_action, action, reason, research_only: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn passing_context() -> DecisionContext {
        DecisionContext { expected_edge_bps: Some(50.0), fee_bps: Some(2.0), slippage_bps: Some(2.0), state_age_ms: Some(100), state_quality: Some(0.9), persistence_ticks: 3, regime_allowed: true }
    }
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
    fn decayed_event_intensity_below_floor_abstains() {
        let s = EdgeStrategy::baseline();
        let d = s.decide("BTCUSDT", 0.9, None, Some(1.0), 0.001, 0.05, 0.0, None).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("recent-evidence"));
    }
    #[test]
    fn persistence_gate_rejects_insufficient_ticks() {
        let s = EdgeStrategy::baseline();
        let mut c = passing_context(); c.persistence_ticks = 1;
        let d = s.decide_with_context("BTCUSDT", 0.9, Some(0.001), Some(1.0), 0.001, 4.0, 0.0, None, &c).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("persisted"));
    }
    #[test]
    fn cost_gate_rejects_edge_below_cost_multiple() {
        let s = EdgeStrategy::baseline();
        let mut c = passing_context(); c.expected_edge_bps = Some(3.0);
        let d = s.decide_with_context("BTCUSDT", 0.9, Some(0.001), Some(1.0), 0.001, 4.0, 0.0, None, &c).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("expected edge"));
    }

    #[test]
    fn regime_gate_rejects_when_not_explicitly_allowed() {
        let s = EdgeStrategy::baseline();
        let mut c = passing_context(); c.regime_allowed = false;
        let d = s.decide_with_context("BTCUSDT", 0.9, Some(0.001), Some(1.0), 0.001, 4.0, 0.0, None, &c).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("regime"));
    }
    #[test]
    fn stale_state_rejects_candidate() {
        let s = EdgeStrategy::baseline();
        let mut c = passing_context(); c.state_age_ms = Some(5_001);
        let d = s.decide_with_context("BTCUSDT", 0.9, Some(0.001), Some(1.0), 0.001, 4.0, 0.0, None, &c).unwrap();
        assert_eq!(d.action, "wait");
        assert!(d.reason.contains("stale"));
    }
    #[test]
    fn attention_scale_ablation_changes_attention_weights() {
        let mut selective = EdgeStrategy::baseline();
        let mut uniform = EdgeStrategy::baseline();
        uniform.attention_scale = 0.0;
        let a = selective.decide_with_context("BTCUSDT", 0.8, Some(0.001), Some(2.0), 0.002, 4.0, 0.0, None, &passing_context()).unwrap();
        let b = uniform.decide_with_context("BTCUSDT", 0.8, Some(0.001), Some(2.0), 0.002, 4.0, 0.0, None, &passing_context()).unwrap();
        assert_ne!(a.attention, b.attention);
    }

}
