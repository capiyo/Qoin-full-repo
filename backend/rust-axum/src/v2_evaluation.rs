//! Independent, direction-neutral evaluation of V2 set relationships.
//! V2 does not issue orders or replace the Vector V1 decision authority.

use std::collections::BTreeMap;
use serde::Serialize;
use qoin_set_theoretic_attention::{RelationshipEventType, RelationshipLedgerEvent};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceObservation {
    pub ts_ms: i64,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationshipOutcomeSample {
    pub event_id: String,
    pub relationship_id: String,
    pub relation_kind: String,
    pub observed_at_ms: i64,
    pub evaluated_at_ms: i64,
    pub horizon_ms: i64,
    pub attention_weight: f64,
    pub source_atom_count: usize,
    pub target_atom_count: usize,
    pub intersection_count: usize,
    pub union_count: usize,
    pub realized_return_bps: f64,
    pub max_adverse_excursion_bps: f64,
    pub max_favorable_excursion_bps: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationshipKindSummary {
    pub relation_kind: String,
    pub samples: usize,
    pub positive: usize,
    pub negative: usize,
    pub flat: usize,
    pub mean_return_bps: f64,
    pub attention_weighted_mean_return_bps: f64,
    pub mean_adverse_excursion_bps: f64,
    pub mean_favorable_excursion_bps: f64,
    pub mean_attention_weight: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct V2EvaluationReport {
    pub symbol: String,
    pub venue: String,
    pub horizon_ms: i64,
    pub discovered_relationships: usize,
    pub evaluated_relationships: usize,
    pub unresolved_relationships: usize,
    pub samples: Vec<RelationshipOutcomeSample>,
    pub by_relation_kind: Vec<RelationshipKindSummary>,
    pub interpretation: Vec<String>,
}

fn valid_price(value: f64) -> bool { value.is_finite() && value > 0.0 }

/// Evaluate one relationship using only executable observations strictly after
/// the relationship's evidence timestamp. The first later observation anchors
/// the return; the final observation no later than the horizon closes it.
pub fn evaluate_forward_window(
    observations: &[PriceObservation],
    observed_at_ms: i64,
    horizon_ms: i64,
) -> Option<(i64, f64, f64, f64)> {
    if horizon_ms <= 0 { return None; }
    let end_ms = observed_at_ms.checked_add(horizon_ms)?;
    let mut ordered = observations.iter()
        .filter(|point| point.ts_ms > observed_at_ms && point.ts_ms <= end_ms && valid_price(point.price))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|point| point.ts_ms);
    let first = ordered.first()?;
    let last = ordered.last()?;
    if last.ts_ms <= first.ts_ms { return None; }
    let returns = ordered.iter().map(|point| (point.price / first.price - 1.0) * 10_000.0)
        .filter(|value| value.is_finite()).collect::<Vec<_>>();
    if returns.is_empty() { return None; }
    let minimum = returns.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = returns.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let realized = (last.price / first.price - 1.0) * 10_000.0;
    if !realized.is_finite() { return None; }
    Some((last.ts_ms, realized, minimum.min(0.0), maximum.max(0.0)))
}

pub fn evaluate(
    symbol: &str,
    venue: &str,
    horizon_ms: i64,
    ledger: &[RelationshipLedgerEvent],
    observations: &[PriceObservation],
) -> Result<V2EvaluationReport, String> {
    if symbol.trim().is_empty() || venue.trim().is_empty() || horizon_ms <= 0 || horizon_ms > 90 * 24 * 60 * 60 * 1000 {
        return Err("symbol and venue are required; horizon must be in (0, 90 days]".into());
    }
    let discovered = ledger.iter().filter(|event| {
        event.event_type == RelationshipEventType::Discovered
            && event.relationship.source_symbol.as_deref() == Some(symbol)
            && event.relationship.target_symbol.as_deref() == Some(symbol)
            && event.relationship.source_venue.as_deref() == Some(venue)
            && event.relationship.target_venue.as_deref() == Some(venue)
    }).collect::<Vec<_>>();
    let mut samples = Vec::new();
    for event in &discovered {
        let relationship = &event.relationship;
        let Some((evaluated_at_ms, realized_return_bps, max_adverse_excursion_bps, max_favorable_excursion_bps)) =
            evaluate_forward_window(observations, relationship.observed_at_ms, horizon_ms) else { continue; };
        let source = relationship.source_atoms.iter().collect::<std::collections::BTreeSet<_>>();
        let target = relationship.target_atoms.iter().collect::<std::collections::BTreeSet<_>>();
        let intersection_count = source.intersection(&target).count();
        let union_count = source.union(&target).count();
        samples.push(RelationshipOutcomeSample {
            event_id: event.event_id.clone(),
            relationship_id: relationship.relationship_id.clone(),
            relation_kind: format!("{:?}", relationship.relation_kind),
            observed_at_ms: relationship.observed_at_ms,
            evaluated_at_ms,
            horizon_ms,
            attention_weight: relationship.attention_weight,
            source_atom_count: source.len(),
            target_atom_count: target.len(),
            intersection_count,
            union_count,
            realized_return_bps,
            max_adverse_excursion_bps,
            max_favorable_excursion_bps,
        });
    }
    let mut grouped: BTreeMap<String, Vec<&RelationshipOutcomeSample>> = BTreeMap::new();
    for sample in &samples { grouped.entry(sample.relation_kind.clone()).or_default().push(sample); }
    let by_relation_kind = grouped.into_iter().map(|(relation_kind, rows)| {
        let count = rows.len() as f64;
        let sum = |f: fn(&RelationshipOutcomeSample) -> f64| rows.iter().map(|row| f(row)).sum::<f64>();
        let attention_sum = sum(|row| row.attention_weight.max(0.0));
        let weighted = if attention_sum > 0.0 {
            rows.iter().map(|row| row.realized_return_bps * row.attention_weight.max(0.0)).sum::<f64>() / attention_sum
        } else { 0.0 };
        RelationshipKindSummary {
            relation_kind,
            samples: rows.len(),
            positive: rows.iter().filter(|row| row.realized_return_bps > 0.0).count(),
            negative: rows.iter().filter(|row| row.realized_return_bps < 0.0).count(),
            flat: rows.iter().filter(|row| row.realized_return_bps == 0.0).count(),
            mean_return_bps: sum(|row| row.realized_return_bps) / count,
            attention_weighted_mean_return_bps: weighted,
            mean_adverse_excursion_bps: sum(|row| row.max_adverse_excursion_bps) / count,
            mean_favorable_excursion_bps: sum(|row| row.max_favorable_excursion_bps) / count,
            mean_attention_weight: sum(|row| row.attention_weight) / count,
        }
    }).collect();
    let evaluated = samples.len();
    Ok(V2EvaluationReport {
        symbol: symbol.into(),
        venue: venue.into(),
        horizon_ms,
        discovered_relationships: discovered.len(),
        evaluated_relationships: evaluated,
        unresolved_relationships: discovered.len().saturating_sub(evaluated),
        samples,
        by_relation_kind,
        interpretation: vec![
            "This is a forward association study of V2 relationship structure, not a directional trading strategy.".into(),
            "Attention weights are descriptive model weights, not probabilities, causal effects, or position sizes.".into(),
            "Each sample starts strictly after its evidence timestamp; overlapping horizons are not independent observations.".into(),
            "No fees, fills, slippage, portfolio exposure, or order execution are simulated by this V2 report.".into(),
            "Vector V1 remains the sole decision authority; this report cannot change V1 signals or submit orders.".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_window_excludes_the_evidence_bar_and_uses_only_the_horizon() {
        let points = [
            PriceObservation { ts_ms: 100, price: 100.0 },
            PriceObservation { ts_ms: 101, price: 100.0 },
            PriceObservation { ts_ms: 102, price: 102.0 },
            PriceObservation { ts_ms: 103, price: 99.0 },
            PriceObservation { ts_ms: 104, price: 110.0 },
        ];
        let result = evaluate_forward_window(&points, 100, 3).unwrap();
        assert_eq!(result.0, 103);
        assert!((result.1 + 100.0).abs() < 1e-9);
        assert!((result.2 + 100.0).abs() < 1e-9);
        assert!((result.3 - 200.0).abs() < 1e-9);
    }

    #[test]
    fn forward_window_rejects_missing_or_invalid_future_prices() {
        assert!(evaluate_forward_window(&[], 100, 10).is_none());
        assert!(evaluate_forward_window(&[PriceObservation { ts_ms: 101, price: f64::NAN }], 100, 10).is_none());
        assert!(evaluate_forward_window(&[PriceObservation { ts_ms: 100, price: 10.0 }], 100, 10).is_none());
        assert!(evaluate_forward_window(&[PriceObservation { ts_ms: 101, price: 10.0 }], 100, 0).is_none());
    }
}
