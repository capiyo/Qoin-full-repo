//! Offline evaluation and bounded hyperparameter search for the Version 1 vector engine.
//!
//! This module never mutates the active strategy map and never submits orders.
//! Callers must supply chronological, out-of-sample labels and costs in decimal
//! return units (e.g. 0.0005 = 5 bps). Optimization is research-only.
use crate::{decide, Decision, EngineConfig, ForceObservation};

#[derive(Debug, Clone, Copy)]
pub struct LabeledObservation {
    pub forces: ForceObservation,
    /// Realized forward return over the evaluation horizon, decimal units.
    pub realized_return: f64,
    /// Estimated round-trip cost in decimal return units, always non-negative.
    pub round_trip_cost: f64,
    pub has_open_position: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct LossConfig {
    /// Penalizes downside variation of per-observation net returns.
    pub downside_weight: f64,
    /// Penalizes changes in exposure between adjacent observations.
    pub turnover_weight: f64,
}

impl Default for LossConfig {
    fn default() -> Self {
        Self { downside_weight: 1.0, turnover_weight: 0.05 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evaluation {
    pub loss: f64,
    pub mean_net_return: f64,
    pub downside_rms: f64,
    pub turnover: f64,
    pub observations: usize,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ResearchError {
    #[error("evaluation set must not be empty")]
    Empty,
    #[error("returns, costs, and loss weights must be finite; costs/weights must be non-negative")]
    InvalidValue,
    #[error("vector engine rejected configuration or input")]
    Engine,
    #[error("no valid threshold candidates supplied")]
    NoCandidates,
}

fn exposure(decision: Decision) -> f64 {
    match decision {
        Decision::Buy => 1.0,
        Decision::Sell => -1.0,
        Decision::Close | Decision::Wait => 0.0,
    }
}

/// Evaluate negative net utility plus downside-risk and turnover penalties.
/// Cost is charged when exposure changes into or out of a position.
pub fn evaluate(
    samples: &[LabeledObservation],
    engine_config: &EngineConfig,
    loss_config: LossConfig,
) -> Result<Evaluation, ResearchError> {
    if samples.is_empty() { return Err(ResearchError::Empty); }
    if !loss_config.downside_weight.is_finite()
        || !loss_config.turnover_weight.is_finite()
        || loss_config.downside_weight < 0.0
        || loss_config.turnover_weight < 0.0
    { return Err(ResearchError::InvalidValue); }

    let mut returns = Vec::with_capacity(samples.len());
    let mut previous_exposure = 0.0;
    let mut turnover_sum = 0.0;
    for sample in samples {
        if !sample.realized_return.is_finite()
            || !sample.round_trip_cost.is_finite()
            || sample.round_trip_cost < 0.0
        { return Err(ResearchError::InvalidValue); }
        let signal = decide(sample.forces, engine_config, "research-evaluation", sample.has_open_position)
            .map_err(|_| ResearchError::Engine)?;
        let current = exposure(signal.decision);
        let turnover = (current - previous_exposure).abs();
        let cost = if turnover > 0.0 { sample.round_trip_cost * turnover } else { 0.0 };
        returns.push(current * sample.realized_return - cost);
        turnover_sum += turnover;
        previous_exposure = current;
    }

    let n = returns.len() as f64;
    let mean_net_return = returns.iter().sum::<f64>() / n;
    let downside_rms = (returns.iter().map(|r| r.min(0.0).powi(2)).sum::<f64>() / n).sqrt();
    let turnover = turnover_sum / n;
    let loss = -mean_net_return
        + loss_config.downside_weight * downside_rms
        + loss_config.turnover_weight * turnover;
    Ok(Evaluation { loss, mean_net_return, downside_rms, turnover, observations: samples.len() })
}

/// Small deterministic threshold sweep. Use only on a training split; select and
/// report the returned candidate on a separate chronological validation split.
/// This does not write or activate the returned config.
pub fn optimize_signal_threshold(
    training_samples: &[LabeledObservation],
    base_config: &EngineConfig,
    loss_config: LossConfig,
    candidates: &[f64],
) -> Result<(EngineConfig, Evaluation), ResearchError> {
    if candidates.is_empty() { return Err(ResearchError::NoCandidates); }
    let mut best: Option<(EngineConfig, Evaluation)> = None;
    for threshold in candidates {
        if !threshold.is_finite() || *threshold <= 0.0 { continue; }
        let mut candidate = base_config.clone();
        candidate.signal_threshold = *threshold;
        if let Ok(score) = evaluate(training_samples, &candidate, loss_config) {
            if best.as_ref().map_or(true, |(_, current)| score.loss < current.loss) {
                best = Some((candidate, score));
            }
        }
    }
    best.ok_or(ResearchError::NoCandidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(realized_return: f64) -> LabeledObservation {
        LabeledObservation {
            forces: ForceObservation {
                ofi: 2.0, liquidity: 1.0, volatility: 0.2, funding: -0.5,
                open_interest_change: 1.0, liquidation_pressure: 0.5, basis: 0.0,
            },
            realized_return,
            round_trip_cost: 0.0005,
            has_open_position: false,
        }
    }

    #[test]
    fn loss_accounts_for_costs_and_downside() {
        let samples = [sample(0.01), sample(-0.02)];
        let result = evaluate(&samples, &EngineConfig::default(), LossConfig::default()).unwrap();
        assert!(result.loss.is_finite());
        assert_eq!(result.observations, 2);
        assert!(result.downside_rms > 0.0);
    }

    #[test]
    fn optimizer_returns_candidate_without_mutating_base_config() {
        let samples = [sample(0.01), sample(0.005)];
        let base = EngineConfig::default();
        let (selected, score) = optimize_signal_threshold(
            &samples, &base, LossConfig::default(), &[0.5, 0.65, 0.8],
        ).unwrap();
        assert!(selected.signal_threshold > 0.0);
        assert_eq!(base.signal_threshold, 0.65);
        assert!(score.loss.is_finite());
    }

    #[test]
    fn rejects_empty_evaluation() {
        assert_eq!(
            evaluate(&[], &EngineConfig::default(), LossConfig::default()).unwrap_err(),
            ResearchError::Empty,
        );
    }
}
