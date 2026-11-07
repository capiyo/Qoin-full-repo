//! Static, validated catalogue for independently versioned Qoin research algorithms.
//!
//! This module deliberately contains no payment or user-authorization logic. Paid
//! entitlements must be resolved by the authenticated API boundary before a
//! protected algorithm can be selected or executed.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmStatus {
    Active,
    Preview,
    Deprecated,
    Disabled,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlgorithmManifest {
    pub algorithm_id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub status: AlgorithmStatus,
    pub supported_asset_classes: &'static [&'static str],
    pub supported_timeframes: &'static [&'static str],
    pub required_features: &'static [&'static str],
    pub optional_features: &'static [&'static str],
    pub risk_profile: &'static str,
    pub execution_mode: &'static str,
    pub evaluation_status: &'static str,
}

impl AlgorithmManifest {
    fn validate(&self) -> Result<(), &'static str> {
        if self.algorithm_id.trim().is_empty() || self.display_name.trim().is_empty()
            || self.version.trim().is_empty() {
            return Err("algorithm identity, display name, and version are required");
        }
        if self.supported_asset_classes.is_empty() || self.supported_timeframes.is_empty() {
            return Err("an algorithm must declare supported asset classes and timeframes");
        }
        if self.execution_mode != "research_only" {
            return Err("this catalogue only registers research-only algorithms");
        }
        if !matches!(self.status, AlgorithmStatus::Active | AlgorithmStatus::Preview
            | AlgorithmStatus::Deprecated | AlgorithmStatus::Disabled) {
            return Err("unknown algorithm lifecycle status");
        }
        Ok(())
    }
}

/// The existing deterministic vector decision path is registered first without
/// changing its signal math or implying that it has a validated trading edge.
pub fn registered_algorithms() -> Vec<AlgorithmManifest> {
    vec![AlgorithmManifest {
        algorithm_id: "qoin-vector-baseline",
        display_name: "Qoin Vector Baseline",
        description: "Deterministic multi-force vector research using order flow, liquidity, volatility, funding, positioning and liquidation context.",
        version: "edge-map-v1",
        status: AlgorithmStatus::Active,
        supported_asset_classes: &["crypto"],
        supported_timeframes: &["1m", "5m", "15m", "30m", "1h", "4h", "1d"],
        required_features: &["order_flow_imbalance", "liquidity_stress", "event_intensity"],
        optional_features: &["funding_pressure", "volatility_shock", "liquidation_pressure"],
        risk_profile: "research_baseline; abstains when spread or event evidence is insufficient",
        execution_mode: "research_only",
        evaluation_status: "not_independently_validated",
    }]
}

pub fn validate_catalogue() -> Result<(), &'static str> {
    let algorithms = registered_algorithms();
    for algorithm in &algorithms {
        algorithm.validate()?;
    }
    for (index, algorithm) in algorithms.iter().enumerate() {
        if algorithms[index + 1..].iter().any(|other| {
            other.algorithm_id == algorithm.algorithm_id && other.version == algorithm.version
        }) {
            return Err("duplicate algorithm ID and version");
        }
    }
    Ok(())
}

pub async fn get_algorithms() -> impl IntoResponse {
    if let Err(message) = validate_catalogue() {
        tracing::error!(reason = message, "algorithm catalogue validation failed");
        return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"algorithm catalogue unavailable"})));
    }
    let algorithms = registered_algorithms();
    (StatusCode::OK, Json(serde_json::json!({
        "catalogue_version": 1,
        "algorithms": algorithms,
        "selection_enabled": false,
        "entitlements_enabled": false,
        "note": "Catalogue metadata only. User-specific access and selection require authenticated server-side entitlements."
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_is_registered_and_valid() {
        assert!(validate_catalogue().is_ok());
        let algorithms = registered_algorithms();
        assert_eq!(algorithms.len(), 1);
        assert_eq!(algorithms[0].algorithm_id, "qoin-vector-baseline");
        assert_eq!(algorithms[0].execution_mode, "research_only");
    }

    #[test]
    fn every_registered_algorithm_has_explicit_capabilities() {
        for algorithm in registered_algorithms() {
            assert!(!algorithm.supported_asset_classes.is_empty());
            assert!(!algorithm.supported_timeframes.is_empty());
            assert!(!algorithm.required_features.is_empty());
            assert_eq!(algorithm.evaluation_status, "not_independently_validated");
        }
    }
}
