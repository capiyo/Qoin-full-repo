use qoin_market_space::{MarketFeature, MarketVector};
use qoin_vector_engine::Signal;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use super::{MarketEvent, SymbolState};

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() { return None; }
    let value = values.iter().sum::<f64>() / values.len() as f64;
    value.is_finite().then_some(value)
}

fn quote_currency(symbol: &str, venue: &str) -> Option<String> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if let Some((_, quote)) = symbol.rsplit_once('/') {
        return (!quote.is_empty()).then(|| quote.to_string());
    }
    if let Some((_, quote)) = symbol.rsplit_once('_') {
        return (!quote.is_empty()).then(|| quote.to_string());
    }
    for suffix in ["USDT", "USDC", "BUSD", "USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "BTC", "ETH"] {
        if symbol.len() > suffix.len() && symbol.ends_with(suffix) {
            return Some(suffix.to_string());
        }
    }
    if venue == "finnhub" && symbol.chars().all(|value| value.is_ascii_alphanumeric()) {
        return Some("USD".into());
    }
    None
}

fn minute_return_buckets(state: &SymbolState, cutoff_ms: i64) -> BTreeMap<i64, f64> {
    let mut buckets = BTreeMap::new();
    let current_bucket = cutoff_ms.div_euclid(60_000);
    for (timestamp, log_return) in &state.recent_return_observations {
        let bucket = timestamp.div_euclid(60_000);
        // Only completed minute buckets are eligible. This prevents partial-bar,
        // same-minute ordering, and future-peer leakage.
        if bucket < current_bucket && *timestamp <= cutoff_ms
            && cutoff_ms.saturating_sub(*timestamp) <= 24 * 60 * 60 * 1_000
            && log_return.is_finite()
        {
            *buckets.entry(bucket).or_insert(0.0) += *log_return;
        }
    }
    buckets
}

fn pearson_correlation(left: &[f64], right: &[f64]) -> Option<(f64, f64, f64)> {
    if left.len() != right.len() || left.len() < 2 { return None; }
    let left_mean = mean(left)?;
    let right_mean = mean(right)?;
    let covariance = left.iter().zip(right)
        .map(|(a, b)| (a - left_mean) * (b - right_mean))
        .sum::<f64>() / left.len() as f64;
    let left_variance = left.iter().map(|value| (value - left_mean).powi(2))
        .sum::<f64>() / left.len() as f64;
    let right_variance = right.iter().map(|value| (value - right_mean).powi(2))
        .sum::<f64>() / right.len() as f64;
    if left_variance <= 1.0e-18 || right_variance <= 1.0e-18 { return None; }
    let correlation = (covariance / (left_variance * right_variance).sqrt()).clamp(-1.0, 1.0);
    let beta = covariance / right_variance;
    Some((correlation, covariance, beta))
}

/// Cross-asset features use same-venue, same-quote, event-time-aligned completed
/// one-minute return buckets. Peers newer than the current event are never read.
fn cross_asset_raw_features(
    state: &SymbolState,
    peers: &[SymbolState],
    cutoff_ms: i64,
) -> Option<(BTreeMap<MarketFeature, f64>, f64)> {
    let current_last_ts = state.last_price_ts_ms?;
    if current_last_ts > cutoff_ms || cutoff_ms.saturating_sub(current_last_ts) > 5 * 60_000 {
        return None;
    }
    let quote = quote_currency(&state.symbol, &state.venue)?;
    let current = minute_return_buckets(state, cutoff_ms);
    let mut correlations = Vec::new();
    let mut covariances = Vec::new();
    let mut betas = Vec::new();
    let mut breakdowns = Vec::new();
    let mut relative_strengths = Vec::new();
    let mut common_counts = Vec::new();
    let mut peer_quality = Vec::new();

    for peer in peers {
        if peer.venue != state.venue || peer.symbol == state.symbol
            || quote_currency(&peer.symbol, &peer.venue).as_deref() != Some(quote.as_str()) {
            continue;
        }
        let Some(peer_last_ts) = peer.last_price_ts_ms else { continue; };
        if peer_last_ts > cutoff_ms || cutoff_ms.saturating_sub(peer_last_ts) > 5 * 60_000 {
            continue;
        }
        let other = minute_return_buckets(peer, cutoff_ms);
        let common = current.iter().filter_map(|(bucket, value)| {
            other.get(bucket).map(|peer_value| (*value, *peer_value))
        }).collect::<Vec<_>>();
        if common.len() < 12 { continue; }
        let left = common.iter().map(|pair| pair.0).collect::<Vec<_>>();
        let right = common.iter().map(|pair| pair.1).collect::<Vec<_>>();
        let Some((correlation, covariance, beta)) = pearson_correlation(&left, &right) else { continue; };
        let split = left.len() / 2;
        if split < 6 || left.len() - split < 6 { continue; }
        let Some((early, late)) = pearson_correlation(&left[..split], &right[..split])
            .map(|value| value.0)
            .zip(pearson_correlation(&left[split..], &right[split..]).map(|value| value.0)) else {
            continue;
        };
        correlations.push(correlation);
        covariances.push(covariance);
        betas.push(beta);
        breakdowns.push((late - early).abs().clamp(0.0, 2.0) / 2.0);
        relative_strengths.push(mean(&left).unwrap_or(0.0) - mean(&right).unwrap_or(0.0));
        common_counts.push(common.len());
        peer_quality.push(state.state_quality.min(peer.state_quality));
    }
    if correlations.is_empty() { return None; }

    let freshness_ms = cutoff_ms.saturating_sub(current_last_ts)
        .max(cutoff_ms.saturating_sub(peers.iter().filter_map(|peer| peer.last_price_ts_ms)
            .filter(|timestamp| *timestamp <= cutoff_ms).max().unwrap_or(current_last_ts)));
    let freshness = (-(freshness_ms as f64) / 300_000.0).exp();
    let history_quality = (common_counts.iter().copied().min().unwrap_or(0) as f64 / 32.0).min(1.0);
    let quality = (history_quality * freshness * mean(&peer_quality).unwrap_or(0.0)).clamp(0.0, 1.0);
    if quality <= 0.0 { return None; }

    let mut output = BTreeMap::new();
    output.insert(MarketFeature::Correlation, mean(&correlations)?);
    output.insert(MarketFeature::Covariance, mean(&covariances)?.mul_add(1_000_000.0, 0.0).tanh());
    output.insert(MarketFeature::Beta, mean(&betas)?.tanh());
    output.insert(MarketFeature::CorrelationBreakdown, mean(&breakdowns)?);
    output.insert(MarketFeature::RelativeStrength, (mean(&relative_strengths)? * 100.0).tanh());
    output.insert(MarketFeature::RegimeCorrelationStress,
        mean(&correlations.iter().map(|value| value.abs()).collect::<Vec<_>>())?);
    Some((output, quality))
}

/// Persist a sparse but honest 53-coordinate snapshot from the authoritative V1
/// engine output plus event-time-aligned cross-asset evidence. Unsupported
/// portfolio/Greeks dimensions remain explicitly NotApplicable.
pub(super) fn build_market_vector(
    event: &MarketEvent,
    state: &SymbolState,
    signal: &Signal,
    available_at_ms: i64,
    peers: &[SymbolState],
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
    if let Some((cross_features, quality)) = cross_asset_raw_features(state, peers, cutoff) {
        for (feature, value) in cross_features {
            if vector.set_value(feature, Some(value.clamp(-1.0, 1.0)), quality).is_ok() {
                observed += 1;
            }
        }
    }
    if observed == 0 || vector.validate().is_err() { return None; }
    Some(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(venue: &str, symbol: &str, last_ts: i64, multiplier: f64) -> SymbolState {
        let mut value = SymbolState::default();
        value.venue = venue.into();
        value.symbol = symbol.into();
        value.last_price_ts_ms = Some(last_ts);
        value.state_quality = 1.0;
        for minute in 1..24 {
            let base = minute as f64 * 0.0002;
            value.recent_return_observations.push_back((minute * 60_000 + 1_000, base * multiplier + 0.00001));
        }
        value
    }

    #[test]
    fn cross_asset_alignment_uses_same_venue_and_quote_currency() {
        let current = state("binance_usdm", "BTCUSDT", 24 * 60_000, 1.0);
        let peer = state("binance_usdm", "ETHUSDT", 24 * 60_000, 1.8);
        let foreign_venue = state("kraken_spot", "ETH/USD", 24 * 60_000, 1.2);
        let foreign_quote = state("binance_usdm", "ETHBTC", 24 * 60_000, 1.2);
        let (features, quality) = cross_asset_raw_features(&current, &[peer, foreign_venue, foreign_quote], 24 * 60_000).unwrap();
        assert!(features[&MarketFeature::Correlation] > 0.99);
        assert!(features[&MarketFeature::RegimeCorrelationStress] > 0.99);
        assert!(features.contains_key(&MarketFeature::CorrelationBreakdown));
        assert!(quality > 0.0 && quality <= 1.0);
    }

    #[test]
    fn cross_asset_alignment_excludes_future_peer_data() {
        let current = state("binance_usdm", "BTCUSDT", 15 * 60_000 + 2_000, 1.0);
        let mut peer = state("binance_usdm", "ETHUSDT", 16 * 60_000 + 1_000, 1.8);
        peer.recent_return_observations.push_back((16 * 60_000 + 1_000, 0.01));
        assert!(cross_asset_raw_features(&current, &[peer], 15 * 60_000 + 2_000).is_none());
    }

    #[test]
    fn cross_asset_alignment_requires_twelve_completed_buckets() {
        let current = state("binance_usdm", "BTCUSDT", 13 * 60_000, 1.0);
        let peer = state("binance_usdm", "ETHUSDT", 13 * 60_000, 1.8);
        assert!(cross_asset_raw_features(&current, &[peer], 13 * 60_000).is_none());
    }
}
