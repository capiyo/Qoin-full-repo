//! Static, validated catalogue for independently versioned Qoin research algorithms.
//!
//! This module deliberately contains no payment or user-authorization logic. Paid
//! entitlements must be resolved by the authenticated API boundary before a
//! protected algorithm can be selected or executed.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
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
    pub supported_market_data_providers: &'static [&'static str],
    pub supported_venues: &'static [&'static str],
    pub supported_instrument_groups: &'static [&'static str],
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
        supported_market_data_providers: &["binance_public"],
        supported_venues: &["binance_usdm"],
        supported_instrument_groups: &["crypto_perpetuals"],
        supported_timeframes: &["1m", "5m", "15m", "30m", "1h", "4h", "1d"],
        required_features: &["order_flow_imbalance", "liquidity_stress", "event_intensity"],
        optional_features: &["funding_pressure", "volatility_shock", "liquidation_pressure"],
        risk_profile: "research_baseline; abstains when spread or event evidence is insufficient",
        execution_mode: "research_only",
        evaluation_status: "not_independently_validated",
    }]
}


/// Commercial access packages describe market-data coverage, not strategy quality.
/// These are preview definitions only: no prices, payments or authorization are active.
#[derive(Debug, Clone, Serialize)]
pub struct MarketAccessTier {
    pub tier_id: &'static str,
    pub display_name: &'static str,
    pub status: &'static str,
    pub market_data_providers: &'static [&'static str],
    pub venues: &'static [&'static str],
    pub asset_classes: &'static [&'static str],
    pub instrument_groups: &'static [&'static str],
    pub max_active_instruments: u16,
    pub historical_depth_days: u16,
    pub supported_timeframes: &'static [&'static str],
    pub max_stream_subscriptions: u16,
    pub execution_brokers: &'static [&'static str],
    pub notes: &'static str,
}
pub fn market_access_tiers() -> Vec<MarketAccessTier> {
    vec![
        MarketAccessTier { tier_id:"qoin_starter", display_name:"Starter", status:"preview",
            market_data_providers:&["binance_public"], venues:&["binance_usdm"],
            asset_classes:&["crypto"], instrument_groups:&["crypto_major_perpetuals"],
            max_active_instruments:5, historical_depth_days:7,
            supported_timeframes:&["5m","15m","1h","4h","1d"], max_stream_subscriptions:5,
            execution_brokers:&[], notes:"Narrow crypto universe; no commodities or broker execution." },
        MarketAccessTier { tier_id:"qoin_plus", display_name:"Plus", status:"preview",
            market_data_providers:&["binance_public","finnhub_optional"], venues:&["binance_usdm","finnhub_covered_markets"],
            asset_classes:&["crypto","equities","forex"],
            instrument_groups:&["crypto_perpetuals","large_cap_equities","major_fx"],
            max_active_instruments:25, historical_depth_days:90,
            supported_timeframes:&["1m","5m","15m","30m","1h","4h","1d"], max_stream_subscriptions:15,
            execution_brokers:&[], notes:"Expanded provider and instrument coverage; Finnhub requires configured credentials and provider entitlement." },
        MarketAccessTier { tier_id:"qoin_pro", display_name:"Pro", status:"preview",
            market_data_providers:&["binance_public","finnhub_optional"], venues:&["binance_usdm","finnhub_covered_markets"],
            asset_classes:&["crypto","equities","forex","indices","commodities"],
            instrument_groups:&["crypto_perpetuals","equities","major_fx","indices","energy","metals","agriculture"],
            max_active_instruments:100, historical_depth_days:365,
            supported_timeframes:&["1m","5m","15m","30m","1h","4h","1d"], max_stream_subscriptions:50,
            execution_brokers:&[], notes:"Broadest planned research universe; actual instruments depend on live provider coverage and licensing. No broker execution is currently enabled." }
    ]
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
        "catalogue_version": 2,
        "market_access_tiers": market_access_tiers(),
        "market_access_tiers_enabled": false,
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
            assert!(!algorithm.supported_market_data_providers.is_empty());
            assert!(!algorithm.supported_venues.is_empty());
            assert!(!algorithm.supported_timeframes.is_empty());
            assert!(!algorithm.required_features.is_empty());
            assert_eq!(algorithm.evaluation_status, "not_independently_validated");
        }
    }
    #[test]
    fn access_tiers_expand_coverage_without_claiming_broker_execution() {
        let tiers = market_access_tiers();
        assert_eq!(tiers.len(), 3);
        assert!(!tiers[0].asset_classes.contains(&"commodities"));
        assert!(tiers[2].asset_classes.contains(&"commodities"));
        assert!(tiers[0].historical_depth_days < tiers[1].historical_depth_days);
        assert!(tiers[1].historical_depth_days < tiers[2].historical_depth_days);
        assert!(tiers.iter().all(|tier| tier.execution_brokers.is_empty()));
        assert!(tiers.iter().all(|tier| tier.status == "preview"));
    }

}
