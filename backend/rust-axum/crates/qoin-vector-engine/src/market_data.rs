//! Causal market-event aggregation into horizon-specific force observations.
//!
//! The caller supplies an event window bounded by the decision timestamp. This
//! module never reads future events, fills missing fields with invented values,
//! or performs exchange execution.

use serde::{Deserialize, Serialize};

use crate::ForceObservation;
use crate::multi_horizon::Horizon;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketEvent {
    pub ts_ms: i64,
    pub event_type: String,
    pub price: Option<f64>,
    pub size: Option<f64>,
    pub side: Option<String>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub bid_size: Option<f64>,
    pub ask_size: Option<f64>,
    pub funding_rate: Option<f64>,
    pub open_interest: Option<f64>,
    pub liquidation_side: Option<String>,
    pub basis: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HorizonObservation {
    pub horizon: Horizon,
    pub observation: ForceObservation,
    pub event_count: usize,
    pub trade_count: usize,
    pub book_count: usize,
    pub first_event_ts_ms: Option<i64>,
    pub last_event_ts_ms: Option<i64>,
    pub data_quality: f64,
    pub coverage_ratio: f64,
    pub ready: bool,
    pub reasons: Vec<String>,
}

pub fn aggregate_horizon(
    events: &[MarketEvent],
    horizon: Horizon,
    decision_ts_ms: i64,
    min_events: usize,
) -> HorizonObservation {
    let cutoff = decision_ts_ms.saturating_sub(
        (horizon.duration_seconds() as i64).saturating_mul(1_000),
    );
    let mut rows: Vec<&MarketEvent> = events.iter()
        .filter(|event| event.ts_ms > 0 && event.ts_ms >= cutoff && event.ts_ms <= decision_ts_ms)
        .filter(|event| event.numeric_values_are_finite())
        .collect();
    rows.sort_by_key(|event| event.ts_ms);

    let mut signed_trade = 0.0;
    let mut classified_trade_size = 0.0;
    let mut book_imbalance_sum = 0.0;
    let mut book_count = 0usize;
    let mut squared_log_returns = 0.0;
    let mut return_count = 0usize;
    let mut previous_price: Option<f64> = None;
    let mut first_oi: Option<f64> = None;
    let mut last_oi: Option<f64> = None;
    let mut latest_funding: Option<f64> = None;
    let mut latest_basis: Option<f64> = None;
    let mut signed_liquidations = 0.0;
    let mut total_liquidations = 0.0;
    let mut trade_count = 0usize;

    for event in &rows {
        match event.event_type.as_str() {
            "trade" => {
                let size = event.size.unwrap_or(0.0);
                if size > 0.0 {
                    match event.side.as_deref() {
                        Some("buy") => {
                            signed_trade += size;
                            classified_trade_size += size;
                            trade_count += 1;
                        }
                        Some("sell") => {
                            signed_trade -= size;
                            classified_trade_size += size;
                            trade_count += 1;
                        }
                        _ => {}
                    }
                }
                if let Some(price) = event.price.filter(|price| *price > 0.0) {
                    if let Some(previous) = previous_price.filter(|previous| *previous > 0.0) {
                        let log_return = (price / previous).ln();
                        squared_log_returns += log_return * log_return;
                        return_count += 1;
                    }
                    previous_price = Some(price);
                }
            }
            "book" => {
                if let (Some(bid_size), Some(ask_size)) = (event.bid_size, event.ask_size) {
                    let depth = bid_size + ask_size;
                    if depth > 0.0 {
                        book_imbalance_sum += (bid_size - ask_size) / depth;
                        book_count += 1;
                    }
                }
            }
            "funding" => {
                if event.funding_rate.is_some() {
                    latest_funding = event.funding_rate;
                }
            }
            "basis" => {
                if event.basis.is_some() {
                    latest_basis = event.basis;
                }
            }
            "open_interest" => {
                if let Some(value) = event.open_interest {
                    first_oi.get_or_insert(value);
                    last_oi = Some(value);
                }
            }
            "liquidation" => {
                let size = event.size.unwrap_or(0.0);
                if size > 0.0 {
                    total_liquidations += size;
                    match event.liquidation_side.as_deref() {
                        Some("short") => signed_liquidations += size,
                        Some("long") => signed_liquidations -= size,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // Unknown aggressor-side trades must not dilute signed OFI or count as
    // signed-flow evidence for readiness.
    let ofi = if classified_trade_size > 0.0 {
        signed_trade / classified_trade_size
    } else {
        0.0
    };
    let book_imbalance = if book_count > 0 {
        book_imbalance_sum / book_count as f64
    } else { 0.0 };
    let open_interest_change = match (first_oi, last_oi) {
        (Some(first), Some(last)) if first > 0.0 => (last - first) / first,
        _ => 0.0,
    };
    let liquidation_pressure = if total_liquidations > 0.0 {
        signed_liquidations / total_liquidations
    } else { 0.0 };
    // Explicit research normalization scales: 1% realized move, 10 bps funding,
    // 1% OI change, and 10 bps spot/perpetual basis map to unit magnitude.
    let volatility = if return_count > 0 {
        (squared_log_returns.sqrt() / 0.01).clamp(0.0, 3.0)
    } else { 0.0 };
    let funding_force = (latest_funding.unwrap_or(0.0) / 0.001).clamp(-3.0, 3.0);
    let open_interest_force = (open_interest_change / 0.01).clamp(-3.0, 3.0);
    let basis_force = (latest_basis.unwrap_or(0.0) / 0.001).clamp(-3.0, 3.0);

    let event_count = rows.len();
    let first_event_ts_ms = rows.first().map(|event| event.ts_ms);
    let last_event_ts_ms = rows.last().map(|event| event.ts_ms);
    let observed_span_ms = match (first_event_ts_ms, last_event_ts_ms) {
        (Some(first), Some(last)) => last.saturating_sub(first).max(0) as f64,
        _ => 0.0,
    };
    // Require evidence distributed across at least 15% of the target window.
    // This is a conservative completeness gate, not a performance parameter.
    const MIN_WINDOW_COVERAGE_RATIO: f64 = 0.15;
    let coverage_ratio = (observed_span_ms
        / (horizon.duration_seconds() as f64 * 1_000.0))
        .clamp(0.0, 1.0);
    let mut reasons = Vec::new();
    if event_count < min_events {
        reasons.push(format!("requires at least {min_events} events; observed {event_count}"));
    }
    if coverage_ratio < MIN_WINDOW_COVERAGE_RATIO {
        reasons.push(format!(
            "window coverage {:.1}% is below the 15% minimum",
            coverage_ratio * 100.0,
        ));
    }
    if trade_count == 0 {
        reasons.push("no signed trade-flow observations in this window".to_owned());
    }
    if book_count == 0 {
        reasons.push("no order-book depth observations in this window".to_owned());
    }
    if first_oi.is_none() || last_oi.is_none() {
        reasons.push("open-interest change unavailable; treated as neutral".to_owned());
    }
    if latest_funding.is_none() {
        reasons.push("funding unavailable; treated as neutral".to_owned());
    }
    if latest_basis.is_none() {
        reasons.push("spot-perpetual basis unavailable; treated as neutral".to_owned());
    }
    let data_quality = (event_count as f64 / min_events.max(1) as f64).min(1.0)
        * ((trade_count > 0) as u8 as f64 * 0.5 + (book_count > 0) as u8 as f64 * 0.5);

    HorizonObservation {
        horizon,
        observation: ForceObservation {
            ofi,
            liquidity: book_imbalance,
            volatility,
            funding: funding_force,
            open_interest_change: open_interest_force,
            liquidation_pressure,
            basis: basis_force,
        },
        event_count,
        trade_count,
        book_count,
        first_event_ts_ms,
        last_event_ts_ms,
        data_quality,
        coverage_ratio,
        ready: event_count >= min_events
            && trade_count > 0
            && book_count > 0
            && coverage_ratio >= MIN_WINDOW_COVERAGE_RATIO,
        reasons,
    }
}

impl MarketEvent {
    fn numeric_values_are_finite(&self) -> bool {
        [
            self.price, self.size, self.bid, self.ask, self.bid_size, self.ask_size,
            self.funding_rate, self.open_interest, self.basis,
        ].into_iter().flatten().all(f64::is_finite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(ts_ms: i64, event_type: &str) -> MarketEvent {
        MarketEvent {
            ts_ms,
            event_type: event_type.to_owned(),
            price: None,
            size: None,
            side: None,
            bid: None,
            ask: None,
            bid_size: None,
            ask_size: None,
            funding_rate: None,
            open_interest: None,
            liquidation_side: None,
            basis: None,
        }
    }

    #[test]
    fn excludes_future_and_out_of_window_events() {
        let mut old = event(1_000, "trade");
        old.size = Some(10.0);
        old.side = Some("buy".into());
        let mut current = event(299_000, "trade");
        current.size = Some(2.0);
        current.side = Some("sell".into());
        let mut future = event(301_000, "trade");
        future.size = Some(100.0);
        future.side = Some("buy".into());

        let result = aggregate_horizon(
            &[old, current, future], Horizon::FiveMinutes, 300_000, 1,
        );
        assert_eq!(result.event_count, 2);
        assert_eq!(result.observation.ofi, (10.0 - 2.0) / 12.0);
    }

    #[test]
    fn derives_book_imbalance_oi_funding_and_liquidation_pressure() {
        let mut trade = event(100_000, "trade");
        trade.size = Some(4.0);
        trade.side = Some("buy".into());
        let mut book = event(110_000, "book");
        book.bid_size = Some(3.0);
        book.ask_size = Some(1.0);
        let mut oi_a = event(120_000, "open_interest");
        oi_a.open_interest = Some(100.0);
        let mut oi_b = event(130_000, "open_interest");
        oi_b.open_interest = Some(110.0);
        let mut funding = event(140_000, "funding");
        funding.funding_rate = Some(0.001);
        let mut liquidation = event(150_000, "liquidation");
        liquidation.size = Some(5.0);
        liquidation.liquidation_side = Some("short".into());

        let result = aggregate_horizon(
            &[trade, book, oi_a, oi_b, funding, liquidation],
            Horizon::OneMinute, 200_000, 1,
        );
        assert_eq!(result.observation.liquidity, 0.5);
        assert!((result.observation.open_interest_change - 3.0).abs() < 1e-12);
        assert_eq!(result.observation.funding, 1.0);
        assert_eq!(result.observation.liquidation_pressure, 1.0);
        assert!(result.ready);
    }

    #[test]
    fn basis_event_is_carried_into_the_horizon_force_vector() {
        let mut basis = event(100_000, "basis");
        basis.basis = Some(0.0025);
        let result = aggregate_horizon(&[basis], Horizon::OneHour, 100_000, 1);
        assert_eq!(result.observation.basis, 2.5);
        assert!(!result.reasons.iter().any(|reason| reason.contains("basis unavailable")));
    }

    #[test]
    fn insufficient_or_missing_book_data_is_not_ready() {
        let mut trade = event(100_000, "trade");
        trade.size = Some(1.0);
        trade.side = Some("buy".into());
        let result = aggregate_horizon(&[trade], Horizon::FiveMinutes, 100_000, 1);
        assert!(!result.ready);
        assert!(result.reasons.iter().any(|reason| reason.contains("order-book")));
    }

    #[test]
    fn long_horizon_requires_events_spread_across_its_window() {
        let mut trade = event(100_000, "trade");
        trade.size = Some(1.0);
        trade.side = Some("buy".into());
        let mut book = event(120_000, "book");
        book.bid_size = Some(2.0);
        book.ask_size = Some(1.0);

        let result = aggregate_horizon(
            &[trade, book],
            Horizon::OneDay,
            130_000,
            2,
        );
        assert!(result.coverage_ratio < 0.15);
        assert!(!result.ready);
        assert!(result.reasons.iter().any(|reason| reason.contains("window coverage")));
    }

    #[test]
    fn volatility_uses_root_sum_squared_log_returns() {
        let mut first = event(100_000, "trade");
        first.price = Some(100.0);
        first.size = Some(1.0);
        first.side = Some("buy".into());
        let mut second = event(110_000, "trade");
        second.price = Some(101.0);
        second.size = Some(1.0);
        second.side = Some("buy".into());
        let mut third = event(120_000, "trade");
        third.price = Some(100.0);
        third.size = Some(1.0);
        third.side = Some("sell".into());

        let result = aggregate_horizon(
            &[first, second, third],
            Horizon::OneHour,
            130_000,
            1,
        );
        let expected = (((101.0_f64 / 100.0).ln().powi(2)
            + (100.0_f64 / 101.0).ln().powi(2)).sqrt() / 0.01)
            .clamp(0.0, 3.0);
        assert!((result.observation.volatility - expected).abs() < 1e-12);
    }

    #[test]
    fn unknown_trade_sides_do_not_dilute_signed_flow_or_readiness() {
        let mut buy = event(100_000, "trade");
        buy.size = Some(2.0);
        buy.side = Some("buy".into());
        let mut unknown = event(110_000, "trade");
        unknown.size = Some(100.0);
        unknown.side = Some("unknown".into());
        let mut book = event(120_000, "book");
        book.bid_size = Some(2.0);
        book.ask_size = Some(1.0);

        let result = aggregate_horizon(
            &[buy, unknown, book],
            Horizon::OneMinute,
            130_000,
            2,
        );
        assert_eq!(result.observation.ofi, 1.0);
        assert_eq!(result.trade_count, 1);
        assert!(result.ready);
    }

    #[test]
    fn non_finite_events_are_excluded() {
        let mut invalid = event(100_000, "trade");
        invalid.size = Some(f64::NAN);
        invalid.side = Some("buy".into());
        let result = aggregate_horizon(&[invalid], Horizon::FiveMinutes, 100_000, 1);
        assert_eq!(result.event_count, 0);
        assert!(!result.ready);
    }
}
