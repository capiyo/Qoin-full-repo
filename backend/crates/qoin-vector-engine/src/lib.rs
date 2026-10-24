//! Deterministic, research-stage market-force vector decision engine.
//!
//! This crate emits signals only. It does not size positions, place orders, or
//! claim profitability. Inputs must be causal and normalized using past-only
//! statistics by the caller. All coefficients are explicit baseline parameters,
//! not trained weights and not evidence of an edge.

pub mod strategy;

use serde::{Deserialize, Serialize};

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
    /// Maximum allowed absolute input after normalization/clipping.
    pub input_clip: f64,
    /// Maximum absolute output score.
    pub score_clip: f64,
    /// Number of strongest absolute forces retained by Pareto selection.
    pub top_k: usize,
    /// Maximum angular deviation in radians for a directional entry.
    pub max_angle_radians: f64,
    /// Exit hysteresis threshold, strictly below the entry signal threshold.
    pub exit_threshold: f64,
    /// Volatility dampening coefficient.
    pub volatility_dampener: f64,
    /// Small regularized OFI × OI interaction coefficient.
    pub interaction_coefficient: f64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            signal_threshold: 0.65,
            input_clip: 3.0,
            score_clip: 1.0,
            top_k: 5,
            max_angle_radians: 0.70,
            exit_threshold: 0.35,
            volatility_dampener: 0.15,
            interaction_coefficient: 0.025,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CostContext {
    pub expected_edge_bps: f64,
    pub fees_bps: f64,
    pub spread_bps: f64,
    pub slippage_bps: f64,
    pub multiplier: f64,
}
pub fn cost_gate(costs: Option<&CostContext>) -> bool {
    let Some(c) = costs else { return false; };
    let values = [c.expected_edge_bps, c.fees_bps, c.spread_bps, c.slippage_bps, c.multiplier];
    if values.iter().any(|v| !v.is_finite()) || c.expected_edge_bps <= 0.0
        || c.fees_bps < 0.0 || c.spread_bps < 0.0 || c.slippage_bps < 0.0 || c.multiplier <= 0.0 {
        return false;
    }
    c.expected_edge_bps > c.multiplier * (c.fees_bps + c.spread_bps + c.slippage_bps)
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
        && config.input_clip.is_finite()
        && config.score_clip.is_finite()
        && config.max_angle_radians.is_finite()
        && config.exit_threshold.is_finite()
        && config.volatility_dampener.is_finite()
        && config.interaction_coefficient.is_finite()
        && config.signal_threshold > 0.0
        && config.input_clip > 0.0
        && config.score_clip > 0.0
        && config.score_clip <= 1.0
        && (1..=7).contains(&config.top_k)
        && (0.0..=std::f64::consts::PI).contains(&config.max_angle_radians)
        && config.exit_threshold >= 0.0
        && config.exit_threshold < config.signal_threshold
        && config.volatility_dampener >= 0.0
        && config.interaction_coefficient.abs() <= 0.25
}

pub fn decide(
    input: ForceObservation,
    config: &EngineConfig,
    config_version: &str,
    has_open_position: bool,
    costs: Option<&CostContext>,
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
    let buy_aligned = angle_radians <= config.max_angle_radians;
    let sell_aligned = (std::f64::consts::PI - angle_radians) <= config.max_angle_radians;
    let cost_ok = cost_gate(costs);
    let decision = if score >= config.signal_threshold && buy_aligned && cost_ok {
        Decision::Buy
    } else if score <= -config.signal_threshold && sell_aligned && cost_ok {
        Decision::Sell
    } else if has_open_position && score.abs() <= config.exit_threshold {
        Decision::Close
    } else {
        Decision::Wait
    };
    let directional_candidate = (score >= config.signal_threshold && buy_aligned)
        || (score <= -config.signal_threshold && sell_aligned);
    let rationale = if directional_candidate && !cost_ok {
        "WAIT: calibrated expected edge must exceed fees, spread, and slippage stress".to_owned()
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

    fn test_cost_context() -> CostContext {
        CostContext { expected_edge_bps: 100.0, fees_bps: 1.0, spread_bps: 2.0, slippage_bps: 1.0, multiplier: 2.0 }
    }

    fn sample() -> ForceObservation {
        ForceObservation {
            ofi: 2.0, liquidity: 1.0, volatility: 0.2, funding: -0.5,
            open_interest_change: 1.0, liquidation_pressure: 0.5, basis: 0.0,
        }
    }

    #[test]
    fn strong_positive_force_produces_buy() {
        assert_eq!(decide(sample(), &EngineConfig::default(), "baseline-v2", false, Some(&test_cost_context())).unwrap().decision, Decision::Buy);
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
    fn opposing_force_vector_does_not_trigger_entry() {
        let input = ForceObservation { ofi: -2.5, liquidity: -2.0, volatility: 0.1, funding: 1.5, open_interest_change: -2.0, liquidation_pressure: -1.5, basis: 1.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_ne!(result.decision, Decision::Buy);
    }
    #[test]
    fn open_position_does_not_close_on_every_subthreshold_tick() {
        let input = ForceObservation { ofi: 1.5, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v3", true, None).unwrap();
        assert_eq!(result.decision, Decision::Wait);
        assert!(result.score.abs() > EngineConfig::default().exit_threshold);
        assert!(result.score.abs() < EngineConfig::default().signal_threshold);
    }

    #[test]
    fn open_position_closes_only_below_exit_threshold() {
        let input = ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v3", true).unwrap();
        assert_eq!(result.decision, Decision::Close);
    }

    #[test]
    fn exit_threshold_must_be_below_entry_threshold() {
        let mut config = EngineConfig::default();
        config.exit_threshold = config.signal_threshold;
        assert_eq!(decide(sample(), &config, "baseline-v3", false, None).unwrap_err(), EngineError::InvalidConfig);
    }

    #[test]
    fn directional_entry_is_vetoed_without_cost_context() {
        let result = decide(sample(), &EngineConfig::default(), "baseline-v3", false, None).unwrap();
        assert_eq!(result.decision, Decision::Wait);
        assert!(result.rationale.contains("expected edge"));
    }

}
