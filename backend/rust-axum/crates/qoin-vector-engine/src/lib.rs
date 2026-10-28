//! Deterministic, research-stage market-force vector decision engine.
//!
//! This crate emits signals only. It does not size positions, place orders, or
//! claim profitability. Inputs must be causal and normalized using past-only
//! statistics by the caller. All coefficients are explicit baseline parameters,
//! not trained weights and not evidence of an edge.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Index of order-flow imbalance in ForceObservation::values.
pub const OFI_IDX: usize = 0;
/// Index of open-interest change in ForceObservation::values.
pub const OI_IDX: usize = 4;
const INPUT_COUNT: usize = 7;

/// Caller-supplied, caller-normalized market-force observations.
///
/// Field order is part of the interaction contract: OFI is index 0 and OI
/// change is index 4. Inputs are neither imputed nor smoothed by this crate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ForceObservation {
    /// Order-flow imbalance.
    pub ofi: f64,
    /// Caller-defined liquidity force.
    pub liquidity: f64,
    /// Volatility context; it dampens score but has no directional coefficient.
    pub volatility: f64,
    /// Funding force.
    pub funding: f64,
    /// Change in open interest.
    pub open_interest_change: f64,
    /// Liquidation-pressure force.
    pub liquidation_pressure: f64,
    /// Basis force.
    pub basis: f64,
}

impl ForceObservation {
    /// Returns values in the stable coefficient/index order documented above.
    pub fn values(self) -> [f64; INPUT_COUNT] {
        [
            self.ofi, self.liquidity, self.volatility, self.funding,
            self.open_interest_change, self.liquidation_pressure, self.basis,
        ]
    }
}

/// Directional or neutral outcome emitted by the signal engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    /// Positive directional signal passed all gates.
    Buy,
    /// Negative directional signal passed all gates.
    Sell,
    /// No directional signal while a position is reported open.
    Close,
    /// No directional signal while no position is reported open.
    Wait,
}

/// Machine-readable explanation for a rejected directional entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectReason {
    /// Directional score did not reach the configured threshold.
    LowMagnitude,
    /// Score had sufficient magnitude, but directional validation failed.
    Misaligned,
    /// Both magnitude and directional alignment failed.
    Both,
    /// Effective directional vector has no measurable magnitude.
    FlatVector,
    /// No position was open, so a neutral signal resolves to WAIT.
    NoPosition,
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::LowMagnitude => "low magnitude",
            Self::Misaligned => "misaligned direction",
            Self::Both => "low magnitude and misaligned direction",
            Self::FlatVector => "flat directional vector",
            Self::NoPosition => "no open position",
        };
        f.write_str(label)
    }
}

/// Configuration for the deterministic signal engine.
///
/// Coefficients are research placeholders, not fitted parameters. Calibrate
/// offline using strictly chronological validation before interpreting them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineConfig {
    /// Minimum absolute directional score; placeholder, calibrate offline.
    pub signal_threshold: f64,
    /// Maximum absolute caller-normalized input; values beyond it are clipped.
    pub input_clip: f64,
    /// Maximum absolute output score; values beyond it are saturated.
    pub score_clip: f64,
    /// Number of largest absolute input magnitudes retained (magnitude gate, not Pareto frontier).
    pub top_k: usize,
    /// Minimum cosine alignment between effective force and directional template.
    pub min_alignment: f64,
    /// Maximum angular deviation in radians for a directional entry.
    pub max_angle_radians: f64,
    /// Nonnegative volatility score dampener; placeholder, calibrate offline.
    pub volatility_dampener: f64,
    /// OFI × OI interaction coefficient; interaction is clamped to [-0.25, 0.25].
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

impl EngineConfig {
    /// Computes a stable FNV-1a fingerprint over every configuration field.
    ///
    /// This is an identity/checksum, not a cryptographic hash. Floating-point
    /// values are hashed by their IEEE-754 bit patterns, so -0.0 and 0.0
    /// intentionally have different fingerprints.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        let mut feed = |byte: u8| {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        };
        for value in [
            self.signal_threshold, self.input_clip, self.score_clip,
            self.min_alignment, self.max_angle_radians,
            self.volatility_dampener, self.interaction_coefficient,
        ] {
            for byte in value.to_bits().to_le_bytes() {
                feed(byte);
            }
        }
        for byte in (self.top_k as u64).to_le_bytes() {
            feed(byte);
        }
        hash
    }
}

/// Result of a deterministic signal evaluation.
///
/// reject_reason is None for accepted BUY/SELL signals and populated for
/// WAIT/CLOSE outcomes. rationale is derived from decision and structured
/// reason; consumers should aggregate by reject_reason, never parse text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    /// Final signal outcome.
    pub decision: Decision,
    /// Clipped directional score.
    pub score: f64,
    /// Cosine alignment of the effective input vector and directional template.
    pub alignment: f64,
    /// Angular distance in radians from the directional template.
    pub angle_radians: f64,
    /// Count of nonzero selected input forces before applying coefficients.
    pub active_force_count: usize,
    /// Stable fingerprint of the exact configuration used.
    pub config_fingerprint: u64,
    /// Structured reason for a rejected directional entry, if any.
    pub reject_reason: Option<RejectReason>,
    /// Human-readable explanation derived from decision and structured reason.
    pub rationale: String,
}

/// Errors returned when inputs or configuration violate the engine contract.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EngineError {
    /// At least one observation is NaN or infinite.
    #[error("all observations must be finite")]
    NonFiniteInput,
    /// A configuration value is outside the supported domain.
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
        && (1..=INPUT_COUNT).contains(&config.top_k)
        && (-1.0..=1.0).contains(&config.min_alignment)
        && (0.0..=std::f64::consts::PI).contains(&config.max_angle_radians)
        && config.volatility_dampener >= 0.0
        && config.interaction_coefficient.abs() <= 0.25
}

/// Computes a deterministic signal from one observation and an explicit config.
///
/// Breaking API change: the caller-supplied config_version string was removed.
/// The returned config_fingerprint is computed from the actual config, so a
/// label cannot accidentally claim a different configuration. Inputs remain
/// caller-supplied and caller-normalized; this function uses no history.
pub fn decide(
    input: ForceObservation,
    config: &EngineConfig,
    has_open_position: bool,
) -> Result<Signal, EngineError> {
    let raw = input.values();
    if raw.iter().any(|value| !value.is_finite()) {
        return Err(EngineError::NonFiniteInput);
    }
    if !valid_config(config) {
        return Err(EngineError::InvalidConfig);
    }

    // Coefficients are baseline placeholders; calibrate offline. OFI +0.30
    // and liquidity +0.18 reward aligned positive pressure; volatility 0.0 is
    // context-only; funding -0.10 and basis -0.08 encode inverse effects; OI
    // +0.12 and liquidation +0.12 are directional placeholders.
    let weights = [0.30_f64, 0.18, 0.0, -0.10, 0.12, 0.12, -0.08];
    let clipped = raw.map(|value| value.clamp(-config.input_clip, config.input_clip));

    // Retain the strongest magnitudes; this is a top-k gate, not Pareto dominance.
    let mut rank: Vec<(usize, f64)> = clipped.iter().enumerate()
        .map(|(index, value)| (index, value.abs())).collect();
    rank.sort_by(|left, right| right.1.total_cmp(&left.1));
    let mut selected = [false; INPUT_COUNT];
    for (index, magnitude) in rank.into_iter().take(config.top_k) {
        if magnitude > 0.0 {
            selected[index] = true;
        }
    }
    let mut values = clipped;
    for (value, is_selected) in values.iter_mut().zip(selected.iter()) {
        if !*is_selected {
            *value = 0.0;
        }
    }
    let active_force_count = selected.iter().filter(|selected| **selected).count();

    // Express interaction as an OFI-equivalent input so both gates see it.
    let interaction = (config.interaction_coefficient * values[OFI_IDX] * values[OI_IDX])
        .clamp(-0.25, 0.25);
    let mut effective_direction = values;
    effective_direction[OFI_IDX] += interaction / weights[OFI_IDX];

    let mut dot = 0.0;
    let mut observed_norm_sq = 0.0;
    let mut template_norm_sq = 0.0;
    for (direction, weight) in effective_direction.iter().zip(weights.iter()) {
        dot += *direction * *weight;
        observed_norm_sq += *direction * *direction;
        template_norm_sq += *weight * *weight;
    }
    let observed_norm = observed_norm_sq.sqrt();
    let template_norm = template_norm_sq.sqrt();
    let alignment = if observed_norm <= f64::EPSILON || template_norm <= f64::EPSILON {
        0.0
    } else {
        (dot / (observed_norm * template_norm)).clamp(-1.0, 1.0)
    };
    let angle_radians = alignment.acos();
    let dampener = 1.0 / (1.0 + values[2].abs() * config.volatility_dampener);
    let score = (dot * dampener).clamp(-config.score_clip, config.score_clip);

    // At max_angle = acos(min_alignment), angular and cosine gates agree; other
    // valid configs may make either gate stricter, so both remain explicit.
    let buy_aligned = alignment >= config.min_alignment
        && angle_radians <= config.max_angle_radians;
    let sell_aligned = alignment <= -config.min_alignment
        && (std::f64::consts::PI - angle_radians) <= config.max_angle_radians;
    let directional_score_passes = score.abs() >= config.signal_threshold;
    let flat_vector = observed_norm <= f64::EPSILON;
    let decision = if score >= config.signal_threshold && buy_aligned {
        Decision::Buy
    } else if score <= -config.signal_threshold && sell_aligned {
        // A valid SELL remains SELL even when a position is open.
        Decision::Sell
    } else if has_open_position {
        Decision::Close
    } else {
        Decision::Wait
    };

    let reject_reason = match decision {
        Decision::Buy | Decision::Sell => None,
        Decision::Close | Decision::Wait if flat_vector => Some(RejectReason::FlatVector),
        Decision::Close | Decision::Wait => {
            let aligned = if score >= 0.0 { buy_aligned } else { sell_aligned };
            match (directional_score_passes, aligned) {
                (false, false) => Some(RejectReason::Both),
                (false, true) => Some(RejectReason::LowMagnitude),
                (true, false) => Some(RejectReason::Misaligned),
                (true, true) => Some(RejectReason::LowMagnitude),
            }
        }
    };
    let rationale = match reject_reason {
        Some(reason) => format!("{:?}: {reason}", decision),
        None => format!("{:?}: directional gates passed", decision),
    };

    Ok(Signal {
        decision, score, alignment, angle_radians, active_force_count,
        config_fingerprint: config.fingerprint(), reject_reason, rationale,
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

    fn zero() -> ForceObservation {
        ForceObservation {
            ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0,
        }
    }

    #[test]
    fn strong_positive_force_produces_buy() {
        assert_eq!(decide(sample(), &EngineConfig::default(), false).unwrap().decision, Decision::Buy);
    }

    #[test]
    fn weak_signal_without_position_waits_with_structured_reason() {
        let result = decide(zero(), &EngineConfig::default(), false).unwrap();
        assert_eq!(result.decision, Decision::Wait);
        assert_eq!(result.active_force_count, 0);
        assert_eq!(result.reject_reason, Some(RejectReason::FlatVector));
    }

    #[test]
    fn non_finite_values_are_rejected() {
        let mut input = sample();
        input.ofi = f64::NAN;
        assert_eq!(decide(input, &EngineConfig::default(), false).unwrap_err(), EngineError::NonFiniteInput);
    }

    #[test]
    fn strong_negative_force_produces_sell() {
        let input = ForceObservation {
            ofi: -2.0, liquidity: -1.0, volatility: 0.2, funding: 0.5,
            open_interest_change: -1.0, liquidation_pressure: -0.5, basis: 0.0,
        };
        assert_eq!(decide(input, &EngineConfig::default(), false).unwrap().decision, Decision::Sell);
    }

    #[test]
    fn invalid_top_k_is_rejected() {
        let mut config = EngineConfig::default();
        config.top_k = 0;
        assert_eq!(decide(sample(), &config, false).unwrap_err(), EngineError::InvalidConfig);
    }

    #[test]
    fn opposing_force_vector_does_not_trigger_buy() {
        let input = ForceObservation {
            ofi: -2.5, liquidity: -2.0, volatility: 0.1, funding: 1.5,
            open_interest_change: -2.0, liquidation_pressure: -1.5, basis: 1.0,
        };
        assert_ne!(decide(input, &EngineConfig::default(), false).unwrap().decision, Decision::Buy);
    }

    #[test]
    fn config_fingerprint_is_stable_and_content_bound() {
        let first = EngineConfig::default();
        let identical = EngineConfig::default();
        let mut different = EngineConfig::default();
        different.top_k += 1;
        assert_eq!(first.fingerprint(), identical.fingerprint());
        assert_ne!(first.fingerprint(), different.fingerprint());
        assert_eq!(decide(sample(), &first, false).unwrap().config_fingerprint, first.fingerprint());
    }

    #[test]
    fn zero_vector_with_position_closes_for_flat_vector() {
        let result = decide(zero(), &EngineConfig::default(), true).unwrap();
        assert_eq!(result.decision, Decision::Close);
        assert_eq!(result.reject_reason, Some(RejectReason::FlatVector));
    }

    #[test]
    fn valid_sell_with_open_position_remains_sell() {
        let input = ForceObservation {
            ofi: -2.0, liquidity: -1.0, volatility: 0.2, funding: 0.5,
            open_interest_change: -1.0, liquidation_pressure: -0.5, basis: 0.0,
        };
        assert_eq!(decide(input, &EngineConfig::default(), true).unwrap().decision, Decision::Sell);
    }

    #[test]
    fn seven_inputs_and_unrestricted_angles_are_well_defined() {
        let mut config = EngineConfig::default();
        config.top_k = 7;
        config.min_alignment = 0.0;
        config.max_angle_radians = std::f64::consts::PI;
        assert_eq!(decide(sample(), &config, false).unwrap().decision, Decision::Buy);
        assert_eq!(decide(zero(), &config, false).unwrap().decision, Decision::Wait);
    }

    #[test]
    fn score_clip_saturates_beyond_input_clip() {
        let mut config = EngineConfig::default();
        config.input_clip = 3.0;
        config.score_clip = 0.2;
        let mut input = sample();
        input.ofi = 1.0e12;
        input.open_interest_change = 1.0e12;
        assert_eq!(decide(input, &config, false).unwrap().score, config.score_clip);
        input.ofi = -1.0e12;
        input.open_interest_change = -1.0e12;
        assert_eq!(decide(input, &config, false).unwrap().score, -config.score_clip);
    }

    #[test]
    fn inputs_at_and_beyond_clip_produce_identical_signals() {
        let mut config = EngineConfig::default();
        config.input_clip = 3.0;
        let mut at = sample();
        at.ofi = config.input_clip;
        let mut beyond = at;
        beyond.ofi = config.input_clip + f64::EPSILON * 4.0;
        assert_eq!(decide(at, &config, false).unwrap(), decide(beyond, &config, false).unwrap());
        at.ofi = -config.input_clip;
        beyond.ofi = -config.input_clip - f64::EPSILON * 4.0;
        assert_eq!(decide(at, &config, false).unwrap(), decide(beyond, &config, false).unwrap());
    }

    #[test]
    fn sell_angle_boundary_matches_alignment_boundary() {
        let mut config = EngineConfig::default();
        config.min_alignment = 0.15;
        config.max_angle_radians = config.min_alignment.acos();
        let input = ForceObservation {
            ofi: -2.0, liquidity: -1.0, volatility: 0.0, funding: 0.0,
            open_interest_change: -1.0, liquidation_pressure: 0.0, basis: 0.0,
        };
        let result = decide(input, &config, false).unwrap();
        let angle_boundary = std::f64::consts::PI - config.max_angle_radians;
        let alignment_boundary = (-config.min_alignment).acos();
        assert!((angle_boundary - alignment_boundary).abs() < 1.0e-12);
        let angle_gate = (std::f64::consts::PI - result.angle_radians) <= config.max_angle_radians;
        let alignment_gate = result.alignment <= -config.min_alignment;
        assert_eq!(angle_gate, alignment_gate);
    }

    #[test]
    fn doubled_interaction_can_cross_magnitude_gate_but_alignment_uses_same_vector() {
        let input = ForceObservation {
            ofi: 1.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
            open_interest_change: 1.0, liquidation_pressure: 0.0, basis: 0.0,
        };
        let mut baseline = EngineConfig::default();
        baseline.input_clip = 1.0;
        baseline.signal_threshold = 0.46;
        baseline.top_k = 7;
        let baseline_signal = decide(input, &baseline, false).unwrap();

        let mut doubled = baseline.clone();
        doubled.interaction_coefficient *= 2.0;
        let doubled_signal = decide(input, &doubled, false).unwrap();

        assert_eq!(baseline_signal.decision, Decision::Wait);
        assert_eq!(baseline_signal.reject_reason, Some(RejectReason::LowMagnitude));
        assert_eq!(doubled_signal.decision, Decision::Buy);
        assert!(doubled_signal.score > baseline_signal.score);
        assert!(doubled_signal.alignment > baseline_signal.alignment);
    }

    #[test]
    fn observation_indices_are_stable() {
        let values = sample().values();
        assert_eq!(OFI_IDX, 0);
        assert_eq!(OI_IDX, 4);
        assert_eq!(values[OFI_IDX], sample().ofi);
        assert_eq!(values[OI_IDX], sample().open_interest_change);
    }
}
