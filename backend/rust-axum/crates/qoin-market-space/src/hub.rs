//! Shared variable hub and dependency lifecycle for the common market state.
//!
//! One hub instance belongs to one venue/symbol/timeframe stream. It tracks the
//! progression of variables from observations through causal normalization and
//! dependency-checked derivation into immutable market-vector snapshots.
//! Algorithm versions never own separate copies of these shared variables.

use crate::{MarketFeature, MarketSpaceError, MarketVector};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableStage {
    Input,
    Derived,
    Regime,
    Portfolio,
    Temporal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableLifecycle {
    AwaitingInput,
    Blocked,
    Observed,
    Derived,
    Normalized,
    Published,
    Stale,
    NotApplicable,
}

#[derive(Debug, Clone, Copy)]
pub struct VariableDescriptor {
    pub feature: MarketFeature,
    pub stage: VariableStage,
    pub dependencies: &'static [MarketFeature],
}

const NONE: &[MarketFeature] = &[];
const LOG_RETURN: &[MarketFeature] = &[MarketFeature::LogReturn];
const PRICE_DISPLACEMENT: &[MarketFeature] = &[MarketFeature::PriceDisplacement];
const RATE_AND_RETURN: &[MarketFeature] = &[MarketFeature::RateOfChange, MarketFeature::LogReturn];
const REALIZED_VOL: &[MarketFeature] = &[MarketFeature::RealizedVolatility];
const RETURN_AND_VOL: &[MarketFeature] = &[MarketFeature::LogReturn, MarketFeature::RealizedVolatility];
const MOMENTUM: &[MarketFeature] = &[MarketFeature::Momentum];
const MOMENTUM_VOL: &[MarketFeature] = &[MarketFeature::Momentum, MarketFeature::RealizedVolatility];
const MOMENTUM_TIMEFRAME: &[MarketFeature] = &[MarketFeature::Momentum, MarketFeature::TimeframeAgreement];
const TREND_PERSISTENCE: &[MarketFeature] = &[MarketFeature::TrendStrength, MarketFeature::TrendPersistence];
const SPREAD_DEPTH: &[MarketFeature] = &[MarketFeature::SpreadBps, MarketFeature::OrderBookDepth];
const LIQUIDITY_STRESS_INPUTS: &[MarketFeature] = &[MarketFeature::SpreadBps, MarketFeature::OrderBookDepth, MarketFeature::MarketImpact, MarketFeature::LiquidityConcentration];
const FLOW_DEPTH: &[MarketFeature] = &[MarketFeature::OrderFlowImbalance, MarketFeature::OrderBookDepth];
const LIQUIDITY_COSTS: &[MarketFeature] = &[MarketFeature::SpreadBps, MarketFeature::MarketImpact];
const CROSS_ASSET: &[MarketFeature] = &[MarketFeature::Correlation, MarketFeature::Covariance, MarketFeature::Beta];
const TAIL_RISK_INPUTS: &[MarketFeature] = &[MarketFeature::RealizedVolatility, MarketFeature::ReturnKurtosis, MarketFeature::JumpIntensity, MarketFeature::Drawdown];
const FEATURE_PERSISTENCE_INPUTS: &[MarketFeature] = &[MarketFeature::LogReturn, MarketFeature::Momentum];
const SIGNAL_DECAY_INPUTS: &[MarketFeature] = &[MarketFeature::FeaturePersistence, MarketFeature::TrendExhaustion];
const REGIME_BASE: &[MarketFeature] = &[
    MarketFeature::RegimeTrendStrength,
    MarketFeature::RegimeVolatilityLevel,
    MarketFeature::RegimeLiquidityStress,
    MarketFeature::RegimeCorrelationStress,
    MarketFeature::RegimeTransitionIntensity,
];
const UNCERTAINTY_INPUTS: &[MarketFeature] = &[
    MarketFeature::TrendStrength,
    MarketFeature::RealizedVolatility,
    MarketFeature::SpreadBps,
    MarketFeature::Correlation,
    MarketFeature::EventIntensity,
];
const TRANSITION_INPUTS: &[MarketFeature] = &[
    MarketFeature::VolatilityOfVolatility,
    MarketFeature::CorrelationBreakdown,
    MarketFeature::TrendExhaustion,
    MarketFeature::TimeSinceStructuralBreak,
];

/// The single source of truth for the variable dependency graph.
/// Cross-asset, options and event producers enter as aligned inputs; portfolio and
/// temporal producers enter their own stages after validation.
pub fn descriptor(feature: MarketFeature) -> VariableDescriptor {
    use MarketFeature::*;
    let (stage, dependencies) = match feature {
        // Inputs from canonical event aggregation or aligned specialist producers.
        LogReturn | ImpliedVolatility | SpreadBps | OrderBookDepth
        | OrderFlowImbalance | VolumeDelta | Correlation | Covariance | Beta
        | RelativeStrength | FundingPressure | OpenInterestChange
        | LiquidationPressure | GammaExposure | DeltaExposure | Basis
        | EventIntensity | MacroSurprise | EventProximity
        | PortfolioExposure | Concentration | Drawdown | RiskContribution
        | TimeSinceStructuralBreak | TimeframeAgreement =>
            (VariableStage::Input, NONE),

        // Price and distribution-derived variables.
        PriceDisplacement => (VariableStage::Derived, LOG_RETURN),
        RateOfChange => (VariableStage::Derived, LOG_RETURN),
        GapMagnitude => (VariableStage::Derived, LOG_RETURN),
        SwingPosition => (VariableStage::Derived, PRICE_DISPLACEMENT),
        RealizedVolatility => (VariableStage::Derived, LOG_RETURN),
        VolatilityOfVolatility => (VariableStage::Derived, REALIZED_VOL),
        ReturnSkew | ReturnKurtosis => (VariableStage::Derived, LOG_RETURN),
        JumpIntensity => (VariableStage::Derived, RETURN_AND_VOL),

        // Trend variables.
        Momentum => (VariableStage::Derived, LOG_RETURN),
        TrendStrength => (VariableStage::Derived, MOMENTUM_VOL),
        TrendPersistence => (VariableStage::Derived, MOMENTUM_TIMEFRAME),
        MeanReversionStrength => (VariableStage::Derived, RATE_AND_RETURN),
        TrendExhaustion => (VariableStage::Derived, TREND_PERSISTENCE),

        // Execution/liquidity cost variables.
        MarketImpact => (VariableStage::Derived, FLOW_DEPTH),
        SlippageEstimate => (VariableStage::Derived, LIQUIDITY_COSTS),
        LiquidityConcentration => (VariableStage::Derived, SPREAD_DEPTH),

        // Cross-asset structural change.
        CorrelationBreakdown => (VariableStage::Derived, CROSS_ASSET),

        // Regime coordinates are derived from ordinary market variables.
        RegimeTrendStrength => (VariableStage::Regime, TREND_PERSISTENCE),
        RegimeVolatilityLevel => (VariableStage::Regime, VOL_TAIL),
        RegimeLiquidityStress => (VariableStage::Regime, LIQUIDITY_STRESS_INPUTS),
        RegimeCorrelationStress => (VariableStage::Regime, CROSS_ASSET),
        RegimeTransitionIntensity => (VariableStage::Regime, TRANSITION_INPUTS),
        RegimeUncertainty => (VariableStage::Regime, UNCERTAINTY_INPUTS),
        RegimePersistence => (VariableStage::Regime, REGIME_BASE),

        // Portfolio/temporal outputs.
        TailRisk => (VariableStage::Portfolio, TAIL_RISK_INPUTS),
        FeaturePersistence => (VariableStage::Temporal, FEATURE_PERSISTENCE_INPUTS),
        SignalDecay => (VariableStage::Temporal, SIGNAL_DECAY_INPUTS),
    };
    VariableDescriptor { feature, stage, dependencies }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizationProfile {
    /// Historical center estimated only from observations available by fit_cutoff_ms.
    pub center: f64,
    /// Positive historical scale (e.g. robust scale or standard deviation).
    pub scale: f64,
    pub fit_cutoff_ms: i64,
    pub version: String,
}

impl NormalizationProfile {
    fn validate(&self, event_time_ms: i64) -> Result<(), HubError> {
        if !self.center.is_finite()
            || !self.scale.is_finite()
            || self.scale <= 0.0
            || self.fit_cutoff_ms <= 0
            || self.fit_cutoff_ms > event_time_ms
            || self.version.trim().is_empty()
            || self.version.len() > 96
        {
            return Err(HubError::InvalidNormalizationProfile);
        }
        Ok(())
    }

    /// Past-only centered scale mapped into the open interval (-1, 1).
    pub fn apply(&self, raw_value: f64) -> f64 {
        ((raw_value - self.center) / self.scale).tanh().clamp(-1.0 + f64::EPSILON, 1.0 - f64::EPSILON)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableValue {
    pub feature: MarketFeature,
    pub raw_value: Option<f64>,
    pub normalized_value: Option<f64>,
    pub quality: f64,
    pub event_time_ms: i64,
    pub available_at_ms: i64,
    pub producer: String,
    pub calculation_version: String,
    pub normalization_profile: Option<NormalizationProfile>,
    pub dependency_revisions: BTreeMap<MarketFeature, u64>,
    pub revision: u64,
    pub not_applicable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableProgress {
    pub feature: MarketFeature,
    pub family: crate::FeatureFamily,
    pub stage: VariableStage,
    pub lifecycle: VariableLifecycle,
    pub dependency_count: usize,
    pub ready_dependencies: usize,
    pub blocked_by: Vec<MarketFeature>,
    pub revision: Option<u64>,
    pub quality: Option<f64>,
    pub event_time_ms: Option<i64>,
    pub available_at_ms: Option<i64>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HubError {
    #[error("feature must be supplied by an input producer")]
    WrongStage,
    #[error("variable hub identity does not match the published vector")]
    IdentityMismatch,
    #[error("raw observation must be finite, have positive quality, and valid timestamps")]
    InvalidObservation,
    #[error("producer and calculation version are required")]
    InvalidProvenance,
    #[error("derived variable is waiting for dependency {0:?}")]
    MissingDependency(MarketFeature),
    #[error("derived variable has stale dependency {0:?}")]
    StaleDependency(MarketFeature),
    #[error("derived event time or availability precedes an input dependency")]
    DerivedBeforeDependency,
    #[error("normalization profile is invalid or uses future calibration data")]
    InvalidNormalizationProfile,
    #[error("variable has no raw observation to normalize")]
    MissingRawObservation,
    #[error("variable hub dependency graph contains a cycle")]
    DependencyCycle,
    #[error(transparent)]
    MarketSpace(#[from] MarketSpaceError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariableHubIdentity {
    pub venue: String,
    pub symbol: String,
    pub timeframe: String,
}

#[derive(Debug, Clone)]
pub struct VariableHub {
    pub identity: VariableHubIdentity,
    values: BTreeMap<MarketFeature, VariableValue>,
    published_revisions: BTreeMap<MarketFeature, u64>,
    next_revision: u64,
}

impl VariableHub {
    /// Create a hub permanently scoped to one venue/symbol/timeframe stream.
    pub fn new(
        venue: impl Into<String>,
        symbol: impl Into<String>,
        timeframe: impl Into<String>,
    ) -> Result<Self, HubError> {
        let identity = VariableHubIdentity {
            venue: venue.into(),
            symbol: symbol.into(),
            timeframe: timeframe.into(),
        };
        crate::validate_identifier(&identity.venue, 64)?;
        crate::validate_identifier(&identity.symbol, 96)?;
        crate::validate_identifier(&identity.timeframe, 32)?;
        Ok(Self {
            identity,
            values: BTreeMap::new(),
            published_revisions: BTreeMap::new(),
            next_revision: 0,
        })
    }

    fn revision(&mut self) -> u64 {
        self.next_revision = self.next_revision.saturating_add(1);
        self.next_revision
    }

    fn validate_observation(
        raw_value: f64,
        quality: f64,
        event_time_ms: i64,
        available_at_ms: i64,
        producer: &str,
        calculation_version: &str,
    ) -> Result<(), HubError> {
        if !raw_value.is_finite()
            || !quality.is_finite()
            || !(0.0..=1.0).contains(&quality)
            || quality <= 0.0
            || event_time_ms <= 0
            || available_at_ms < event_time_ms
        {
            return Err(HubError::InvalidObservation);
        }
        if producer.trim().is_empty()
            || producer.len() > 96
            || calculation_version.trim().is_empty()
            || calculation_version.len() > 96
        {
            return Err(HubError::InvalidProvenance);
        }
        Ok(())
    }

    /// Record a canonical input from event aggregation or an aligned specialist
    /// producer. It remains Observed until a past-only profile normalizes it.
    pub fn observe_input(
        &mut self,
        feature: MarketFeature,
        raw_value: f64,
        quality: f64,
        event_time_ms: i64,
        available_at_ms: i64,
        producer: impl Into<String>,
        calculation_version: impl Into<String>,
    ) -> Result<u64, HubError> {
        if descriptor(feature).stage != VariableStage::Input {
            return Err(HubError::WrongStage);
        }
        let producer = producer.into();
        let calculation_version = calculation_version.into();
        Self::validate_observation(
            raw_value, quality, event_time_ms, available_at_ms,
            &producer, &calculation_version,
        )?;
        let revision = self.revision();
        self.values.insert(feature, VariableValue {
            feature,
            raw_value: Some(raw_value),
            normalized_value: None,
            quality,
            event_time_ms,
            available_at_ms,
            producer,
            calculation_version,
            normalization_profile: None,
            dependency_revisions: BTreeMap::new(),
            revision,
            not_applicable: false,
        });
        Ok(revision)
    }

    /// Record the raw output of a derived feature calculator. All declared
    /// dependencies must already be normalized and fresh; their exact revisions
    /// are captured so any upstream update invalidates this derived value.
    pub fn record_derived(
        &mut self,
        feature: MarketFeature,
        raw_value: f64,
        quality: f64,
        event_time_ms: i64,
        available_at_ms: i64,
        producer: impl Into<String>,
        calculation_version: impl Into<String>,
    ) -> Result<u64, HubError> {
        let definition = descriptor(feature);
        if definition.stage == VariableStage::Input || definition.dependencies.is_empty() {
            return Err(HubError::WrongStage);
        }
        let producer = producer.into();
        let calculation_version = calculation_version.into();
        Self::validate_observation(
            raw_value, quality, event_time_ms, available_at_ms,
            &producer, &calculation_version,
        )?;

        let mut dependency_revisions = BTreeMap::new();
        for dependency in definition.dependencies {
            let value = self.values.get(dependency)
                .ok_or(HubError::MissingDependency(*dependency))?;
            if !self.is_current(*dependency) {
                return Err(HubError::StaleDependency(*dependency));
            }
            if value.normalized_value.is_none() {
                return Err(HubError::MissingDependency(*dependency));
            }
            if value.event_time_ms > event_time_ms || value.available_at_ms > available_at_ms {
                return Err(HubError::DerivedBeforeDependency);
            }
            dependency_revisions.insert(*dependency, value.revision);
        }

        let revision = self.revision();
        self.values.insert(feature, VariableValue {
            feature,
            raw_value: Some(raw_value),
            normalized_value: None,
            quality,
            event_time_ms,
            available_at_ms,
            producer,
            calculation_version,
            normalization_profile: None,
            dependency_revisions,
            revision,
            not_applicable: false,
        });
        Ok(revision)
    }

    /// Normalize an observed or derived value. Calibration must end no later
    /// than the feature's event time, preventing future-distribution leakage.
    pub fn normalize(
        &mut self,
        feature: MarketFeature,
        profile: NormalizationProfile,
    ) -> Result<u64, HubError> {
        let value = self.values.get(&feature).ok_or(HubError::MissingRawObservation)?;
        let raw_value = value.raw_value.ok_or(HubError::MissingRawObservation)?;
        profile.validate(value.event_time_ms)?;
        for (dependency, revision) in &value.dependency_revisions {
            if self.values.get(dependency).map(|item| item.revision) != Some(*revision)
                || !self.is_current(*dependency)
            {
                return Err(HubError::StaleDependency(*dependency));
            }
        }
        let normalized = profile.apply(raw_value);
        if !normalized.is_finite() || !(-1.0..=1.0).contains(&normalized) {
            return Err(HubError::InvalidObservation);
        }
        let revision = self.revision();
        let value = self.values.get_mut(&feature).expect("value checked above");
        value.normalized_value = Some(normalized);
        value.normalization_profile = Some(profile);
        value.revision = revision;
        Ok(revision)
    }

    /// Mark a dimension as structurally inapplicable for this instrument or
    /// provider, rather than confusing it with a missing observation.
    pub fn mark_not_applicable(
        &mut self,
        feature: MarketFeature,
        event_time_ms: i64,
        available_at_ms: i64,
        producer: impl Into<String>,
        calculation_version: impl Into<String>,
    ) -> Result<u64, HubError> {
        let producer = producer.into();
        let calculation_version = calculation_version.into();
        if event_time_ms <= 0 || available_at_ms < event_time_ms {
            return Err(HubError::InvalidObservation);
        }
        if producer.trim().is_empty() || calculation_version.trim().is_empty() {
            return Err(HubError::InvalidProvenance);
        }
        let revision = self.revision();
        self.values.insert(feature, VariableValue {
            feature,
            raw_value: None,
            normalized_value: None,
            quality: 0.0,
            event_time_ms,
            available_at_ms,
            producer,
            calculation_version,
            normalization_profile: None,
            dependency_revisions: BTreeMap::new(),
            revision,
            not_applicable: true,
        });
        Ok(revision)
    }

    fn is_current(&self, feature: MarketFeature) -> bool {
        let Some(value) = self.values.get(&feature) else { return false; };
        if value.not_applicable || value.normalized_value.is_none() {
            return false;
        }
        for (dependency, revision) in &value.dependency_revisions {
            let Some(current) = self.values.get(dependency) else { return false; };
            if current.revision != *revision || !self.is_current(*dependency) {
                return false;
            }
        }
        true
    }

    pub fn value(&self, feature: MarketFeature) -> Option<&VariableValue> {
        self.values.get(&feature)
    }

    pub fn progress(&self, feature: MarketFeature) -> VariableProgress {
        let definition = descriptor(feature);
        let mut ready_dependencies = 0;
        let mut blocked_by = Vec::new();
        for dependency in definition.dependencies {
            if self.is_current(*dependency) {
                ready_dependencies += 1;
            } else {
                blocked_by.push(*dependency);
            }
        }
        let value = self.values.get(&feature);
        let lifecycle = match value {
            Some(value) if value.not_applicable => VariableLifecycle::NotApplicable,
            Some(value) if value.normalized_value.is_some() && !self.is_current(feature) => VariableLifecycle::Stale,
            Some(value) if value.normalized_value.is_some()
                && self.published_revisions.get(&feature) == Some(&value.revision) =>
                    VariableLifecycle::Published,
            Some(value) if value.normalized_value.is_some() => VariableLifecycle::Normalized,
            Some(_) if definition.stage == VariableStage::Input => VariableLifecycle::Observed,
            Some(_) => VariableLifecycle::Derived,
            None if !blocked_by.is_empty() => VariableLifecycle::Blocked,
            None => VariableLifecycle::AwaitingInput,
        };
        VariableProgress {
            feature,
            family: feature.family(),
            stage: definition.stage,
            lifecycle,
            dependency_count: definition.dependencies.len(),
            ready_dependencies,
            blocked_by,
            revision: value.map(|item| item.revision),
            quality: value.map(|item| item.quality),
            event_time_ms: value.map(|item| item.event_time_ms),
            available_at_ms: value.map(|item| item.available_at_ms),
        }
    }

    pub fn progress_all(&self) -> Vec<VariableProgress> {
        MarketFeature::ALL.into_iter().map(|feature| self.progress(feature)).collect()
    }

    /// Freeze the currently fresh, normalized values into the common immutable
    /// vector. Every other coordinate remains explicitly missing or N/A.
    pub fn publish(
        &mut self,
        vector_id: impl Into<String>,
        venue: impl Into<String>,
        symbol: impl Into<String>,
        timeframe: impl Into<String>,
        event_time_cutoff_ms: i64,
        available_at_ms: i64,
    ) -> Result<MarketVector, HubError> {
        let venue = venue.into();
        let symbol = symbol.into();
        let timeframe = timeframe.into();
        if venue != self.identity.venue
            || symbol != self.identity.symbol
            || timeframe != self.identity.timeframe
        {
            return Err(HubError::IdentityMismatch);
        }
        let mut vector = MarketVector::new(
            vector_id, venue, symbol, timeframe, event_time_cutoff_ms, available_at_ms,
        )?;
        let mut published = Vec::new();
        for feature in MarketFeature::ALL {
            let Some(value) = self.values.get(&feature) else { continue; };
            if value.event_time_ms > event_time_cutoff_ms || value.available_at_ms > available_at_ms {
                continue;
            }
            if value.not_applicable {
                vector.set_not_applicable(feature);
                continue;
            }
            if value.normalized_value.is_some() && self.is_current(feature) {
                vector.set_value(feature, value.normalized_value, value.quality)?;
                published.push((feature, value.revision));
            }
        }
        vector.validate()?;
        for (feature, revision) in published {
            self.published_revisions.insert(feature, revision);
        }
        Ok(vector)
    }
}

/// Return a deterministic topological ordering or reject a cyclic dependency
/// graph. This is also a schema test: every fixed coordinate must have a node.
pub fn validate_dependency_graph() -> Result<Vec<MarketFeature>, HubError> {
    let mut indegree: BTreeMap<MarketFeature, usize> =
        MarketFeature::ALL.into_iter().map(|feature| (feature, 0)).collect();
    let mut dependents: BTreeMap<MarketFeature, Vec<MarketFeature>> = BTreeMap::new();
    for feature in MarketFeature::ALL {
        for dependency in descriptor(feature).dependencies {
            if !indegree.contains_key(dependency) {
                return Err(HubError::DependencyCycle);
            }
            *indegree.get_mut(&feature).expect("all schema features registered") += 1;
            dependents.entry(*dependency).or_default().push(feature);
        }
    }
    let mut ready: BTreeSet<MarketFeature> = indegree.iter()
        .filter_map(|(feature, degree)| (*degree == 0).then_some(*feature))
        .collect();
    let mut ordered = Vec::with_capacity(MarketFeature::ALL.len());
    while let Some(feature) = ready.pop_first() {
        ordered.push(feature);
        if let Some(children) = dependents.get(&feature) {
            for child in children {
                let degree = indegree.get_mut(child).expect("all schema features registered");
                *degree -= 1;
                if *degree == 0 { ready.insert(*child); }
            }
        }
    }
    if ordered.len() != MarketFeature::ALL.len() {
        return Err(HubError::DependencyCycle);
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FeatureAvailability;

    fn profile(cutoff: i64) -> NormalizationProfile {
        NormalizationProfile {
            center: 0.0,
            scale: 1.0,
            fit_cutoff_ms: cutoff,
            version: "test-robust-v1".into(),
        }
    }

    fn input(hub: &mut VariableHub, feature: MarketFeature, raw: f64) {
        hub.observe_input(feature, raw, 0.95, 1_800_000_000_000, 1_800_000_000_001,
            "test-feed", "test-input-v1").unwrap();
        hub.normalize(feature, profile(1_799_999_999_999)).unwrap();
    }

    #[test]
    fn every_coordinate_has_a_valid_acyclic_dependency_path() {
        let order = validate_dependency_graph().unwrap();
        assert_eq!(order.len(), MarketFeature::ALL.len());
        assert_eq!(descriptor(MarketFeature::RegimeVolatilityLevel).stage, VariableStage::Regime);
        assert!(descriptor(MarketFeature::RegimeVolatilityLevel).dependencies.contains(&MarketFeature::RealizedVolatility));
    }

    #[test]
    fn variable_progresses_from_observed_to_normalized_to_published() {
        let mut hub = VariableHub::new("venue", "BTCUSD", "5m").unwrap();
        hub.observe_input(MarketFeature::LogReturn, 0.02, 0.9,
            1_800_000_000_000, 1_800_000_000_001, "trade-aggregator", "returns-v1").unwrap();
        assert_eq!(hub.progress(MarketFeature::LogReturn).lifecycle, VariableLifecycle::Observed);
        hub.normalize(MarketFeature::LogReturn, profile(1_799_999_999_999)).unwrap();
        assert_eq!(hub.progress(MarketFeature::LogReturn).lifecycle, VariableLifecycle::Normalized);
        let vector = hub.publish("vector-1", "venue", "BTCUSD", "5m",
            1_800_000_000_000, 1_800_000_000_002).unwrap();
        assert!(vector.value(MarketFeature::LogReturn).is_some());
        assert_eq!(hub.progress(MarketFeature::LogReturn).lifecycle, VariableLifecycle::Published);
    }

    #[test]
    fn derived_variable_waits_for_dependencies_then_becomes_stale_on_upstream_change() {
        let mut hub = VariableHub::new("venue", "BTCUSD", "5m").unwrap();
        assert_eq!(hub.progress(MarketFeature::RealizedVolatility).lifecycle, VariableLifecycle::Blocked);
        input(&mut hub, MarketFeature::LogReturn, 0.01);
        hub.record_derived(MarketFeature::RealizedVolatility, 0.012, 0.9,
            1_800_000_000_000, 1_800_000_000_002, "rolling-volatility", "ewma-v1").unwrap();
        hub.normalize(MarketFeature::RealizedVolatility, profile(1_799_999_999_999)).unwrap();
        assert_eq!(hub.progress(MarketFeature::RealizedVolatility).lifecycle, VariableLifecycle::Normalized);
        input(&mut hub, MarketFeature::LogReturn, 0.03);
        assert_eq!(hub.progress(MarketFeature::RealizedVolatility).lifecycle, VariableLifecycle::Stale);
    }

    #[test]
    fn derived_features_cannot_use_future_calibration_or_future_dependencies() {
        let mut hub = VariableHub::new("venue", "BTCUSD", "5m").unwrap();
        input(&mut hub, MarketFeature::LogReturn, 0.01);
        assert_eq!(
            hub.normalize(MarketFeature::LogReturn, profile(1_800_000_000_001)),
            Err(HubError::InvalidNormalizationProfile),
        );
        assert_eq!(
            hub.record_derived(MarketFeature::RealizedVolatility, 0.01, 0.9,
                1_800_000_000_000, 1_800_000_000_000, "vol", "v1"),
            Err(HubError::DerivedBeforeDependency),
        );
    }

    #[test]
    fn regime_coordinate_is_blocked_until_its_full_dependency_set_is_ready() {
        let hub = VariableHub::new("venue", "BTCUSD", "5m").unwrap();
        let progress = hub.progress(MarketFeature::RegimeVolatilityLevel);
        assert_eq!(progress.stage, VariableStage::Regime);
        assert_eq!(progress.lifecycle, VariableLifecycle::Blocked);
        assert_eq!(progress.blocked_by.len(), 3);
    }

    #[test]
    fn normalization_is_bounded_and_not_applicable_is_preserved() {
        let mut hub = VariableHub::new("venue", "BTCUSD", "5m").unwrap();
        input(&mut hub, MarketFeature::LogReturn, 1_000_000.0);
        assert!(hub.value(MarketFeature::LogReturn).unwrap().normalized_value.unwrap() < 1.0);
        hub.mark_not_applicable(MarketFeature::GammaExposure, 1_800_000_000_000,
            1_800_000_000_001, "instrument-capabilities", "capability-map-v1").unwrap();
        let vector = hub.publish("vector-2", "venue", "BTCUSD", "5m",
            1_800_000_000_000, 1_800_000_000_002).unwrap();
        assert_eq!(vector.coordinates[&MarketFeature::GammaExposure].status, FeatureAvailability::NotApplicable);
    }
}
