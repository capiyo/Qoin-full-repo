//! Version 2 research primitives: permutation-invariant set embeddings,
//! set algebra, deterministic set attention, and append-only relationship events.
//!
//! This crate is deliberately isolated from the Version 1 vector engine. It does
//! not generate orders, mutate the active strategy, or claim predictive edge.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use qoin_market_space::{attribution::{attribute_set_measure, SetMeasureInput, SetMeasureReport}, FeatureAvailability, MarketFeature, MarketVector};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VectorEvidenceConfig {
    pub negative_threshold: f64,
    pub positive_threshold: f64,
    pub minimum_quality: f64,
    pub include_missing: bool,
    pub include_not_applicable: bool,
}

impl Default for VectorEvidenceConfig {
    fn default() -> Self {
        Self {
            negative_threshold: -0.25,
            positive_threshold: 0.25,
            minimum_quality: 0.5,
            include_missing: true,
            include_not_applicable: true,
        }
    }
}

impl VectorEvidenceConfig {
    pub fn validate(&self) -> Result<(), SetAttentionError> {
        if !self.negative_threshold.is_finite()
            || !self.positive_threshold.is_finite()
            || !self.minimum_quality.is_finite()
            || !(-1.0..0.0).contains(&self.negative_threshold)
            || !(0.0..=1.0).contains(&self.positive_threshold)
            || self.positive_threshold <= 0.0
            || !(0.0..=1.0).contains(&self.minimum_quality)
        {
            return Err(SetAttentionError::InvalidVectorEvidenceConfig);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceSet {
    pub id: String,
    pub domain: String,
    pub observed_at_ms: i64,
    pub atoms: BTreeSet<String>,
    #[serde(default)]
    pub source_vector_id: Option<String>,
    #[serde(default)]
    pub source_vector_schema_version: Option<u16>,
    #[serde(default)]
    pub source_venue: Option<String>,
    #[serde(default)]
    pub source_symbol: Option<String>,
    #[serde(default)]
    pub source_timeframe: Option<String>,
    #[serde(default)]
    pub source_vector_snapshot: Option<MarketVector>,
}

impl EvidenceSet {
    pub fn new<I, S>(
        id: impl Into<String>,
        domain: impl Into<String>,
        observed_at_ms: i64,
        atoms: I,
    ) -> Result<Self, SetAttentionError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let set = Self {
            id: id.into(),
            domain: domain.into(),
            observed_at_ms,
            atoms: atoms.into_iter().map(Into::into).collect(),
            source_vector_id: None,
            source_vector_schema_version: None,
            source_venue: None,
            source_symbol: None,
            source_timeframe: None,
            source_vector_snapshot: None,
        };
        set.validate()?;
        Ok(set)
    }


    /// Convert the common numerical state into a deterministic set of typed
    /// conditions. Continuous magnitudes remain in the shared vector; V2 uses
    /// explicit bins and preserves the originating vector identity/schema.
    pub fn from_market_vector(
        vector: &MarketVector,
        config: &VectorEvidenceConfig,
    ) -> Result<Self, SetAttentionError> {
        vector.validate().map_err(|_| SetAttentionError::InvalidMarketVector)?;
        config.validate()?;
        let mut atoms = BTreeSet::new();
        for feature in MarketFeature::ALL {
            let coordinate = vector.coordinates.get(&feature)
                .ok_or(SetAttentionError::InvalidMarketVector)?;
            let name = feature.as_str();
            match coordinate.status {
                FeatureAvailability::Missing => {
                    if config.include_missing {
                        atoms.insert(format!("missing:{name}"));
                    }
                }
                FeatureAvailability::NotApplicable => {
                    if config.include_not_applicable {
                        atoms.insert(format!("not_applicable:{name}"));
                    }
                }
                FeatureAvailability::Observed if coordinate.quality < config.minimum_quality => {
                    atoms.insert(format!("low_quality:{name}"));
                }
                FeatureAvailability::Observed if coordinate.value.is_some_and(|value| value <= config.negative_threshold) => {
                    atoms.insert(format!("{name}:negative"));
                }
                FeatureAvailability::Observed if coordinate.value.is_some_and(|value| value >= config.positive_threshold) => {
                    atoms.insert(format!("{name}:positive"));
                }
                FeatureAvailability::Observed => {
                    atoms.insert(format!("{name}:neutral"));
                }
            }
        }
        let set = Self {
            id: format!("market-vector:{}", vector.vector_id),
            domain: "market_state".into(),
            observed_at_ms: vector.event_time_cutoff_ms,
            atoms,
            source_vector_id: Some(vector.vector_id.clone()),
            source_vector_schema_version: Some(vector.schema_version),
            source_venue: Some(vector.venue.clone()),
            source_symbol: Some(vector.symbol.clone()),
            source_timeframe: Some(vector.timeframe.clone()),
            source_vector_snapshot: Some(vector.clone()),
        };
        set.validate()?;
        Ok(set)
    }

    pub fn validate(&self) -> Result<(), SetAttentionError> {
        let vector_provenance_valid = match &self.source_vector_snapshot {
            Some(vector) => vector.validate().is_ok()
                && self.source_vector_id.as_deref() == Some(vector.vector_id.as_str())
                && self.source_vector_schema_version == Some(vector.schema_version)
                && self.source_venue.as_deref() == Some(vector.venue.as_str())
                && self.source_symbol.as_deref() == Some(vector.symbol.as_str())
                && self.source_timeframe.as_deref() == Some(vector.timeframe.as_str())
                && self.observed_at_ms == vector.event_time_cutoff_ms,
            None => self.source_vector_id.is_none()
                && self.source_vector_schema_version.is_none()
                && self.source_venue.is_none()
                && self.source_symbol.is_none()
                && self.source_timeframe.is_none(),
        };
        if !vector_provenance_valid || self.id.trim().is_empty() || self.id.len() > 160
            || self.domain.trim().is_empty() || self.domain.len() > 160
            || self.observed_at_ms <= 0
            || self.atoms.len() > 512
            || self.atoms.iter().any(|atom| atom.trim().is_empty() || atom.len() > 256)
            || self.source_vector_id.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.source_vector_schema_version == Some(0)
            || self.source_venue.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 64)
            || self.source_symbol.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.source_timeframe.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 32)
            || (self.source_vector_id.is_some() != self.source_vector_schema_version.is_some())
        {
            return Err(SetAttentionError::InvalidEvidenceSet);
        }
        Ok(())
    }

    pub fn intersection(&self, other: &Self) -> BTreeSet<String> {
        self.atoms.intersection(&other.atoms).cloned().collect()
    }

    pub fn union(&self, other: &Self) -> BTreeSet<String> {
        self.atoms.union(&other.atoms).cloned().collect()
    }

    pub fn difference(&self, other: &Self) -> BTreeSet<String> {
        self.atoms.difference(&other.atoms).cloned().collect()
    }

    pub fn symmetric_difference(&self, other: &Self) -> BTreeSet<String> {
        self.atoms.symmetric_difference(&other.atoms).cloned().collect()
    }

    pub fn relation_metrics(&self, other: &Self) -> SetRelationMetrics {
        let intersection = self.atoms.intersection(&other.atoms).count();
        let union = self.atoms.union(&other.atoms).count();
        let source_size = self.atoms.len();
        let target_size = other.atoms.len();
        SetRelationMetrics {
            intersection_size: intersection,
            union_size: union,
            source_size,
            target_size,
            jaccard: if union == 0 { 1.0 } else { intersection as f64 / union as f64 },
            source_coverage: if source_size == 0 {
                if target_size == 0 { 1.0 } else { 0.0 }
            } else {
                intersection as f64 / source_size as f64
            },
            target_coverage: if target_size == 0 {
                if source_size == 0 { 1.0 } else { 0.0 }
            } else {
                intersection as f64 / target_size as f64
            },
        }
    }

    pub fn relation_kind(&self, other: &Self) -> SetRelationKind {
        if self.atoms == other.atoms {
            SetRelationKind::Equal
        } else if self.atoms.is_subset(&other.atoms) {
            SetRelationKind::SourceSubsetOfTarget
        } else if other.atoms.is_subset(&self.atoms) {
            SetRelationKind::TargetSubsetOfSource
        } else if self.atoms.is_disjoint(&other.atoms) {
            SetRelationKind::Disjoint
        } else {
            SetRelationKind::Overlap
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetRelationMetrics {
    pub intersection_size: usize,
    pub union_size: usize,
    pub source_size: usize,
    pub target_size: usize,
    pub jaccard: f64,
    pub source_coverage: f64,
    pub target_coverage: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetRelationKind {
    Equal,
    SourceSubsetOfTarget,
    TargetSubsetOfSource,
    Overlap,
    Disjoint,
}

/// Stable, sorted vocabulary. Building it from a collection of sets is
/// permutation-invariant and produces the same coordinate system for the same
/// atom universe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vocabulary {
    pub atoms: Vec<String>,
}

impl Vocabulary {
    pub fn from_sets(sets: &[EvidenceSet]) -> Result<Self, SetAttentionError> {
        let mut atoms = BTreeSet::new();
        for set in sets {
            set.validate()?;
            atoms.extend(set.atoms.iter().cloned());
        }
        Ok(Self { atoms: atoms.into_iter().collect() })
    }

    /// Binary incidence embedding with L2 normalization. This is an explicit
    /// set embedding, not a learned semantic embedding.
    pub fn embed(&self, set: &EvidenceSet) -> Result<SetEmbedding, SetAttentionError> {
        set.validate()?;
        let coordinates = self.atoms.iter()
            .map(|atom| if set.atoms.contains(atom) { 1.0 } else { 0.0 })
            .collect::<Vec<_>>();
        let norm = coordinates.iter().map(|value| value * value).sum::<f64>().sqrt();
        let values = if norm > f64::EPSILON {
            coordinates.into_iter().map(|value| value / norm).collect()
        } else {
            coordinates
        };
        Ok(SetEmbedding { values })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetEmbedding {
    pub values: Vec<f64>,
}

impl SetEmbedding {
    pub fn validate(&self) -> Result<(), SetAttentionError> {
        if self.values.is_empty() || self.values.iter().any(|value| !value.is_finite()) {
            return Err(SetAttentionError::InvalidEmbedding);
        }
        Ok(())
    }
}

/// Deterministic temperature-scaled cosine attention over L2-normalized set
/// embeddings. The returned weights are normalized across candidate keys. Since
/// the embeddings are unit-normalized, their dot product is already cosine
/// similarity; applying the transformer's additional sqrt(d) divisor would
/// suppress scores as vocabulary size grows. No parameters are trained here.
pub fn set_attention_weights(
    query: &SetEmbedding,
    keys: &[SetEmbedding],
    temperature: f64,
) -> Result<Vec<f64>, SetAttentionError> {
    query.validate()?;
    if keys.is_empty() || !temperature.is_finite() || temperature <= 0.0 {
        return Err(SetAttentionError::InvalidAttentionConfig);
    }
    for key in keys {
        key.validate()?;
        if key.values.len() != query.values.len() {
            return Err(SetAttentionError::EmbeddingDimensionMismatch);
        }
    }

    let scale = temperature;
    let logits = keys.iter().map(|key| {
        query.values.iter().zip(&key.values)
            .map(|(left, right)| left * right)
            .sum::<f64>() / scale
    }).collect::<Vec<_>>();
    let max_logit = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exponentials = logits.iter().map(|logit| (logit - max_logit).exp()).collect::<Vec<_>>();
    let denominator = exponentials.iter().sum::<f64>();
    if !denominator.is_finite() || denominator <= 0.0 {
        return Err(SetAttentionError::InvalidAttentionConfig);
    }
    Ok(exponentials.into_iter().map(|value| value / denominator).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipRecord {
    pub relationship_id: String,
    pub source_set_id: String,
    pub target_set_id: String,
    pub source_domain: String,
    pub target_domain: String,
    pub source_observed_at_ms: i64,
    pub target_observed_at_ms: i64,
    pub source_atoms: Vec<String>,
    pub target_atoms: Vec<String>,
    pub observed_at_ms: i64,
    pub model_version: String,
    pub relation_kind: SetRelationKind,
    pub metrics: SetRelationMetrics,
    pub attention_weight: f64,
    pub attention_temperature: f64,
    #[serde(default)]
    pub source_vector_id: Option<String>,
    #[serde(default)]
    pub target_vector_id: Option<String>,
    #[serde(default)]
    pub source_vector_schema_version: Option<u16>,
    #[serde(default)]
    pub target_vector_schema_version: Option<u16>,
    #[serde(default)]
    pub source_venue: Option<String>,
    #[serde(default)]
    pub source_symbol: Option<String>,
    #[serde(default)]
    pub source_timeframe: Option<String>,
    #[serde(default)]
    pub target_venue: Option<String>,
    #[serde(default)]
    pub target_symbol: Option<String>,
    #[serde(default)]
    pub target_timeframe: Option<String>,
    #[serde(default)]
    pub source_vector_snapshot: Option<MarketVector>,
    #[serde(default)]
    pub target_vector_snapshot: Option<MarketVector>,
    /// Pairwise atomic measure of the source/target evidence union. Shared
    /// atoms are split across memberships and retained in the immutable ledger.
    #[serde(default)]
    pub set_measure: Option<SetMeasureReport>,
}

impl RelationshipRecord {
    pub fn discover(
        source: &EvidenceSet,
        target: &EvidenceSet,
        model_version: impl Into<String>,
        attention_weight: f64,
        attention_temperature: f64,
    ) -> Result<Self, SetAttentionError> {
        source.validate()?;
        target.validate()?;
        let model_version = model_version.into();
        let pair_sets = vec![
            SetMeasureInput { evidence_set_id: source.id.clone(), atoms: source.atoms.clone() },
            SetMeasureInput { evidence_set_id: target.id.clone(), atoms: target.atoms.clone() },
        ];
        let atom_masses = source.atoms.union(&target.atoms)
            .map(|atom| (atom.clone(), 1.0))
            .collect::<BTreeMap<_, _>>();
        let set_measure = attribute_set_measure(&pair_sets, &atom_masses)
            .map_err(|_| SetAttentionError::InvalidSetMeasure)?;
        let record = Self {
            relationship_id: format!("{}::{}", source.id, target.id),
            source_set_id: source.id.clone(),
            target_set_id: target.id.clone(),
            source_domain: source.domain.clone(),
            target_domain: target.domain.clone(),
            source_observed_at_ms: source.observed_at_ms,
            target_observed_at_ms: target.observed_at_ms,
            source_atoms: source.atoms.iter().cloned().collect(),
            target_atoms: target.atoms.iter().cloned().collect(),
            observed_at_ms: source.observed_at_ms.max(target.observed_at_ms),
            model_version,
            relation_kind: source.relation_kind(target),
            metrics: source.relation_metrics(target),
            attention_weight,
            attention_temperature,
            source_vector_id: source.source_vector_id.clone(),
            target_vector_id: target.source_vector_id.clone(),
            source_vector_schema_version: source.source_vector_schema_version,
            target_vector_schema_version: target.source_vector_schema_version,
            source_venue: source.source_venue.clone(),
            source_symbol: source.source_symbol.clone(),
            source_timeframe: source.source_timeframe.clone(),
            target_venue: target.source_venue.clone(),
            target_symbol: target.source_symbol.clone(),
            target_timeframe: target.source_timeframe.clone(),
            source_vector_snapshot: source.source_vector_snapshot.clone(),
            target_vector_snapshot: target.source_vector_snapshot.clone(),
            set_measure: Some(set_measure),
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), SetAttentionError> {
        let metrics = &self.metrics;
        let source_vector_provenance_valid = match &self.source_vector_snapshot {
            Some(vector) => vector.validate().is_ok()
                && self.source_vector_id.as_deref() == Some(vector.vector_id.as_str())
                && self.source_vector_schema_version == Some(vector.schema_version)
                && self.source_venue.as_deref() == Some(vector.venue.as_str())
                && self.source_symbol.as_deref() == Some(vector.symbol.as_str())
                && self.source_timeframe.as_deref() == Some(vector.timeframe.as_str())
                && self.source_observed_at_ms == vector.event_time_cutoff_ms,
            None => self.source_vector_id.is_none()
                && self.source_vector_schema_version.is_none()
                && self.source_venue.is_none()
                && self.source_symbol.is_none()
                && self.source_timeframe.is_none(),
        };
        let target_vector_provenance_valid = match &self.target_vector_snapshot {
            Some(vector) => vector.validate().is_ok()
                && self.target_vector_id.as_deref() == Some(vector.vector_id.as_str())
                && self.target_vector_schema_version == Some(vector.schema_version)
                && self.target_venue.as_deref() == Some(vector.venue.as_str())
                && self.target_symbol.as_deref() == Some(vector.symbol.as_str())
                && self.target_timeframe.as_deref() == Some(vector.timeframe.as_str())
                && self.target_observed_at_ms == vector.event_time_cutoff_ms,
            None => self.target_vector_id.is_none()
                && self.target_vector_schema_version.is_none()
                && self.target_venue.is_none()
                && self.target_symbol.is_none()
                && self.target_timeframe.is_none(),
        };
        let expected_union = metrics.source_size.saturating_add(metrics.target_size)
            .saturating_sub(metrics.intersection_size);
        let expected_jaccard = if metrics.union_size == 0 { 1.0 }
            else { metrics.intersection_size as f64 / metrics.union_size as f64 };
        let expected_source_coverage = if metrics.source_size == 0 {
            if metrics.target_size == 0 { 1.0 } else { 0.0 }
        } else { metrics.intersection_size as f64 / metrics.source_size as f64 };
        let expected_target_coverage = if metrics.target_size == 0 {
            if metrics.source_size == 0 { 1.0 } else { 0.0 }
        } else { metrics.intersection_size as f64 / metrics.target_size as f64 };
        let sorted_unique = |atoms: &[String]| atoms.windows(2).all(|pair| pair[0] < pair[1]);
        let source_atoms = self.source_atoms.iter().cloned().collect::<BTreeSet<_>>();
        let target_atoms = self.target_atoms.iter().cloned().collect::<BTreeSet<_>>();
        if let Some(measure) = &self.set_measure {
            let pair_sets = vec![
                SetMeasureInput { evidence_set_id: self.source_set_id.clone(), atoms: source_atoms.clone() },
                SetMeasureInput { evidence_set_id: self.target_set_id.clone(), atoms: target_atoms.clone() },
            ];
            let atom_masses = source_atoms.union(&target_atoms)
                .map(|atom| (atom.clone(), 1.0))
                .collect::<BTreeMap<_, _>>();
            let expected_measure = attribute_set_measure(&pair_sets, &atom_masses)
                .map_err(|_| SetAttentionError::InvalidSetMeasure)?;
            if measure != &expected_measure {
                return Err(SetAttentionError::InvalidSetMeasure);
            }
        }
        let actual_intersection = source_atoms.intersection(&target_atoms).count();
        let actual_union = source_atoms.union(&target_atoms).count();
        let expected_relation_kind = if source_atoms == target_atoms {
            SetRelationKind::Equal
        } else if source_atoms.is_subset(&target_atoms) {
            SetRelationKind::SourceSubsetOfTarget
        } else if target_atoms.is_subset(&source_atoms) {
            SetRelationKind::TargetSubsetOfSource
        } else if source_atoms.is_disjoint(&target_atoms) {
            SetRelationKind::Disjoint
        } else {
            SetRelationKind::Overlap
        };
        if !source_vector_provenance_valid || !target_vector_provenance_valid
            || self.relationship_id != format!("{}::{}", self.source_set_id, self.target_set_id)
            || self.source_set_id.trim().is_empty() || self.source_set_id.len() > 160
            || self.target_set_id.trim().is_empty() || self.target_set_id.len() > 160
            || self.source_domain.trim().is_empty() || self.source_domain.len() > 160
            || self.target_domain.trim().is_empty() || self.target_domain.len() > 160
            || self.source_observed_at_ms <= 0 || self.target_observed_at_ms <= 0
            || self.observed_at_ms != self.source_observed_at_ms.max(self.target_observed_at_ms)
            || self.source_atoms.len() > 512 || self.target_atoms.len() > 512
            || self.source_atoms.len() != metrics.source_size
            || self.target_atoms.len() != metrics.target_size
            || !sorted_unique(&self.source_atoms) || !sorted_unique(&self.target_atoms)
            || self.source_atoms.iter().chain(&self.target_atoms)
                .any(|atom| atom.trim().is_empty() || atom.len() > 256)
            || self.model_version.trim().is_empty() || self.model_version.len() > 128
            || !self.attention_weight.is_finite()
            || !(0.0..=1.0).contains(&self.attention_weight)
            || !self.attention_temperature.is_finite() || self.attention_temperature <= 0.0
            || self.source_vector_id.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.target_vector_id.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.source_vector_schema_version == Some(0) || self.target_vector_schema_version == Some(0)
            || self.source_vector_id.is_some() != self.source_vector_schema_version.is_some()
            || self.target_vector_id.is_some() != self.target_vector_schema_version.is_some()
            || self.source_venue.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 64)
            || self.source_symbol.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.source_timeframe.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 32)
            || self.target_venue.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 64)
            || self.target_symbol.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 96)
            || self.target_timeframe.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 32)
            || metrics.intersection_size > metrics.source_size.min(metrics.target_size)
            || metrics.intersection_size != actual_intersection
            || metrics.union_size != actual_union || metrics.union_size != expected_union
            || self.relation_kind != expected_relation_kind
            || !metrics.jaccard.is_finite() || !metrics.source_coverage.is_finite()
            || !metrics.target_coverage.is_finite()
            || (metrics.jaccard - expected_jaccard).abs() > 1e-12
            || (metrics.source_coverage - expected_source_coverage).abs() > 1e-12
            || (metrics.target_coverage - expected_target_coverage).abs() > 1e-12
        {
            return Err(SetAttentionError::InvalidRelationship);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipEventType {
    Discovered,
    OutcomeObserved,
    Invalidated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipOutcome {
    pub evaluated_at_ms: i64,
    pub horizon_ms: i64,
    pub realized_return_bps: Option<f64>,
    pub max_adverse_excursion_bps: Option<f64>,
    pub max_favorable_excursion_bps: Option<f64>,
}

impl RelationshipOutcome {
    fn validate(&self) -> Result<(), SetAttentionError> {
        if self.evaluated_at_ms <= 0 || self.horizon_ms <= 0
            || [self.realized_return_bps, self.max_adverse_excursion_bps,
                self.max_favorable_excursion_bps]
                .into_iter().flatten().any(|value| !value.is_finite())
        {
            return Err(SetAttentionError::InvalidOutcome);
        }
        Ok(())
    }
}

/// Immutable event intended for append-only persistence. Corrections, later
/// outcomes and invalidations are new events, never updates to prior records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipLedgerEvent {
    pub event_id: String,
    pub event_type: RelationshipEventType,
    pub recorded_at_ms: i64,
    pub relationship: RelationshipRecord,
    pub outcome: Option<RelationshipOutcome>,
}

impl RelationshipLedgerEvent {
    pub fn validate(&self) -> Result<(), SetAttentionError> {
        self.relationship.validate()?;
        if self.event_id.trim().is_empty() || self.event_id.len() > 256
            || self.recorded_at_ms < self.relationship.observed_at_ms
        {
            return Err(SetAttentionError::InvalidLedgerEvent);
        }
        match (self.event_type, &self.outcome) {
            (RelationshipEventType::Discovered, None)
            | (RelationshipEventType::Invalidated, None) => {}
            (RelationshipEventType::OutcomeObserved, Some(outcome)) => {
                outcome.validate()?;
                if outcome.evaluated_at_ms < self.recorded_at_ms {
                    return Err(SetAttentionError::InvalidOutcome);
                }
            }
            (RelationshipEventType::OutcomeObserved, None) => {
                return Err(SetAttentionError::MissingOutcome);
            }
            _ => return Err(SetAttentionError::InvalidLedgerEvent),
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetAttentionError {
    #[error("invalid evidence set")]
    InvalidEvidenceSet,
    #[error("invalid or empty embedding")]
    InvalidEmbedding,
    #[error("embedding dimensions do not match")]
    EmbeddingDimensionMismatch,
    #[error("invalid attention configuration")]
    InvalidAttentionConfig,
    #[error("invalid relationship record")]
    InvalidRelationship,
    #[error("invalid relationship ledger event")]
    InvalidLedgerEvent,
    #[error("outcome event requires an outcome payload")]
    MissingOutcome,
    #[error("invalid relationship outcome")]
    InvalidOutcome,
    #[error("invalid shared market vector")]
    InvalidMarketVector,
    #[error("invalid or inconsistent pairwise set measure")]
    InvalidSetMeasure,
    #[error("invalid vector-to-evidence binning configuration")]
    InvalidVectorEvidenceConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(id: &str, atoms: &[&str]) -> EvidenceSet {
        EvidenceSet::new(id, "XAUUSD/5m", 1_800_000_000_000,
            atoms.iter().copied()).unwrap()
    }


    #[test]
    fn v2_builds_evidence_sets_from_the_shared_vector_and_keeps_provenance() {
        let mut vector = MarketVector::new(
            "market-state-17", "binance_usdm", "BTCUSDT", "5m",
            1_800_000_000_000, 1_800_000_000_010,
        ).unwrap();
        vector.set_value(MarketFeature::LogReturn, Some(0.8), 1.0).unwrap();
        vector.set_value(MarketFeature::RegimeTransitionIntensity, Some(0.6), 0.9).unwrap();
        vector.set_value(MarketFeature::OrderFlowImbalance, Some(-0.7), 1.0).unwrap();
        vector.set_not_applicable(MarketFeature::GammaExposure);
        let set = EvidenceSet::from_market_vector(&vector, &VectorEvidenceConfig::default()).unwrap();
        assert!(set.atoms.contains("log_return:positive"));
        assert!(set.atoms.contains("regime_transition_intensity:positive"));
        assert!(set.atoms.contains("order_flow_imbalance:negative"));
        assert!(set.atoms.contains("missing:implied_volatility"));
        assert!(set.atoms.contains("not_applicable:gamma_exposure"));
        assert_eq!(set.source_vector_id.as_deref(), Some("market-state-17"));
        assert_eq!(set.source_vector_schema_version, Some(1));
        assert_eq!(set.source_symbol.as_deref(), Some("BTCUSDT"));
    }

    #[test]
    fn v2_relationship_ledger_record_carries_shared_vector_provenance() {
        let mut left = MarketVector::new(
            "vector-left", "venue-a", "BTCUSD", "5m", 1_800_000_000_000, 1_800_000_000_010,
        ).unwrap();
        let mut right = MarketVector::new(
            "vector-right", "venue-a", "BTCUSD", "5m", 1_800_000_000_100, 1_800_000_000_110,
        ).unwrap();
        left.set_value(MarketFeature::RegimeUncertainty, Some(0.8), 1.0).unwrap();
        right.set_value(MarketFeature::RegimeUncertainty, Some(0.7), 1.0).unwrap();
        let config = VectorEvidenceConfig::default();
        let source = EvidenceSet::from_market_vector(&left, &config).unwrap();
        let target = EvidenceSet::from_market_vector(&right, &config).unwrap();
        let record = RelationshipRecord::discover(&source, &target, "v2-vector-set-1", 0.5, 0.2).unwrap();
        assert_eq!(record.source_vector_id.as_deref(), Some("vector-left"));
        assert_eq!(record.target_vector_id.as_deref(), Some("vector-right"));
        assert_eq!(record.source_symbol.as_deref(), Some("BTCUSD"));
        assert_eq!(record.source_vector_snapshot.as_ref().map(|snapshot| snapshot.vector_id.as_str()), Some("vector-left"));
        assert_eq!(record.target_vector_snapshot.as_ref().map(|snapshot| snapshot.vector_id.as_str()), Some("vector-right"));
        let measure = record.set_measure.as_ref().expect("relationship persists pairwise set measure");
        assert!((measure.union_mass - record.metrics.union_size as f64).abs() < 1e-12);
        assert!((measure.contributions.iter().map(|item| item.allocated_mass).sum::<f64>() - measure.union_mass).abs() < 1e-12);
        assert!(record.validate().is_ok());
        let mut tampered = record.clone();
        tampered.set_measure.as_mut().unwrap().union_mass += 1.0;
        assert_eq!(tampered.validate(), Err(SetAttentionError::InvalidSetMeasure));
    }

    #[test]
    fn set_algebra_and_relation_metrics_are_exact() {
        let left = evidence("trend", &["higher_high", "higher_low", "ema"]);
        let right = evidence("liquidity", &["higher_low", "sweep"]);
        assert_eq!(left.intersection(&right), BTreeSet::from(["higher_low".to_owned()]));
        assert_eq!(left.difference(&right), BTreeSet::from([
            "ema".to_owned(), "higher_high".to_owned()
        ]));
        assert_eq!(left.symmetric_difference(&right).len(), 3);
        let metrics = left.relation_metrics(&right);
        assert_eq!(metrics.intersection_size, 1);
        assert_eq!(metrics.union_size, 4);
        assert!((metrics.jaccard - 0.25).abs() < 1e-12);
        assert_eq!(left.relation_kind(&right), SetRelationKind::Overlap);
    }

    #[test]
    fn vocabulary_and_embeddings_are_permutation_invariant() {
        let a = evidence("a", &["z", "a", "m"]);
        let b = evidence("b", &["m", "z", "a"]);
        assert_eq!(a.atoms, b.atoms);
        let vocabulary = Vocabulary::from_sets(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(vocabulary.embed(&a).unwrap(), vocabulary.embed(&b).unwrap());
    }

    #[test]
    fn attention_is_finite_normalized_and_prefers_the_matching_set() {
        let query = evidence("query", &["trend", "sweep"]);
        let near = evidence("near", &["trend", "sweep", "volume"]);
        let far = evidence("far", &["funding", "basis"]);
        let vocabulary = Vocabulary::from_sets(&[query.clone(), near.clone(), far.clone()]).unwrap();
        let q = vocabulary.embed(&query).unwrap();
        let keys = [vocabulary.embed(&near).unwrap(), vocabulary.embed(&far).unwrap()];
        let weights = set_attention_weights(&q, &keys, 1.0).unwrap();
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(weights[0] > weights[1]);
        assert!(weights.iter().all(|weight| weight.is_finite() && *weight >= 0.0));
        let sharp = set_attention_weights(&q, &keys, 0.1).unwrap();
        let diffuse = set_attention_weights(&q, &keys, 10.0).unwrap();
        assert!(sharp[0] > diffuse[0]);
    }

    #[test]
    fn ledger_rejects_non_finite_metrics_and_missing_outcomes() {
        let source = evidence("source", &["trend"]);
        let target = evidence("target", &["trend", "sweep"]);
        let mut relationship = RelationshipRecord::discover(&source, &target, "set-attention-v2.0.0", 0.7, 1.0).unwrap();
        let mut event = RelationshipLedgerEvent {
            event_id: "evt-1".into(),
            event_type: RelationshipEventType::Discovered,
            recorded_at_ms: 1_800_000_000_001,
            relationship: relationship.clone(),
            outcome: None,
        };
        event.validate().unwrap();
        event.event_type = RelationshipEventType::OutcomeObserved;
        assert_eq!(event.validate(), Err(SetAttentionError::MissingOutcome));
        relationship.metrics.jaccard = f64::NAN;
        event.relationship = relationship;
        event.outcome = Some(RelationshipOutcome {
            evaluated_at_ms: 1_800_000_000_100,
            horizon_ms: 300_000,
            realized_return_bps: Some(2.0),
            max_adverse_excursion_bps: Some(-4.0),
            max_favorable_excursion_bps: Some(6.0),
        });
        assert_eq!(event.validate(), Err(SetAttentionError::InvalidRelationship));
    }

    #[test]
    fn ledger_enforces_append_event_chronology_and_payload_semantics() {
        let source = evidence("source", &["trend"]);
        let target = evidence("target", &["trend", "sweep"]);
        let relationship = RelationshipRecord::discover(
            &source, &target, "set-attention-v2.0.0", 0.7, 1.0,
        ).unwrap();
        let mut event = RelationshipLedgerEvent {
            event_id: "evt-chronology".into(),
            event_type: RelationshipEventType::Discovered,
            recorded_at_ms: relationship.observed_at_ms,
            relationship,
            outcome: None,
        };
        event.validate().unwrap();
        event.recorded_at_ms -= 1;
        assert_eq!(event.validate(), Err(SetAttentionError::InvalidLedgerEvent));
        event.recorded_at_ms += 1;
        event.event_type = RelationshipEventType::OutcomeObserved;
        event.outcome = Some(RelationshipOutcome {
            evaluated_at_ms: event.recorded_at_ms - 1,
            horizon_ms: 60_000,
            realized_return_bps: Some(1.0),
            max_adverse_excursion_bps: Some(-2.0),
            max_favorable_excursion_bps: Some(3.0),
        });
        assert_eq!(event.validate(), Err(SetAttentionError::InvalidOutcome));
    }
}
