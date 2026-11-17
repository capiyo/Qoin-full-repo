//! Shared contribution attribution for Qoin V1 and V2.
//!
//! Percentages describe a decomposition of a model score or measured evidence
//! mass. They are not probabilities that a feature caused a market move.
//! Vector projections explain additive numerical scores; exact Shapley values
//! allocate interactions in nonlinear outcomes; atomic measures allocate
//! overlapping set evidence without double-counting the union.

use crate::{FeatureAvailability, FeatureFamily, MarketFeature, MarketVector};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionMethod {
    VectorProjection,
    ExactShapley,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContributionDirection {
    Positive,
    Negative,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contribution {
    /// Stable variable, evidence group, or model component key.
    pub key: String,
    pub feature: Option<MarketFeature>,
    pub signed_effect: f64,
    /// Share of total absolute effect. Shares sum to 100 when total effect > 0.
    pub contribution_pct: f64,
    pub quality: Option<f64>,
    pub direction: ContributionDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributionReport {
    pub method: AttributionMethod,
    pub baseline: f64,
    pub outcome: f64,
    pub net_effect: f64,
    pub absolute_effect_total: f64,
    /// Percentage of configured applicable input weight that was observed.
    pub coverage_pct: f64,
    pub contributions: Vec<Contribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetMeasureInput {
    pub evidence_set_id: String,
    pub atoms: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AtomMeasureAllocation {
    pub atom: String,
    pub evidence_set_id: String,
    pub allocated_mass: f64,
    pub membership_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetMeasureContribution {
    pub evidence_set_id: String,
    pub allocated_mass: f64,
    /// Share of the measure of the union, after overlap allocation.
    pub contribution_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetMeasureReport {
    pub union_mass: f64,
    pub contributions: Vec<SetMeasureContribution>,
    pub atom_allocations: Vec<AtomMeasureAllocation>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AttributionError {
    #[error("attribution inputs must be finite and within their valid ranges")]
    InvalidInput,
    #[error("attribution component keys must be unique and non-empty")]
    InvalidKey,
    #[error("market vector is invalid")]
    InvalidMarketVector,
    #[error("no applicable non-zero model weights were supplied")]
    EmptyWeights,
    #[error("coalition values must contain exactly 2^n entries for 1 <= n <= 12")]
    InvalidCoalitionTable,
    #[error("coalition values do not contain a finite baseline and outcome")]
    InvalidCoalitionValue,
    #[error("every atom in every evidence set must have a non-negative finite measure")]
    MissingOrInvalidAtomMeasure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedEffectInput {
    pub key: String,
    pub feature: Option<MarketFeature>,
    pub signed_effect: f64,
    pub quality: Option<f64>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FamilyContribution {
    pub family: FeatureFamily,
    pub signed_effect: f64,
    pub absolute_effect: f64,
    pub contribution_pct: f64,
    pub quality: Option<f64>,
    pub direction: ContributionDirection,
}

/// Aggregate coordinate-level effects by the schema's canonical feature
/// families. Percentages sum absolute coordinate effects, not absolute net
/// family effects, so opposing variables cannot disappear through cancellation.
pub fn aggregate_by_family(report: &AttributionReport) -> Vec<FamilyContribution> {
    let mut groups: BTreeMap<FeatureFamily, (f64, f64, f64, f64)> = BTreeMap::new();
    for contribution in &report.contributions {
        let Some(feature) = contribution.feature else { continue; };
        let entry = groups.entry(feature.family()).or_insert((0.0, 0.0, 0.0, 0.0));
        entry.0 += contribution.signed_effect;
        entry.1 += contribution.signed_effect.abs();
        if let Some(quality) = contribution.quality {
            let weight = contribution.signed_effect.abs();
            entry.2 += quality * weight;
            entry.3 += weight;
        }
    }
    let absolute_total = groups.values().map(|entry| entry.1).sum::<f64>();
    groups.into_iter().map(|(family, (signed_effect, absolute_effect, quality_total, quality_weight))| {
        let direction = if signed_effect > 0.0 {
            ContributionDirection::Positive
        } else if signed_effect < 0.0 {
            ContributionDirection::Negative
        } else {
            ContributionDirection::Neutral
        };
        FamilyContribution {
            family,
            signed_effect,
            absolute_effect,
            contribution_pct: if absolute_total > 0.0 { absolute_effect / absolute_total * 100.0 } else { 0.0 },
            quality: if quality_weight > 0.0 { Some(quality_total / quality_weight) } else { None },
            direction,
        }
    }).collect()
}

/// Attribute an already-computed variable or score using signed component effects.
pub fn attribute_signed_effects(
    method: AttributionMethod,
    baseline: f64,
    inputs: Vec<SignedEffectInput>,
    coverage_pct: f64,
) -> Result<AttributionReport, AttributionError> {
    report_from_effects(
        method,
        baseline,
        inputs.into_iter()
            .map(|input| (input.key, input.feature, input.signed_effect, input.quality))
            .collect(),
        coverage_pct,
    )
}

/// Build an additive attribution report from signed effects. Percentages use
/// absolute effects so positive and negative contributors are both visible;
/// direction and signed_effect preserve whether each raised or lowered score.
fn report_from_effects(
    method: AttributionMethod,
    baseline: f64,
    effects: Vec<(String, Option<MarketFeature>, f64, Option<f64>)>,
    coverage_pct: f64,
) -> Result<AttributionReport, AttributionError> {
    if !baseline.is_finite() || !coverage_pct.is_finite() || !(0.0..=100.0).contains(&coverage_pct) {
        return Err(AttributionError::InvalidInput);
    }
    let mut seen = BTreeSet::new();
    let mut net_effect = 0.0;
    let mut absolute_effect_total = 0.0;
    for (key, _, effect, quality) in &effects {
        if key.trim().is_empty() || !seen.insert(key.clone()) {
            return Err(AttributionError::InvalidKey);
        }
        if !effect.is_finite()
            || quality.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(AttributionError::InvalidInput);
        }
        net_effect += effect;
        absolute_effect_total += effect.abs();
    }
    if !net_effect.is_finite() || !absolute_effect_total.is_finite() {
        return Err(AttributionError::InvalidInput);
    }

    let contributions = effects.into_iter().map(|(key, feature, signed_effect, quality)| {
        let direction = if signed_effect > 0.0 {
            ContributionDirection::Positive
        } else if signed_effect < 0.0 {
            ContributionDirection::Negative
        } else {
            ContributionDirection::Neutral
        };
        let contribution_pct = if absolute_effect_total > 0.0 {
            signed_effect.abs() / absolute_effect_total * 100.0
        } else {
            0.0
        };
        Contribution { key, feature, signed_effect, contribution_pct, quality, direction }
    }).collect();

    Ok(AttributionReport {
        method,
        baseline,
        outcome: baseline + net_effect,
        net_effect,
        absolute_effect_total,
        coverage_pct,
        contributions,
    })
}

/// Attribute a linear vector score: effect_i = coefficient_i * value_i *
/// quality_i. Missing applicable coordinates reduce coverage; N/A coordinates
/// are excluded from the applicable-weight denominator rather than penalized.
pub fn attribute_vector_projection(
    vector: &MarketVector,
    baseline: f64,
    coefficients: &BTreeMap<MarketFeature, f64>,
) -> Result<AttributionReport, AttributionError> {
    vector.validate().map_err(|_| AttributionError::InvalidMarketVector)?;
    if coefficients.is_empty() {
        return Err(AttributionError::EmptyWeights);
    }
    let mut total_applicable_weight = 0.0;
    let mut observed_weight = 0.0;
    let mut effects = Vec::new();

    for (feature, coefficient) in coefficients {
        if !coefficient.is_finite() {
            return Err(AttributionError::InvalidInput);
        }
        let coordinate = vector.coordinates.get(feature)
            .ok_or(AttributionError::InvalidMarketVector)?;
        if coordinate.status == FeatureAvailability::NotApplicable {
            continue;
        }
        let absolute_weight = coefficient.abs();
        total_applicable_weight += absolute_weight;
        if let Some(value) = coordinate.value {
            observed_weight += absolute_weight * coordinate.quality;
            let effect = coefficient * value * coordinate.quality;
            effects.push((feature.as_str().to_owned(), Some(*feature), effect, Some(coordinate.quality)));
        }
    }
    if !total_applicable_weight.is_finite() || total_applicable_weight <= 0.0 {
        return Err(AttributionError::EmptyWeights);
    }
    let coverage_pct = (observed_weight / total_applicable_weight * 100.0).clamp(0.0, 100.0);
    report_from_effects(AttributionMethod::VectorProjection, baseline, effects, coverage_pct)
}

fn binomial(n: usize, k: usize) -> f64 {
    if k > n { return 0.0; }
    let k = k.min(n - k);
    let mut value = 1.0;
    for i in 0..k {
        value *= (n - i) as f64 / (i + 1) as f64;
    }
    value
}

/// Exact Shapley attribution from a complete coalition-value table. The table
/// is indexed by bitmask, where bit i means component i is present. This gives
/// each component a principled share of nonlinear interactions in a final
/// score. Limit n to 12 to keep exact computation and payload sizes bounded.
pub fn attribute_exact_shapley(
    component_keys: &[String],
    coalition_values: &[f64],
) -> Result<AttributionReport, AttributionError> {
    let n = component_keys.len();
    if n == 0 || n > 12 || coalition_values.len() != (1usize << n) {
        return Err(AttributionError::InvalidCoalitionTable);
    }
    let mut seen = BTreeSet::new();
    if component_keys.iter().any(|key| key.trim().is_empty() || !seen.insert(key.clone())) {
        return Err(AttributionError::InvalidKey);
    }
    if coalition_values.iter().any(|value| !value.is_finite()) {
        return Err(AttributionError::InvalidCoalitionValue);
    }
    let baseline = coalition_values[0];
    let full_mask = coalition_values.len() - 1;
    let mut effects = Vec::with_capacity(n);

    for (index, key) in component_keys.iter().enumerate() {
        let bit = 1usize << index;
        let mut shapley = 0.0;
        for mask in 0..coalition_values.len() {
            if mask & bit != 0 { continue; }
            let size = mask.count_ones() as usize;
            let weight = 1.0 / (n as f64 * binomial(n - 1, size));
            shapley += weight * (coalition_values[mask | bit] - coalition_values[mask]);
        }
        effects.push((key.clone(), None, shapley, None));
    }
    let mut report = report_from_effects(AttributionMethod::ExactShapley, baseline, effects, 100.0)?;
    // Use the measured full coalition as the authoritative outcome. The Shapley
    // efficiency axiom should make this equal to baseline + sum(phi_i).
    let full_outcome = coalition_values[full_mask];
    let tolerance = 1e-9 * (1.0 + full_outcome.abs());
    if (report.outcome - full_outcome).abs() > tolerance {
        return Err(AttributionError::InvalidCoalitionValue);
    }
    report.outcome = full_outcome;
    report.net_effect = full_outcome - baseline;
    Ok(report)
}

/// Compute a finite atomic measure over the union of evidence sets. An atom
/// shared by k sets contributes mass/k to each of those sets. This is the
/// Shapley allocation for the union-measure coverage game and guarantees that
/// overlapping evidence is counted once, not k times.
pub fn attribute_set_measure(
    sets: &[SetMeasureInput],
    atom_masses: &BTreeMap<String, f64>,
) -> Result<SetMeasureReport, AttributionError> {
    let mut ids = BTreeSet::new();
    let mut memberships: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for set in sets {
        if set.evidence_set_id.trim().is_empty() || !ids.insert(set.evidence_set_id.clone()) {
            return Err(AttributionError::InvalidKey);
        }
        for atom in &set.atoms {
            if atom.trim().is_empty() {
                return Err(AttributionError::InvalidKey);
            }
            memberships.entry(atom.clone()).or_default().push(set.evidence_set_id.clone());
        }
    }
    let mut allocated: BTreeMap<String, f64> =
        sets.iter().map(|set| (set.evidence_set_id.clone(), 0.0)).collect();
    let mut atom_allocations = Vec::new();
    let mut union_mass = 0.0;

    for (atom, set_ids) in memberships {
        let mass = *atom_masses.get(&atom)
            .ok_or(AttributionError::MissingOrInvalidAtomMeasure)?;
        if !mass.is_finite() || mass < 0.0 {
            return Err(AttributionError::MissingOrInvalidAtomMeasure);
        }
        union_mass += mass;
        let allocated_mass = mass / set_ids.len() as f64;
        for evidence_set_id in &set_ids {
            *allocated.get_mut(evidence_set_id).expect("set membership registered") += allocated_mass;
            atom_allocations.push(AtomMeasureAllocation {
                atom: atom.clone(),
                evidence_set_id: evidence_set_id.clone(),
                allocated_mass,
                membership_count: set_ids.len(),
            });
        }
    }
    if !union_mass.is_finite() {
        return Err(AttributionError::InvalidInput);
    }
    let contributions = allocated.into_iter().map(|(evidence_set_id, allocated_mass)| {
        let contribution_pct = if union_mass > 0.0 { allocated_mass / union_mass * 100.0 } else { 0.0 };
        SetMeasureContribution { evidence_set_id, allocated_mass, contribution_pct }
    }).collect();

    Ok(SetMeasureReport { union_mass, contributions, atom_allocations })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MarketVector;

    fn vector() -> MarketVector {
        let mut vector = MarketVector::new("attrib-test", "venue", "BTCUSD", "5m", 100, 101).unwrap();
        vector.set_value(MarketFeature::LogReturn, Some(0.5), 1.0).unwrap();
        vector.set_value(MarketFeature::Momentum, Some(-1.0), 1.0).unwrap();
        vector.set_not_applicable(MarketFeature::GammaExposure);
        vector
    }

    #[test]
    fn vector_percentages_show_positive_and_negative_contributions() {
        let mut weights = BTreeMap::new();
        weights.insert(MarketFeature::LogReturn, 2.0);
        weights.insert(MarketFeature::Momentum, 1.0);
        let report = attribute_vector_projection(&vector(), 0.0, &weights).unwrap();
        assert!((report.outcome - 0.0).abs() < 1e-12);
        assert!((report.contributions[0].contribution_pct - 50.0).abs() < 1e-12);
        assert!((report.contributions[1].contribution_pct - 50.0).abs() < 1e-12);
        assert_eq!(report.contributions[0].direction, ContributionDirection::Positive);
        assert_eq!(report.contributions[1].direction, ContributionDirection::Negative);
        assert!((report.contributions.iter().map(|item| item.contribution_pct).sum::<f64>() - 100.0).abs() < 1e-12);
    }

    #[test]
    fn missing_coordinates_reduce_coverage_but_not_applicable_do_not() {
        let mut weights = BTreeMap::new();
        weights.insert(MarketFeature::LogReturn, 1.0);
        weights.insert(MarketFeature::RealizedVolatility, 1.0); // missing
        weights.insert(MarketFeature::GammaExposure, 1.0); // N/A
        let report = attribute_vector_projection(&vector(), 0.0, &weights).unwrap();
        assert!((report.coverage_pct - 50.0).abs() < 1e-12);
        assert_eq!(report.contributions.len(), 1);
        assert!((report.contributions[0].contribution_pct - 100.0).abs() < 1e-12);
    }

    #[test]
    fn set_measure_allocates_overlap_once_and_percentages_sum_to_one_hundred() {
        let sets = vec![
            SetMeasureInput { evidence_set_id: "trend".into(), atoms: BTreeSet::from(["shared".into(), "up".into()]) },
            SetMeasureInput { evidence_set_id: "liquidity".into(), atoms: BTreeSet::from(["shared".into(), "depth".into()]) },
        ];
        let masses = BTreeMap::from([
            ("shared".into(), 2.0), ("up".into(), 1.0), ("depth".into(), 1.0),
        ]);
        let report = attribute_set_measure(&sets, &masses).unwrap();
        assert!((report.union_mass - 4.0).abs() < 1e-12);
        assert!((report.contributions[0].allocated_mass - 2.0).abs() < 1e-12);
        assert!((report.contributions[1].allocated_mass - 2.0).abs() < 1e-12);
        assert!((report.contributions.iter().map(|item| item.contribution_pct).sum::<f64>() - 100.0).abs() < 1e-12);
    }

    #[test]
    fn exact_shapley_allocates_nonlinear_interaction_fairly() {
        // Neither component alone scores much; together they create an interaction.
        let report = attribute_exact_shapley(
            &["vector".into(), "set_relation".into()],
            &[0.0, 1.0, 1.0, 4.0],
        ).unwrap();
        assert!((report.contributions[0].signed_effect - 2.0).abs() < 1e-12);
        assert!((report.contributions[1].signed_effect - 2.0).abs() < 1e-12);
        assert!((report.contributions[0].contribution_pct - 50.0).abs() < 1e-12);
        assert!((report.outcome - 4.0).abs() < 1e-12);
    }
}
