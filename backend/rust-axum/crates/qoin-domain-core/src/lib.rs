//! Domain-neutral contracts shared by Qoin applications.
//!
//! Financial instruments and market events belong in domain adapters. This crate
//! defines the common identity, observation, feature-vector and decision envelope.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DomainId(String);

impl DomainId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainCoreError> {
        let value = value.into();
        validate_identifier("domain", &value)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    pub domain: DomainId,
    pub entity_type: String,
    pub entity_id: String,
}

impl EntityRef {
    pub fn validate(&self) -> Result<(), DomainCoreError> {
        validate_identifier("entity_type", &self.entity_type)?;
        validate_identifier("entity_id", &self.entity_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSource {
    pub provider: String,
    pub stream: String,
}

impl DataSource {
    pub fn validate(&self) -> Result<(), DomainCoreError> {
        validate_identifier("provider", &self.provider)?;
        validate_identifier("stream", &self.stream)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: String,
    pub entity: EntityRef,
    pub source: DataSource,
    pub observed_at_ms: i64,
    pub kind: String,
    pub measurements: BTreeMap<String, f64>,
    pub quality: f64,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl Observation {
    pub fn validate(&self) -> Result<(), DomainCoreError> {
        validate_identifier("observation_id", &self.observation_id)?;
        self.entity.validate()?;
        self.source.validate()?;
        validate_identifier("kind", &self.kind)?;
        if self.observed_at_ms <= 0 { return Err(DomainCoreError::InvalidTimestamp); }
        validate_unit_interval("quality", self.quality)?;
        validate_measurements(&self.measurements)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub entity: EntityRef,
    pub as_of_ms: i64,
    pub model_id: String,
    pub model_version: String,
    pub features: BTreeMap<String, f64>,
}

impl FeatureVector {
    pub fn validate(&self) -> Result<(), DomainCoreError> {
        self.entity.validate()?;
        if self.as_of_ms <= 0 { return Err(DomainCoreError::InvalidTimestamp); }
        validate_identifier("model_id", &self.model_id)?;
        validate_identifier("model_version", &self.model_version)?;
        if self.features.is_empty() { return Err(DomainCoreError::EmptyMeasurements); }
        validate_measurements(&self.features)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub entity: EntityRef,
    pub decided_at_ms: i64,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub action: String,
    pub confidence: f64,
    pub reason_codes: Vec<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl Decision {
    pub fn validate(&self) -> Result<(), DomainCoreError> {
        self.entity.validate()?;
        if self.decided_at_ms <= 0 { return Err(DomainCoreError::InvalidTimestamp); }
        validate_identifier("algorithm_id", &self.algorithm_id)?;
        validate_identifier("algorithm_version", &self.algorithm_version)?;
        validate_identifier("action", &self.action)?;
        validate_unit_interval("confidence", self.confidence)?;
        for reason in &self.reason_codes { validate_identifier("reason_code", reason)?; }
        for evidence in &self.evidence_ids { validate_identifier("evidence_id", evidence)?; }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum DomainCoreError {
    #[error("{0} must be non-empty and at most 128 characters")]
    InvalidIdentifier(&'static str),
    #[error("observation timestamp must be a positive Unix timestamp in milliseconds")]
    InvalidTimestamp,
    #[error("{0} must be finite and between 0 and 1")]
    InvalidUnitInterval(&'static str),
    #[error("measurement names must be valid and values must be finite")]
    InvalidMeasurement,
    #[error("at least one measurement is required")]
    EmptyMeasurements,
}

fn validate_identifier(name: &'static str, value: &str) -> Result<(), DomainCoreError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(DomainCoreError::InvalidIdentifier(name));
    }
    Ok(())
}

fn validate_unit_interval(name: &'static str, value: f64) -> Result<(), DomainCoreError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(DomainCoreError::InvalidUnitInterval(name));
    }
    Ok(())
}

fn validate_measurements(values: &BTreeMap<String, f64>) -> Result<(), DomainCoreError> {
    if values.is_empty() { return Err(DomainCoreError::EmptyMeasurements); }
    if values.iter().any(|(name, value)| name.trim().is_empty() || name.len() > 128 || !value.is_finite()) {
        return Err(DomainCoreError::InvalidMeasurement);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> EntityRef {
        EntityRef {
            domain: DomainId::new("agriculture").unwrap(),
            entity_type: "field".into(),
            entity_id: "field-17".into(),
        }
    }

    #[test]
    fn validates_domain_neutral_observation() {
        let observation = Observation {
            observation_id: "sensor-17-001".into(),
            entity: entity(),
            source: DataSource { provider: "weather-station".into(), stream: "soil-moisture".into() },
            observed_at_ms: 1_800_000_000_000,
            kind: "sensor_reading".into(),
            measurements: [("volumetric_water_content".into(), 0.24)].into_iter().collect(),
            quality: 0.98,
            attributes: BTreeMap::new(),
        };
        assert!(observation.validate().is_ok());
    }

    #[test]
    fn rejects_non_finite_measurements_and_invalid_quality() {
        let observation = Observation {
            observation_id: "obs-1".into(),
            entity: entity(),
            source: DataSource { provider: "provider".into(), stream: "stream".into() },
            observed_at_ms: 1_800_000_000_000,
            kind: "reading".into(),
            measurements: [("temperature".into(), f64::NAN)].into_iter().collect(),
            quality: 0.5,
            attributes: BTreeMap::new(),
        };
        assert_eq!(observation.validate(), Err(DomainCoreError::InvalidMeasurement));
    }

    #[test]
    fn validates_algorithm_decisions_without_finance_specific_fields() {
        let decision = Decision {
            entity: entity(),
            decided_at_ms: 1_800_000_000_000,
            algorithm_id: "irrigation-planner".into(),
            algorithm_version: "1.0.0".into(),
            action: "irrigate".into(),
            confidence: 0.87,
            reason_codes: vec!["soil_moisture_below_target".into()],
            evidence_ids: vec!["sensor-17-001".into()],
            attributes: BTreeMap::new(),
        };
        assert!(decision.validate().is_ok());
    }

    #[test]
    fn rejects_empty_feature_vectors() {
        let vector = FeatureVector {
            entity: entity(),
            as_of_ms: 1_800_000_000_000,
            model_id: "field-state".into(),
            model_version: "1".into(),
            features: BTreeMap::new(),
        };
        assert_eq!(vector.validate(), Err(DomainCoreError::EmptyMeasurements));
    }
}
