//! Version-independent, causal market-state vector contract.
//!
//! Every coordinate is normalized to [-1, 1] using caller-owned, past-only
//! calibration. Missing values remain explicit; they are never imputed as zero.
//! Regime descriptors are ordinary coordinates in this same fixed-dimensional
//! space, not an external categorical switch.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MARKET_VECTOR_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureFamily {
    Price,
    Volatility,
    Trend,
    Liquidity,
    CrossAsset,
    Derivatives,
    Events,
    Regime,
    Portfolio,
    Temporal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureAvailability {
    Observed,
    Missing,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketFeature {
    LogReturn,
    PriceDisplacement,
    RateOfChange,
    GapMagnitude,
    SwingPosition,
    RealizedVolatility,
    ImpliedVolatility,
    VolatilityOfVolatility,
    ReturnSkew,
    ReturnKurtosis,
    JumpIntensity,
    Momentum,
    TrendStrength,
    TrendPersistence,
    MeanReversionStrength,
    TrendExhaustion,
    SpreadBps,
    OrderBookDepth,
    OrderFlowImbalance,
    VolumeDelta,
    MarketImpact,
    SlippageEstimate,
    LiquidityConcentration,
    Correlation,
    Covariance,
    Beta,
    CorrelationBreakdown,
    RelativeStrength,
    FundingPressure,
    OpenInterestChange,
    LiquidationPressure,
    GammaExposure,
    DeltaExposure,
    Basis,
    EventIntensity,
    MacroSurprise,
    EventProximity,
    RegimeTrendStrength,
    RegimeVolatilityLevel,
    RegimeLiquidityStress,
    RegimeCorrelationStress,
    RegimeTransitionIntensity,
    RegimeUncertainty,
    RegimePersistence,
    PortfolioExposure,
    Concentration,
    Drawdown,
    TailRisk,
    RiskContribution,
    TimeSinceStructuralBreak,
    FeaturePersistence,
    SignalDecay,
    TimeframeAgreement,
}

impl MarketFeature {
    /// Stable coordinate order. This order is part of schema version 1.
    pub const ALL: [Self; 53] = [
        Self::LogReturn, Self::PriceDisplacement, Self::RateOfChange,
        Self::GapMagnitude, Self::SwingPosition,
        Self::RealizedVolatility, Self::ImpliedVolatility,
        Self::VolatilityOfVolatility, Self::ReturnSkew, Self::ReturnKurtosis,
        Self::JumpIntensity,
        Self::Momentum, Self::TrendStrength, Self::TrendPersistence,
        Self::MeanReversionStrength, Self::TrendExhaustion,
        Self::SpreadBps, Self::OrderBookDepth, Self::OrderFlowImbalance,
        Self::VolumeDelta, Self::MarketImpact, Self::SlippageEstimate,
        Self::LiquidityConcentration,
        Self::Correlation, Self::Covariance, Self::Beta,
        Self::CorrelationBreakdown, Self::RelativeStrength,
        Self::FundingPressure, Self::OpenInterestChange, Self::LiquidationPressure,
        Self::GammaExposure, Self::DeltaExposure, Self::Basis,
        Self::EventIntensity, Self::MacroSurprise, Self::EventProximity,
        Self::RegimeTrendStrength, Self::RegimeVolatilityLevel,
        Self::RegimeLiquidityStress, Self::RegimeCorrelationStress,
        Self::RegimeTransitionIntensity, Self::RegimeUncertainty,
        Self::RegimePersistence,
        Self::PortfolioExposure, Self::Concentration, Self::Drawdown,
        Self::TailRisk, Self::RiskContribution,
        Self::TimeSinceStructuralBreak, Self::FeaturePersistence,
        Self::SignalDecay, Self::TimeframeAgreement,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LogReturn => "log_return",
            Self::PriceDisplacement => "price_displacement",
            Self::RateOfChange => "rate_of_change",
            Self::GapMagnitude => "gap_magnitude",
            Self::SwingPosition => "swing_position",
            Self::RealizedVolatility => "realized_volatility",
            Self::ImpliedVolatility => "implied_volatility",
            Self::VolatilityOfVolatility => "volatility_of_volatility",
            Self::ReturnSkew => "return_skew",
            Self::ReturnKurtosis => "return_kurtosis",
            Self::JumpIntensity => "jump_intensity",
            Self::Momentum => "momentum",
            Self::TrendStrength => "trend_strength",
            Self::TrendPersistence => "trend_persistence",
            Self::MeanReversionStrength => "mean_reversion_strength",
            Self::TrendExhaustion => "trend_exhaustion",
            Self::SpreadBps => "spread_bps",
            Self::OrderBookDepth => "order_book_depth",
            Self::OrderFlowImbalance => "order_flow_imbalance",
            Self::VolumeDelta => "volume_delta",
            Self::MarketImpact => "market_impact",
            Self::SlippageEstimate => "slippage_estimate",
            Self::LiquidityConcentration => "liquidity_concentration",
            Self::Correlation => "correlation",
            Self::Covariance => "covariance",
            Self::Beta => "beta",
            Self::CorrelationBreakdown => "correlation_breakdown",
            Self::RelativeStrength => "relative_strength",
            Self::FundingPressure => "funding_pressure",
            Self::OpenInterestChange => "open_interest_change",
            Self::LiquidationPressure => "liquidation_pressure",
            Self::GammaExposure => "gamma_exposure",
            Self::DeltaExposure => "delta_exposure",
            Self::Basis => "basis",
            Self::EventIntensity => "event_intensity",
            Self::MacroSurprise => "macro_surprise",
            Self::EventProximity => "event_proximity",
            Self::RegimeTrendStrength => "regime_trend_strength",
            Self::RegimeVolatilityLevel => "regime_volatility_level",
            Self::RegimeLiquidityStress => "regime_liquidity_stress",
            Self::RegimeCorrelationStress => "regime_correlation_stress",
            Self::RegimeTransitionIntensity => "regime_transition_intensity",
            Self::RegimeUncertainty => "regime_uncertainty",
            Self::RegimePersistence => "regime_persistence",
            Self::PortfolioExposure => "portfolio_exposure",
            Self::Concentration => "concentration",
            Self::Drawdown => "drawdown",
            Self::TailRisk => "tail_risk",
            Self::RiskContribution => "risk_contribution",
            Self::TimeSinceStructuralBreak => "time_since_structural_break",
            Self::FeaturePersistence => "feature_persistence",
            Self::SignalDecay => "signal_decay",
            Self::TimeframeAgreement => "timeframe_agreement",
        }
    }

    pub fn family(self) -> FeatureFamily {
        match self {
            Self::LogReturn | Self::PriceDisplacement | Self::RateOfChange
            | Self::GapMagnitude | Self::SwingPosition => FeatureFamily::Price,
            Self::RealizedVolatility | Self::ImpliedVolatility
            | Self::VolatilityOfVolatility | Self::ReturnSkew
            | Self::ReturnKurtosis | Self::JumpIntensity => FeatureFamily::Volatility,
            Self::Momentum | Self::TrendStrength | Self::TrendPersistence
            | Self::MeanReversionStrength | Self::TrendExhaustion => FeatureFamily::Trend,
            Self::SpreadBps | Self::OrderBookDepth | Self::OrderFlowImbalance
            | Self::VolumeDelta | Self::MarketImpact | Self::SlippageEstimate
            | Self::LiquidityConcentration => FeatureFamily::Liquidity,
            Self::Correlation | Self::Covariance | Self::Beta
            | Self::CorrelationBreakdown | Self::RelativeStrength => FeatureFamily::CrossAsset,
            Self::FundingPressure | Self::OpenInterestChange | Self::LiquidationPressure
            | Self::GammaExposure | Self::DeltaExposure | Self::Basis => FeatureFamily::Derivatives,
            Self::EventIntensity | Self::MacroSurprise | Self::EventProximity => FeatureFamily::Events,
            Self::RegimeTrendStrength | Self::RegimeVolatilityLevel
            | Self::RegimeLiquidityStress | Self::RegimeCorrelationStress
            | Self::RegimeTransitionIntensity | Self::RegimeUncertainty
            | Self::RegimePersistence => FeatureFamily::Regime,
            Self::PortfolioExposure | Self::Concentration | Self::Drawdown
            | Self::TailRisk | Self::RiskContribution => FeatureFamily::Portfolio,
            Self::TimeSinceStructuralBreak | Self::FeaturePersistence
            | Self::SignalDecay | Self::TimeframeAgreement => FeatureFamily::Temporal,
        }
    }

    pub fn is_regime(self) -> bool {
        self.family() == FeatureFamily::Regime
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FeatureCoordinate {
    /// Distinguishes an unavailable applicable input from a dimension that does not apply.
    pub status: FeatureAvailability,
    /// Past-only normalized value in [-1, 1]. None means unavailable, not zero.
    pub value: Option<f64>,
    /// Data/reliability quality in (0, 1] when value exists; zero when missing.
    pub quality: f64,
}

impl FeatureCoordinate {
    pub fn missing() -> Self {
        Self { status: FeatureAvailability::Missing, value: None, quality: 0.0 }
    }

    pub fn observed(value: f64, quality: f64) -> Result<Self, MarketSpaceError> {
        let coordinate = Self { status: FeatureAvailability::Observed, value: Some(value), quality };
        coordinate.validate()?;
        Ok(coordinate)
    }

    fn validate(&self) -> Result<(), MarketSpaceError> {
        if !self.quality.is_finite() || !(0.0..=1.0).contains(&self.quality) {
            return Err(MarketSpaceError::InvalidCoordinate);
        }
        match (self.status, self.value) {
            (FeatureAvailability::Observed, Some(value))
                if value.is_finite() && (-1.0..=1.0).contains(&value) && self.quality > 0.0 => Ok(()),
            (FeatureAvailability::Missing | FeatureAvailability::NotApplicable, None)
                if self.quality == 0.0 => Ok(()),
            _ => Err(MarketSpaceError::InvalidCoordinate),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketVector {
    pub vector_id: String,
    pub schema_version: u16,
    pub venue: String,
    pub symbol: String,
    pub timeframe: String,
    /// Latest event-time boundary included in this snapshot.
    pub event_time_cutoff_ms: i64,
    /// Wall-clock time at which this snapshot became available to the algorithm.
    pub available_at_ms: i64,
    /// All 53 schema coordinates are present; unavailable values are explicit.
    pub coordinates: BTreeMap<MarketFeature, FeatureCoordinate>,
}

impl MarketVector {
    pub fn new(
        vector_id: impl Into<String>,
        venue: impl Into<String>,
        symbol: impl Into<String>,
        timeframe: impl Into<String>,
        event_time_cutoff_ms: i64,
        available_at_ms: i64,
    ) -> Result<Self, MarketSpaceError> {
        let coordinates = MarketFeature::ALL.into_iter()
            .map(|feature| (feature, FeatureCoordinate::missing()))
            .collect();
        let vector = Self {
            vector_id: vector_id.into(),
            schema_version: MARKET_VECTOR_SCHEMA_VERSION,
            venue: venue.into(),
            symbol: symbol.into(),
            timeframe: timeframe.into(),
            event_time_cutoff_ms,
            available_at_ms,
            coordinates,
        };
        vector.validate_metadata()?;
        Ok(vector)
    }

    pub fn set_value(
        &mut self,
        feature: MarketFeature,
        value: Option<f64>,
        quality: f64,
    ) -> Result<(), MarketSpaceError> {
        let status = if value.is_some() { FeatureAvailability::Observed } else { FeatureAvailability::Missing };
        let coordinate = FeatureCoordinate { status, value, quality };
        coordinate.validate()?;
        self.coordinates.insert(feature, coordinate);
        Ok(())
    }

    pub fn set_not_applicable(&mut self, feature: MarketFeature) {
        self.coordinates.insert(feature, FeatureCoordinate {
            status: FeatureAvailability::NotApplicable,
            value: None,
            quality: 0.0,
        });
    }

    pub fn value(&self, feature: MarketFeature) -> Option<f64> {
        self.coordinates.get(&feature).and_then(|coordinate| coordinate.value)
    }

    pub fn require(&self, feature: MarketFeature) -> Result<f64, MarketSpaceError> {
        self.value(feature).ok_or(MarketSpaceError::MissingFeature(feature))
    }

    pub fn available_dimensions(&self) -> usize {
        self.coordinates.values().filter(|coordinate| coordinate.value.is_some()).count()
    }

    pub fn validate(&self) -> Result<(), MarketSpaceError> {
        self.validate_metadata()?;
        if self.coordinates.len() != MarketFeature::ALL.len()
            || MarketFeature::ALL.iter().any(|feature| !self.coordinates.contains_key(feature))
        {
            return Err(MarketSpaceError::IncompleteVector);
        }
        for coordinate in self.coordinates.values() {
            coordinate.validate()?;
        }
        if self.available_dimensions() == 0 {
            return Err(MarketSpaceError::EmptyVector);
        }
        Ok(())
    }

    fn validate_metadata(&self) -> Result<(), MarketSpaceError> {
        validate_identifier(&self.vector_id, 96)?;
        validate_identifier(&self.venue, 64)?;
        validate_identifier(&self.symbol, 96)?;
        validate_identifier(&self.timeframe, 32)?;
        if self.schema_version != MARKET_VECTOR_SCHEMA_VERSION {
            return Err(MarketSpaceError::UnsupportedSchemaVersion);
        }
        if self.event_time_cutoff_ms <= 0 || self.available_at_ms <= 0 {
            return Err(MarketSpaceError::InvalidTimestamp);
        }
        if self.available_at_ms < self.event_time_cutoff_ms {
            return Err(MarketSpaceError::AvailabilityPrecedesEventTime);
        }
        Ok(())
    }

    /// Compare two normalized states over dimensions observed in both snapshots.
    /// Reliability is weighted by the lower quality of the two coordinates.
    pub fn compare(&self, other: &Self) -> Result<VectorComparison, MarketSpaceError> {
        self.validate()?;
        other.validate()?;
        let mut dot = 0.0;
        let mut left_norm_sq = 0.0;
        let mut right_norm_sq = 0.0;
        let mut squared_distance = 0.0;
        let mut total_weight = 0.0;
        let mut common_dimensions = 0usize;

        for feature in MarketFeature::ALL {
            let left = self.coordinates.get(&feature).expect("validated fixed schema");
            let right = other.coordinates.get(&feature).expect("validated fixed schema");
            if left.status == FeatureAvailability::Observed
                && right.status == FeatureAvailability::Observed
            {
                if let (Some(a), Some(b)) = (left.value, right.value) {
                    let weight = left.quality.min(right.quality);
                    if weight <= 0.0 { continue; }
                    common_dimensions += 1;
                    total_weight += weight;
                    dot += weight * a * b;
                    left_norm_sq += weight * a * a;
                    right_norm_sq += weight * b * b;
                    squared_distance += weight * (a - b).powi(2);
                }
            }
        }

        if common_dimensions == 0 || total_weight <= f64::EPSILON {
            return Err(MarketSpaceError::NoComparableDimensions);
        }
        if left_norm_sq <= f64::EPSILON || right_norm_sq <= f64::EPSILON {
            return Err(MarketSpaceError::ZeroNorm);
        }
        Ok(VectorComparison {
            cosine_similarity: (dot / (left_norm_sq.sqrt() * right_norm_sq.sqrt())).clamp(-1.0, 1.0),
            weighted_euclidean_distance: (squared_distance / total_weight).sqrt(),
            common_dimensions,
            coverage: common_dimensions as f64 / MarketFeature::ALL.len() as f64,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VectorComparison {
    pub cosine_similarity: f64,
    pub weighted_euclidean_distance: f64,
    pub common_dimensions: usize,
    pub coverage: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MarketSpaceError {
    #[error("invalid market vector identifier")]
    InvalidIdentifier,
    #[error("market vector timestamps must be positive")]
    InvalidTimestamp,
    #[error("vector availability cannot precede its event-time cutoff")]
    AvailabilityPrecedesEventTime,
    #[error("unsupported market vector schema version")]
    UnsupportedSchemaVersion,
    #[error("market vector must contain every schema coordinate")]
    IncompleteVector,
    #[error("market vector must contain at least one observed coordinate")]
    EmptyVector,
    #[error("feature coordinate must be finite, normalized to [-1, 1], and have valid quality")]
    InvalidCoordinate,
    #[error("required market feature {0:?} is unavailable")]
    MissingFeature(MarketFeature),
    #[error("vectors have no comparable observed dimensions")]
    NoComparableDimensions,
    #[error("cosine similarity is undefined for a zero-norm vector")]
    ZeroNorm,
}

fn validate_identifier(value: &str, max_len: usize) -> Result<(), MarketSpaceError> {
    if value.trim().is_empty() || value.len() > max_len || value.chars().any(char::is_control) {
        return Err(MarketSpaceError::InvalidIdentifier);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(id: &str) -> MarketVector {
        let mut vector = MarketVector::new(
            id, "binance_usdm", "BTCUSDT", "5m", 1_800_000_000_000, 1_800_000_000_010,
        ).unwrap();
        vector.set_value(MarketFeature::LogReturn, Some(0.4), 1.0).unwrap();
        vector.set_value(MarketFeature::RegimeTransitionIntensity, Some(0.7), 0.9).unwrap();
        vector
    }

    #[test]
    fn schema_is_fixed_and_regime_is_inside_the_same_vector() {
        let vector = vector("v-1");
        assert_eq!(MarketFeature::ALL.len(), 53);
        assert_eq!(vector.coordinates.len(), 53);
        assert!(vector.coordinates.keys().any(|feature| feature.is_regime()));
        assert_eq!(vector.available_dimensions(), 2);
        assert!(vector.validate().is_ok());
    }

    #[test]
    fn missing_is_not_silently_imputed_as_zero() {
        let vector = vector("v-1");
        assert_eq!(vector.value(MarketFeature::ImpliedVolatility), None);
        assert_eq!(vector.coordinates[&MarketFeature::ImpliedVolatility].status, FeatureAvailability::Missing);
        assert_eq!(vector.require(MarketFeature::ImpliedVolatility), Err(MarketSpaceError::MissingFeature(MarketFeature::ImpliedVolatility)));
    }

    #[test]
    fn comparison_uses_shared_dimensions_and_reports_coverage() {
        let left = vector("left");
        let mut right = vector("right");
        right.set_value(MarketFeature::LogReturn, Some(0.2), 0.8).unwrap();
        right.set_value(MarketFeature::RegimeTransitionIntensity, Some(0.7), 1.0).unwrap();
        let comparison = left.compare(&right).unwrap();
        assert_eq!(comparison.common_dimensions, 2);
        assert!((comparison.coverage - 2.0 / 53.0).abs() < 1e-12);
        assert!(comparison.cosine_similarity > 0.95);
        assert!(comparison.weighted_euclidean_distance > 0.0);
    }


    #[test]
    fn not_applicable_is_distinct_from_missing() {
        let mut vector = vector("availability-state");
        vector.set_not_applicable(MarketFeature::GammaExposure);
        assert_eq!(vector.coordinates[&MarketFeature::GammaExposure].status, FeatureAvailability::NotApplicable);
        assert_eq!(vector.coordinates[&MarketFeature::ImpliedVolatility].status, FeatureAvailability::Missing);
        assert!(vector.validate().is_ok());
    }

    #[test]
    fn rejects_lookahead_and_out_of_range_coordinates() {
        assert_eq!(
            MarketVector::new("bad", "venue", "symbol", "1m", 100, 99).unwrap_err(),
            MarketSpaceError::AvailabilityPrecedesEventTime,
        );
        let mut vector = vector("bad-value");
        assert_eq!(
            vector.set_value(MarketFeature::Momentum, Some(1.5), 1.0),
            Err(MarketSpaceError::InvalidCoordinate),
        );
    }

    #[test]
    fn identical_vectors_have_unit_similarity_and_zero_distance() {
        let left = vector("left");
        let right = vector("right");
        let comparison = left.compare(&right).unwrap();
        assert!((comparison.cosine_similarity - 1.0).abs() < 1e-12);
        assert!(comparison.weighted_euclidean_distance.abs() < 1e-12);
    }
}
