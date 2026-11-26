use qoin_market_space::{MarketFeature, MarketVector};
use qoin_vector_engine::Signal;
use sha2::{Digest, Sha256};
use super::{MarketEvent, SymbolState};

/// Persist a sparse but honest 53-coordinate snapshot from the authoritative V1
/// engine output. Only features with source evidence are marked Observed;
/// unsupported portfolio/Greeks dimensions and spot-only derivatives dimensions
/// remain explicitly NotApplicable rather than being filled with neutral zeros.
pub(super) fn build_market_vector(
    event: &MarketEvent,
    state: &SymbolState,
    signal: &Signal,
    available_at_ms: i64,
) -> Option<MarketVector> {
    let cutoff = state.last_event_ts_ms?;
    if cutoff <= 0 || available_at_ms < cutoff { return None; }
    let identity = format!("{}|{}|{}|event-window|{}", event.venue, event.symbol, event.event_id, cutoff);
    let digest = hex::encode(Sha256::digest(identity.as_bytes()));
    let vector_id = format!("mv-{}", &digest[..32]);
    let mut vector = MarketVector::new(
        vector_id, &event.venue, &event.symbol, "event-window", cutoff, available_at_ms
    ).ok()?;

    for feature in [
        MarketFeature::ImpliedVolatility,
        MarketFeature::GammaExposure,
        MarketFeature::DeltaExposure,
        MarketFeature::PortfolioExposure,
        MarketFeature::Concentration,
        MarketFeature::Drawdown,
        MarketFeature::TailRisk,
        MarketFeature::RiskContribution,
    ] {
        vector.set_not_applicable(feature);
    }
    if event.venue != "binance_usdm" {
        for feature in [
            MarketFeature::FundingPressure,
            MarketFeature::OpenInterestChange,
            MarketFeature::LiquidationPressure,
            MarketFeature::Basis,
        ] {
            vector.set_not_applicable(feature);
        }
    }

    let mut observed = 0usize;
    let mut set = |feature: MarketFeature, name: &str| {
        if let Some(value) = signal.features.get(name).copied().filter(|v| v.is_finite()) {
            if vector.set_value(feature, Some(value.clamp(-1.0, 1.0)), 1.0).is_ok() {
                observed += 1;
            }
        }
    };
    if state.last_trade_ts_ms.is_some() {
        set(MarketFeature::OrderFlowImbalance, "order_flow_imbalance");
        set(MarketFeature::RealizedVolatility, "volatility");
    }
    if state.funding_rate.is_some() && event.venue == "binance_usdm" {
        set(MarketFeature::FundingPressure, "funding_pressure");
    }
    if state.open_interest.is_some() && event.venue == "binance_usdm" {
        set(MarketFeature::OpenInterestChange, "open_interest_change");
    }
    if state.liquidation_events > 0 && event.venue == "binance_usdm" {
        set(MarketFeature::LiquidationPressure, "liquidation_pressure");
    }
    if state.basis_bps.is_some() && event.venue == "binance_usdm" {
        set(MarketFeature::Basis, "basis");
    }
    if observed == 0 || vector.validate().is_err() { return None; }
    Some(vector)
}
