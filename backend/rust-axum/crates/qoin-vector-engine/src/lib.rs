//! Deterministic, research-stage seven-force vector decision engine.
//!
//! This crate is the authoritative signal calculation used by the live market
//! ingestion path. It emits research signals only: it does not size positions,
//! place orders, or claim profitability. Inputs must be causal and normalized
//! using past-only statistics by the caller.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FEATURE_NAMES: [&str; 7] = [
    "order_flow_imbalance",
    "liquidity_imbalance",
    "volatility",
    "funding_pressure",
    "open_interest_change",
    "liquidation_pressure",
    "basis",
];

const DIRECTIONAL_WEIGHTS: [f64; 7] = [0.30, 0.18, 0.0, -0.10, 0.0, 0.12, -0.08];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ForceObservation {
    pub ofi: f64,
    pub liquidity: f64,
    pub volatility: f64,
    pub funding: f64,
    pub open_interest_change: f64,
    pub liquidation_pressure: f64,
    pub basis: f64,
    /// Current best-bid/ask spread in basis points. Missing spread always
    /// causes an abstention; it is not silently treated as a liquid market.
    #[serde(default)]
    pub spread_bps: Option<f64>,
    /// Age of the quote used for the spread, in milliseconds.
    #[serde(default)]
    pub spread_age_ms: Option<i64>,
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
    pub signal_threshold: f64,
    pub input_clip: f64,
    pub score_clip: f64,
    pub top_k: usize,
    pub min_alignment: f64,
    pub max_angle_radians: f64,
    pub volatility_dampener: f64,
    pub interaction_coefficient: f64,
    pub max_spread_bps: f64,
    pub max_spread_age_ms: i64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            signal_threshold: 0.65,
            input_clip: 3.0,
            score_clip: 1.0,
            top_k: 5,
            min_alignment: 0.15,
            max_angle_radians: 1.42,
            volatility_dampener: 0.15,
            interaction_coefficient: 0.025,
            max_spread_bps: 25.0,
            max_spread_age_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureTrace {
    pub name: String,
    pub raw_value: f64,
    pub transform: String,
    pub scale: f64,
    pub normalized_value: f64,
    pub clamped_value: f64,
    pub selected: bool,
    pub attended_value: f64,
    pub bullish_weight: f64,
    pub bearish_weight: f64,
    pub bullish_weighted_term: f64,
    pub bearish_weighted_term: f64,
    pub directional_weight: f64,
    pub directional_contribution: f64,
    pub interaction_contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionDiagnostics {
    pub spread_present: bool,
    pub spread_within_limit: bool,
    pub spread_fresh: bool,
    pub alignment_passed: bool,
    pub magnitude_passed: bool,
    pub angle_passed: bool,
    pub active_force_count: usize,
    pub spread_limit_bps: f64,
    pub signal_threshold: f64,
    pub alignment_threshold: f64,
    pub angle_limit_radians: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    pub decision: Decision,
    pub action: String,
    pub score: f64,
    pub alignment: f64,
    pub angle_radians: f64,
    pub angle_degrees: f64,
    pub active_force_count: usize,
    pub config_version: String,
    pub rationale: String,
    pub feature_names: Vec<&'static str>,
    pub raw_inputs: BTreeMap<String, f64>,
    pub features: BTreeMap<String, f64>,
    pub attention: Vec<Vec<f64>>,
    pub attended_features: Vec<f64>,
    pub feature_trace: Vec<FeatureTrace>,
    /// Positive and negative directional score components, respectively.
    pub decision_vector: [f64; 2],
    pub magnitude: f64,
    pub interaction_contribution: f64,
    pub diagnostics: DecisionDiagnostics,
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
        && config.input_clip.is_finite()
        && config.score_clip.is_finite()
        && config.min_alignment.is_finite()
        && config.max_angle_radians.is_finite()
        && config.volatility_dampener.is_finite()
        && config.interaction_coefficient.is_finite()
        && config.max_spread_bps.is_finite()
        && config.max_spread_age_ms > 0
        && config.signal_threshold > 0.0
        && config.input_clip > 0.0
        && config.score_clip > 0.0
        && config.score_clip <= 1.0
        && (1..=7).contains(&config.top_k)
        && (-1.0..=1.0).contains(&config.min_alignment)
        && (0.0..=std::f64::consts::PI).contains(&config.max_angle_radians)
        && config.volatility_dampener >= 0.0
        && config.interaction_coefficient.abs() <= 0.25
        && config.max_spread_bps > 0.0
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

/// Evaluate one causally-normalized seven-force observation.
pub fn decide(
    input: ForceObservation,
    config: &EngineConfig,
    config_version: &str,
    has_open_position: bool,
) -> Result<Signal, EngineError> {
    let raw = input.values();
    if raw.iter().any(|v| !v.is_finite())
        || input.spread_bps.is_some_and(|v| !v.is_finite() || v < 0.0)
        || input.spread_age_ms.is_some_and(|age| age < 0)
    {
        return Err(EngineError::NonFiniteInput);
    }
    if !valid_config(config) {
        return Err(EngineError::InvalidConfig);
    }

    // Clip before attention so a single corrupt/outlier observation cannot
    // dominate the matrix or overflow its exponentials.
    let clipped = raw.map(|value| value.clamp(-config.input_clip, config.input_clip));
    let mut attention = vec![vec![0.0; FEATURE_NAMES.len()]; FEATURE_NAMES.len()];
    let mut attended = vec![0.0; FEATURE_NAMES.len()];
    for row in 0..FEATURE_NAMES.len() {
        let scores: Vec<f64> = (0..FEATURE_NAMES.len())
            .map(|column| clipped[row] * clipped[column])
            .collect();
        attention[row] = softmax(&scores);
        attended[row] = clipped[row]
            + (0..FEATURE_NAMES.len())
                .map(|column| attention[row][column] * clipped[column])
                .sum::<f64>();
    }

    // Pareto-style top-k selection is applied to attended force magnitudes.
    // It is a deterministic heuristic, not a learned Pareto frontier.
    let mut rank: Vec<(usize, f64)> = attended.iter().enumerate()
        .map(|(index, value)| (index, value.abs())).collect();
    rank.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut selected = [false; 7];
    for (index, magnitude) in rank.into_iter().take(config.top_k) {
        if magnitude > 0.0 { selected[index] = true; }
    }
    for index in 0..FEATURE_NAMES.len() {
        if !selected[index] { attended[index] = 0.0; }
    }

    let active_force_count = selected.iter().filter(|selected| **selected).count();
    let interaction = (config.interaction_coefficient * attended[0] * attended[4].abs())
        .clamp(-0.25, 0.25);
    let directional = (0..FEATURE_NAMES.len())
        .map(|index| attended[index] * DIRECTIONAL_WEIGHTS[index])
        .sum::<f64>() + interaction;
    let dampener = 1.0 / (1.0 + attended[2].abs() * config.volatility_dampener);
    let score = (directional * dampener).clamp(-config.score_clip, config.score_clip);

    let observed_norm = attended.iter().map(|value| value * value).sum::<f64>().sqrt();
    let template_norm = DIRECTIONAL_WEIGHTS.iter().map(|value| value * value).sum::<f64>().sqrt();
    let dot = (0..FEATURE_NAMES.len())
        .map(|index| attended[index] * DIRECTIONAL_WEIGHTS[index])
        .sum::<f64>();
    let alignment = if observed_norm <= f64::EPSILON || template_norm <= f64::EPSILON {
        0.0
    } else {
        (dot / (observed_norm * template_norm)).clamp(-1.0, 1.0)
    };
    let angle_radians = alignment.acos();
    let spread_within_limit = input.spread_bps
        .is_some_and(|spread| spread <= config.max_spread_bps);
    let spread_fresh = input.spread_age_ms
        .is_some_and(|age| age <= config.max_spread_age_ms);
    let market_quality_ok = spread_within_limit && spread_fresh;
    let alignment_passed = alignment.abs() >= config.min_alignment;
    let magnitude_passed = score.abs() >= config.signal_threshold;
    let angle_passed = if score >= 0.0 {
        angle_radians <= config.max_angle_radians
    } else {
        (std::f64::consts::PI - angle_radians) <= config.max_angle_radians
    };

    let decision = if !market_quality_ok {
        Decision::Wait
    } else if magnitude_passed && alignment_passed && angle_passed && score > 0.0 {
        Decision::Buy
    } else if magnitude_passed && alignment_passed && angle_passed && score < 0.0 {
        Decision::Sell
    } else if has_open_position {
        Decision::Close
    } else {
        Decision::Wait
    };
    let action = match decision {
        Decision::Buy => "buy",
        Decision::Sell => "sell",
        Decision::Close => "close",
        Decision::Wait => "wait",
    }.to_owned();
    let rationale = if !spread_within_limit {
        "WAIT: current spread is missing or exceeds the configured liquidity limit".to_owned()
    } else if !spread_fresh {
        "WAIT: the quote used for the spread is missing or stale".to_owned()
    } else if !magnitude_passed {
        "WAIT: directional force magnitude is below the configured threshold".to_owned()
    } else if !alignment_passed || !angle_passed {
        "WAIT: the observed force vector does not converge with the directional template".to_owned()
    } else if decision == Decision::Close {
        "CLOSE: existing-position handling requested a close after the entry gates failed".to_owned()
    } else {
        "Research signal: seven-force vector passed spread, magnitude and angular gates; independent portfolio risk checks remain mandatory".to_owned()
    };

    let features = FEATURE_NAMES.iter().enumerate()
        .map(|(index, name)| ((*name).to_owned(), clipped[index]))
        .collect::<BTreeMap<_, _>>();
    let raw_inputs = FEATURE_NAMES.iter().enumerate()
        .map(|(index, name)| ((*name).to_owned(), raw[index]))
        .collect::<BTreeMap<_, _>>();
    let feature_trace = FEATURE_NAMES.iter().enumerate().map(|(index, name)| {
        let (transform, scale) = match *name {
            "order_flow_imbalance" => ("identity_clip", 1.0),
            "liquidity_imbalance" => ("bid_ask_depth_imbalance", 1.0),
            "volatility" => ("ewma_abs_return_div_0_01", 0.01),
            "funding_pressure" => ("funding_rate_times_100", 100.0),
            "open_interest_change" => ("relative_change_times_100", 100.0),
            "liquidation_pressure" => ("signed_count_div_10", 10.0),
            "basis" => ("basis_bps_div_25", 25.0),
            _ => ("identity", 1.0),
        };
        FeatureTrace {
            name: (*name).to_owned(),
            raw_value: raw[index],
            transform: transform.to_owned(),
            scale,
            normalized_value: raw[index],
            clamped_value: clipped[index],
            selected: selected[index],
            attended_value: attended[index],
            bullish_weight: DIRECTIONAL_WEIGHTS[index],
            bearish_weight: -DIRECTIONAL_WEIGHTS[index],
            bullish_weighted_term: attended[index] * DIRECTIONAL_WEIGHTS[index],
            bearish_weighted_term: attended[index] * -DIRECTIONAL_WEIGHTS[index],
            directional_weight: DIRECTIONAL_WEIGHTS[index],
            directional_contribution: attended[index] * DIRECTIONAL_WEIGHTS[index],
            interaction_contribution: if index == 4 { interaction } else { 0.0 },
        }
    }).collect();

    Ok(Signal {
        decision,
        action,
        score,
        alignment,
        angle_radians,
        angle_degrees: angle_radians.to_degrees(),
        active_force_count,
        config_version: config_version.to_owned(),
        rationale,
        feature_names: FEATURE_NAMES.to_vec(),
        raw_inputs,
        features,
        attention,
        attended_features: attended.to_vec(),
        feature_trace,
        decision_vector: [score.max(0.0), (-score).max(0.0)],
        magnitude: score.abs(),
        interaction_contribution: interaction,
        diagnostics: DecisionDiagnostics {
            spread_present: input.spread_bps.is_some(),
            spread_within_limit,
            spread_fresh,
            alignment_passed,
            magnitude_passed,
            angle_passed,
            active_force_count,
            spread_limit_bps: config.max_spread_bps,
            signal_threshold: config.signal_threshold,
            alignment_threshold: config.min_alignment,
            angle_limit_radians: config.max_angle_radians,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ForceObservation {
        ForceObservation {
            ofi: 2.0, liquidity: 1.0, volatility: 0.2, funding: -0.5,
            open_interest_change: 1.0, liquidation_pressure: 0.5, basis: 0.0,
            spread_bps: Some(2.0), spread_age_ms: Some(0),
        }
    }

    #[test]
    fn strong_positive_force_produces_buy() {
        assert_eq!(decide(sample(), &EngineConfig::default(), "baseline-v2", false).unwrap().decision, Decision::Buy);
    }

    #[test]
    fn weak_signal_without_position_waits() {
        let input = ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0, spread_bps: Some(1.0), spread_age_ms: Some(0) };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(result.decision, Decision::Wait);
        assert_eq!(result.active_force_count, 0);
    }

    #[test]
    fn missing_or_wide_spread_always_abstains() {
        let mut input = sample();
        input.spread_bps = None;
        assert_eq!(decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap().decision, Decision::Wait);
        input.spread_bps = Some(30.0);
        assert_eq!(decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap().decision, Decision::Wait);
        input.spread_bps = Some(2.0);
        input.spread_age_ms = Some(30_001);
        let stale = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(stale.decision, Decision::Wait);
        assert!(!stale.diagnostics.spread_fresh);
    }

    #[test]
    fn non_finite_values_are_rejected() {
        let mut input = sample();
        input.ofi = f64::NAN;
        assert_eq!(decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap_err(), EngineError::NonFiniteInput);
    }

    #[test]
    fn strong_negative_force_produces_sell() {
        let input = ForceObservation { ofi: -2.0, liquidity: -1.0, volatility: 0.2, funding: 0.5,
            open_interest_change: -1.0, liquidation_pressure: -0.5, basis: 0.0, spread_bps: Some(2.0), spread_age_ms: Some(0) };
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
    fn attention_rows_sum_to_one_and_matrix_is_seven_by_seven() {
        let result = decide(sample(), &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(result.attention.len(), 7);
        for row in result.attention {
            assert_eq!(row.len(), 7);
            assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9);
            assert!(row.iter().all(|weight| *weight >= 0.0 && weight.is_finite()));
        }
    }

    #[test]
    fn output_exposes_reproducible_feature_and_selection_ledger() {
        let result = decide(sample(), &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_eq!(result.feature_trace.len(), 7);
        assert_eq!(result.features.len(), 7);
        assert_eq!(result.feature_trace[0].raw_value, 2.0);
        assert_eq!(result.feature_trace[0].directional_weight, 0.30);
        assert_eq!(result.config_version, "baseline-v2");
        assert_eq!(result.decision_vector[0], result.score.max(0.0));
    }
}
