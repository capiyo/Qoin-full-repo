//! Deterministic, research-stage market-force vector decision engine.
//!
//! This crate emits signals only. It does not size positions, place orders, or
//! claim profitability. Inputs must be causal and normalized using past-only
//! statistics by the caller. All coefficients are explicit baseline parameters,
//! not trained weights and not evidence of an edge.

use serde::{Deserialize, Serialize};
use qoin_market_space::{MarketFeature, MarketSpaceError, MarketVector};

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
            input_clip: 3.0,
            score_clip: 1.0,
            top_k: 5,
            min_alignment: 0.15,
            max_angle_radians: 1.42,
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
    } else if has_open_position {
        Decision::Close
    } else {
        Decision::Wait
    };
    let rationale = if !buy_aligned && !sell_aligned {
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


/// Result of evaluating Version 1 against the shared market-state vector.
/// Regime coordinates raise the evidence threshold and volatility dampening;
/// they never create a directional vote of their own.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketVectorDecision {
    pub signal: Signal,
    pub regime_risk_score: f64,
    pub effective_signal_threshold: f64,
    pub effective_volatility_dampener: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MarketVectorDecisionError {
    #[error(transparent)]
    MarketSpace(#[from] MarketSpaceError),
    #[error(transparent)]
    Engine(#[from] EngineError),
}

/// Version 1 adapter for the shared 53-dimensional market vector.
/// Required coordinates remain explicit: unavailable observations fail closed.
/// Positive regime coordinates represent elevated risk; negative values represent
/// subdued conditions. No hard regime label is introduced.
pub fn decide_from_market_vector(
    vector: &MarketVector,
    config: &EngineConfig,
    config_version: &str,
    has_open_position: bool,
) -> Result<MarketVectorDecision, MarketVectorDecisionError> {
    vector.validate()?;
    let input = ForceObservation {
        ofi: vector.require(MarketFeature::OrderFlowImbalance)?,
        liquidity: vector.require(MarketFeature::OrderBookDepth)?,
        volatility: vector.require(MarketFeature::RealizedVolatility)?,
        funding: vector.require(MarketFeature::FundingPressure)?,
        open_interest_change: vector.require(MarketFeature::OpenInterestChange)?,
        liquidation_pressure: vector.require(MarketFeature::LiquidationPressure)?,
        basis: vector.require(MarketFeature::Basis)?,
    };

    let regime_volatility = vector.require(MarketFeature::RegimeVolatilityLevel)?.max(0.0);
    let regime_risk_components = [
        regime_volatility,
        vector.require(MarketFeature::RegimeLiquidityStress)?.max(0.0),
        vector.require(MarketFeature::RegimeCorrelationStress)?.max(0.0),
        vector.require(MarketFeature::RegimeTransitionIntensity)?.max(0.0),
        vector.require(MarketFeature::RegimeUncertainty)?.max(0.0),
    ];
    let regime_risk_score = regime_risk_components.iter().sum::<f64>()
        / regime_risk_components.len() as f64;

    let mut effective_config = config.clone();
    effective_config.signal_threshold = config.signal_threshold * (1.0 + regime_risk_score);
    effective_config.volatility_dampener =
        config.volatility_dampener * (1.0 + regime_volatility);
    let signal = decide(input, &effective_config, config_version, has_open_position)?;

    Ok(MarketVectorDecision {
        signal,
        regime_risk_score,
        effective_signal_threshold: effective_config.signal_threshold,
        effective_volatility_dampener: effective_config.volatility_dampener,
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


    fn shared_market_vector(regime_risk: f64) -> MarketVector {
        let mut vector = MarketVector::new(
            "v1-test-vector", "test-venue", "BTCUSD", "5m",
            1_800_000_000_000, 1_800_000_000_001,
        ).unwrap();
        for (feature, value) in [
            (MarketFeature::OrderFlowImbalance, 1.0),
            (MarketFeature::OrderBookDepth, 1.0),
            (MarketFeature::RealizedVolatility, 0.1),
            (MarketFeature::FundingPressure, -0.5),
            (MarketFeature::OpenInterestChange, 1.0),
            (MarketFeature::LiquidationPressure, 0.5),
            (MarketFeature::Basis, 0.0),
            (MarketFeature::RegimeVolatilityLevel, regime_risk),
            (MarketFeature::RegimeLiquidityStress, regime_risk),
            (MarketFeature::RegimeCorrelationStress, regime_risk),
            (MarketFeature::RegimeTransitionIntensity, regime_risk),
            (MarketFeature::RegimeUncertainty, regime_risk),
        ] {
            vector.set_value(feature, Some(value), 1.0).unwrap();
        }
        vector
    }

    #[test]
    fn shared_market_vector_drives_v1_and_includes_regime_overlay() {
        let calm = decide_from_market_vector(
            &shared_market_vector(0.0), &EngineConfig::default(), "v1-shared-space", false,
        ).unwrap();
        let stressed = decide_from_market_vector(
            &shared_market_vector(1.0), &EngineConfig::default(), "v1-shared-space", false,
        ).unwrap();
        assert_eq!(calm.signal.decision, Decision::Buy);
        assert!(stressed.effective_signal_threshold > calm.effective_signal_threshold);
        assert_eq!(stressed.signal.decision, Decision::Wait);
    }

    #[test]
    fn shared_market_vector_fails_closed_when_required_regime_is_missing() {
        let mut vector = shared_market_vector(0.0);
        vector.set_value(MarketFeature::RegimeUncertainty, None, 0.0).unwrap();
        assert!(matches!(
            decide_from_market_vector(&vector, &EngineConfig::default(), "v1", false),
            Err(MarketVectorDecisionError::MarketSpace(MarketSpaceError::MissingFeature(
                MarketFeature::RegimeUncertainty
            )))
        ));
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
    fn opposing_force_vector_does_not_trigger_entry() {
        let input = ForceObservation { ofi: -2.5, liquidity: -2.0, volatility: 0.1, funding: 1.5, open_interest_change: -2.0, liquidation_pressure: -1.5, basis: 1.0 };
        let result = decide(input, &EngineConfig::default(), "baseline-v2", false).unwrap();
        assert_ne!(result.decision, Decision::Buy);
    }
}
