//! Deterministic Version 2 finite-set and temporal-relation projection.
//!
//! This module derives a read-only finite-set view and builds append-only
//! observation/relation records. It has no trading authority.

use std::collections::BTreeSet;

use mongodb::bson::{doc, Document};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::MarketEvent;

const RELATION_WINDOW_MS: i64 = 60_000;

#[derive(Debug, Clone, Serialize)]
pub struct SetMeasure {
    pub counting_measure: usize,
    pub trade_size_measure: f64,
    pub trade_size_unit: &'static str,
    pub trade_size_observed_count: usize,
    pub trade_size_missing_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSet {
    pub set_id: &'static str,
    pub label: &'static str,
    pub member_event_ids: Vec<String>,
    pub measure: SetMeasure,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationEdge {
    pub from_event_id: String,
    pub to_event_id: String,
    pub lag_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRelation {
    pub relation_id: &'static str,
    pub label: &'static str,
    pub relation_type: &'static str,
    pub source_set: &'static str,
    pub target_set: &'static str,
    pub window_ms: i64,
    pub edge_count: usize,
    pub mean_lag_ms: Option<f64>,
    pub edges: Vec<RelationEdge>,
    pub edges_truncated: bool,
    pub interpretation: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetLedger {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub persistence: &'static str,
    pub symbol: String,
    pub universe_count: usize,
    pub requested_limit: usize,
    pub universe_truncated: bool,
    pub skipped_invalid_events: usize,
    pub window_start_ts_ms: Option<i64>,
    pub window_end_ts_ms: Option<i64>,
    pub sets: Vec<EventSet>,
    pub relations: Vec<EventRelation>,
    pub limitations: Vec<&'static str>,
}

fn event_key(event: &MarketEvent) -> String {
    format!("{}:{}:{}", event.venue, event.symbol, event.event_id)
}

fn measure(events: &[&MarketEvent]) -> SetMeasure {
    let mut volume_total = 0.0;
    let mut volume_observed_count = 0;
    for event in events {
        if event.event_type == "trade" {
            if let Some(size) = event.size.filter(|value| value.is_finite() && *value >= 0.0) {
                volume_total += size;
                volume_observed_count += 1;
            }
        }
    }
    let trade_count = events.iter().filter(|event| event.event_type == "trade").count();
    SetMeasure {
        counting_measure: events.len(),
        trade_size_measure: volume_total,
        trade_size_unit: "instrument_quantity_units",
        trade_size_observed_count: volume_observed_count,
        trade_size_missing_count: trade_count.saturating_sub(volume_observed_count),
    }
}

fn make_set<'a>(
    set_id: &'static str,
    label: &'static str,
    universe: &[&'a MarketEvent],
    predicate: impl Fn(&MarketEvent) -> bool,
) -> (EventSet, Vec<&'a MarketEvent>) {
    let members: Vec<&MarketEvent> = universe.iter().copied().filter(|event| predicate(event)).collect();
    let mut ids: Vec<String> = members.iter().map(|event| event_key(event)).collect();
    ids.sort();
    (
        EventSet {
            set_id,
            label,
            member_event_ids: ids,
            measure: measure(&members),
        },
        members,
    )
}

fn relation(
    relation_id: &'static str,
    label: &'static str,
    source_set: &'static str,
    target_set: &'static str,
    sources: &[&MarketEvent],
    targets: &[&MarketEvent],
) -> EventRelation {
    let mut edges = Vec::new();
    let mut edge_count = 0usize;
    let mut lag_total = 0i128;
    for source in sources {
        for target in targets {
            let lag_ms = target.ts_ms - source.ts_ms;
            if (0..=RELATION_WINDOW_MS).contains(&lag_ms) {
                edge_count += 1;
                lag_total += lag_ms as i128;
                if edges.len() < 100 {
                    edges.push(RelationEdge {
                        from_event_id: event_key(source),
                        to_event_id: event_key(target),
                        lag_ms,
                    });
                }
            }
        }
    }
    edges.sort_by(|left, right| {
        left.lag_ms.cmp(&right.lag_ms)
            .then_with(|| left.from_event_id.cmp(&right.from_event_id))
            .then_with(|| left.to_event_id.cmp(&right.to_event_id))
    });
    let mean_lag_ms = if edge_count == 0 { None } else { Some(lag_total as f64 / edge_count as f64) };
    EventRelation {
        relation_id,
        label,
        relation_type: "temporal_precedence",
        source_set,
        target_set,
        window_ms: RELATION_WINDOW_MS,
        edge_count,
        mean_lag_ms,
        edges_truncated: edge_count > edges.len(),
        edges,
        interpretation: "Observed temporal ordering only; this relation does not establish causation.",
    }
}


/// Build immutable, idempotent V2 ledger records for one newly persisted event.
/// One observation record captures set membership and its source hash. Separate
/// relation records capture every eligible temporal edge in the bounded local
/// event-time neighbourhood. MongoDB's built-in unique _id makes retries safe.
pub fn durable_records(
    event: &MarketEvent,
    neighbours: &[MarketEvent],
    ingested_at: mongodb::bson::DateTime,
) -> Result<Vec<Document>, String> {
    let serialized = serde_json::to_vec(event).map_err(|error| error.to_string())?;
    let source_hash = hex::encode(Sha256::digest(serialized));
    let event_id = event_key(event);
    let mut memberships = vec!["all_events"];
    match event.event_type.as_str() {
        "trade" => {
            memberships.push("trades");
            match event.side.as_deref() {
                Some("buy") => memberships.push("buy_trades"),
                Some("sell") => memberships.push("sell_trades"),
                _ => {}
            }
        }
        "book" => memberships.push("book_events"),
        "funding" => {
            memberships.push("funding_events");
            match event.funding_rate {
                Some(value) if value > 0.0 => memberships.push("positive_funding"),
                Some(value) if value < 0.0 => memberships.push("negative_funding"),
                _ => {}
            }
        }
        "open_interest" => memberships.push("open_interest_events"),
        "liquidation" => {
            memberships.push("liquidations");
            match event.liquidation_side.as_deref() {
                Some("long") => memberships.push("long_liquidations"),
                Some("short") => memberships.push("short_liquidations"),
                _ => {}
            }
        }
        _ => {}
    }
    let event_document = mongodb::bson::to_document(event).map_err(|error| error.to_string())?;
    let mut records = vec![doc! {
        "_id": format!("observation|{}", event_id),
        "record_id": format!("observation|{}", event_id),
        "record_type": "observation",
        "schema_version": "qoin-relation-ledger-v2-alpha2",
        "symbol": &event.symbol,
        "venue": &event.venue,
        "source_event_id": &event.event_id,
        "source_event_key": &event_id,
        "source_event_hash_sha256": source_hash,
        "event_time_ms": event.ts_ms,
        "ingested_at": ingested_at.clone(),
        "source_event": event_document,
        "set_memberships": memberships,
        "counting_measure": 1_i64,
        "trade_size_measure": if event.event_type == "trade" { event.size.unwrap_or(0.0) } else { 0.0 },
        "trade_size_observed": event.event_type == "trade" && event.size.is_some(),
        "trade_size_unit": "instrument_quantity_units",
        "immutability": "append_only_supersede_to_correct"
    }];

    for neighbour in neighbours {
        if event_key(neighbour) == event_id { continue; }
        let (relation_id, source, target) = match (
            event.event_type.as_str(),
            event.side.as_deref(),
            neighbour.event_type.as_str(),
            neighbour.side.as_deref(),
        ) {
            ("trade", Some("buy"), "liquidation", _) => ("buy_trades_before_liquidations", event, neighbour),
            ("liquidation", _, "trade", Some("buy")) => ("buy_trades_before_liquidations", neighbour, event),
            ("trade", Some("sell"), "liquidation", _) => ("sell_trades_before_liquidations", event, neighbour),
            ("liquidation", _, "trade", Some("sell")) => ("sell_trades_before_liquidations", neighbour, event),
            ("funding", _, "open_interest", _) => ("funding_before_open_interest", event, neighbour),
            ("open_interest", _, "funding", _) => ("funding_before_open_interest", neighbour, event),
            _ => continue,
        };
        let lag_ms = target.ts_ms - source.ts_ms;
        if !(0..=RELATION_WINDOW_MS).contains(&lag_ms) { continue; }
        let source_key = event_key(source);
        let target_key = event_key(target);
        let record_id = format!("relation|{relation_id}|{source_key}|{target_key}");
        records.push(doc! {
            "_id": &record_id,
            "record_id": &record_id,
            "record_type": "relation_edge",
            "schema_version": "qoin-relation-ledger-v2-alpha2",
            "symbol": &event.symbol,
            "relation_id": relation_id,
            "relation_type": "temporal_precedence",
            "source_set": match relation_id {
                "buy_trades_before_liquidations" => "buy_trades",
                "sell_trades_before_liquidations" => "sell_trades",
                _ => "funding_events"
            },
            "target_set": match relation_id {
                "buy_trades_before_liquidations" | "sell_trades_before_liquidations" => "liquidations",
                _ => "open_interest_events"
            },
            "source_event_key": source_key,
            "target_event_key": target_key,
            "source_event_id": &source.event_id,
            "target_event_id": &target.event_id,
            "source_venue": &source.venue,
            "target_venue": &target.venue,
            "source_event_time_ms": source.ts_ms,
            "target_event_time_ms": target.ts_ms,
            "lag_ms": lag_ms,
            "window_ms": RELATION_WINDOW_MS,
            "ingested_at": ingested_at,
            "interpretation": "Observed temporal ordering only; this relation does not establish causation.",
            "immutability": "append_only_supersede_to_correct"
        });
    }
    Ok(records)
}

/// Build a deterministic finite-set view over one instrument's persisted events.
pub fn build(symbol: &str, events: &[MarketEvent], skipped_invalid_events: usize, requested_limit: usize, universe_truncated: bool) -> SetLedger {
    let mut ordered: Vec<&MarketEvent> = events.iter().collect();
    ordered.sort_by(|left, right| {
        left.ts_ms.cmp(&right.ts_ms)
            .then_with(|| left.event_id.cmp(&right.event_id))
            .then_with(|| left.venue.cmp(&right.venue))
    });
    let window_start_ts_ms = ordered.first().map(|event| event.ts_ms);
    let window_end_ts_ms = ordered.last().map(|event| event.ts_ms);

    let (all, _) = make_set("all_events", "All validated observations", &ordered, |_| true);
    let (trades, trade_members) = make_set("trades", "Trade observations", &ordered, |event| event.event_type == "trade");
    let (buy_trades, buy_members) = make_set("buy_trades", "Buy-aggressor trades", &ordered, |event| event.event_type == "trade" && event.side.as_deref() == Some("buy"));
    let (sell_trades, sell_members) = make_set("sell_trades", "Sell-aggressor trades", &ordered, |event| event.event_type == "trade" && event.side.as_deref() == Some("sell"));
    let (books, _) = make_set("book_events", "Order-book observations", &ordered, |event| event.event_type == "book");
    let (funding, funding_members) = make_set("funding_events", "Funding observations", &ordered, |event| event.event_type == "funding");
    let (positive_funding, _) = make_set("positive_funding", "Positive funding observations", &ordered, |event| event.event_type == "funding" && event.funding_rate.is_some_and(|value| value > 0.0));
    let (negative_funding, _) = make_set("negative_funding", "Negative funding observations", &ordered, |event| event.event_type == "funding" && event.funding_rate.is_some_and(|value| value < 0.0));
    let (open_interest, oi_members) = make_set("open_interest_events", "Open-interest observations", &ordered, |event| event.event_type == "open_interest");
    let (liquidations, liquidation_members) = make_set("liquidations", "Liquidation observations", &ordered, |event| event.event_type == "liquidation");
    let (long_liquidations, _) = make_set("long_liquidations", "Long-position liquidation observations", &ordered, |event| event.event_type == "liquidation" && event.liquidation_side.as_deref() == Some("long"));
    let (short_liquidations, _) = make_set("short_liquidations", "Short-position liquidation observations", &ordered, |event| event.event_type == "liquidation" && event.liquidation_side.as_deref() == Some("short"));

    let relations = vec![
        relation("buy_trades_before_liquidations", "Buy trades preceding liquidations", "buy_trades", "liquidations", &buy_members, &liquidation_members),
        relation("sell_trades_before_liquidations", "Sell trades preceding liquidations", "sell_trades", "liquidations", &sell_members, &liquidation_members),
        relation("funding_before_open_interest", "Funding observations preceding open-interest observations", "funding_events", "open_interest_events", &funding_members, &oi_members),
    ];

    let sets = vec![all, trades, buy_trades, sell_trades, books, funding, positive_funding, negative_funding, open_interest, liquidations, long_liquidations, short_liquidations];
    let mut unique_ids = BTreeSet::new();
    for event in &ordered {
        unique_ids.insert(event_key(event));
    }

    SetLedger {
        schema_version: "qoin-set-ledger-v2-alpha1",
        status: "derived_read_only",
        persistence: "computed_from_persisted_events_on_read_not_append_only",
        symbol: symbol.to_owned(),
        universe_count: unique_ids.len(),
        requested_limit,
        universe_truncated,
        skipped_invalid_events,
        window_start_ts_ms,
        window_end_ts_ms,
        sets,
        relations,
        limitations: vec![
            "The universe is limited to the raw events returned for this request.",
            "Counting measure is exact for the returned finite event universe; volume measure is the sum of supplied trade sizes only.",
            "Missing trade sizes are counted separately and are never replaced with zero observations.",
            "Temporal relations use a fixed 60-second window and are not causal claims.",
            "At most 100 edge examples per relation are returned; edge_count and mean lag are computed across all matching pairs.",
            "This is a derived read-only projection, not yet an append-only durable ledger.",
            "No V2 output changes the Version 1 vector decision or authorizes an order.",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, event_type: &str, ts_ms: i64) -> MarketEvent {
        MarketEvent {
            event_id: id.into(),
            venue: "test".into(),
            symbol: "BTCUSDT".into(),
            event_type: event_type.into(),
            ts_ms,
            price: Some(100.0),
            size: Some(2.0),
            side: None,
            bid: None,
            ask: None,
            bid_size: None,
            ask_size: None,
            funding_rate: None,
            open_interest: None,
            liquidation_side: None,
        }
    }

    #[test]
    fn creates_disjoint_event_type_sets_and_temporal_relations() {
        let mut buy = event("buy-1", "trade", 1_000);
        buy.side = Some("buy".into());
        buy.size = None;
        let mut sell = event("sell-1", "trade", 1_500);
        sell.side = Some("sell".into());
        let mut liquidation = event("liq-1", "liquidation", 2_000);
        liquidation.liquidation_side = Some("long".into());
        let mut funding = event("fund-1", "funding", 3_000);
        funding.funding_rate = Some(0.0001);
        let oi = event("oi-1", "open_interest", 4_000);

        let ledger = build("BTCUSDT", &[oi, funding, liquidation, sell, buy], 0, 1000, false);
        assert_eq!(ledger.universe_count, 5);
        assert_eq!(ledger.window_start_ts_ms, Some(1_000));
        assert_eq!(ledger.window_end_ts_ms, Some(4_000));
        let set = |id: &str| ledger.sets.iter().find(|item| item.set_id == id).unwrap();
        assert_eq!(set("buy_trades").measure.counting_measure, 1);
        assert_eq!(set("sell_trades").measure.counting_measure, 1);
        assert_eq!(set("long_liquidations").measure.counting_measure, 1);
        assert_eq!(set("positive_funding").measure.counting_measure, 1);
        assert_eq!(set("trades").measure.trade_size_measure, 2.0);
        assert_eq!(set("trades").measure.trade_size_observed_count, 1);
        assert_eq!(set("trades").measure.trade_size_missing_count, 1);
        assert_eq!(ledger.relations[0].edge_count, 1);
        assert_eq!(ledger.relations[1].edge_count, 1);
        assert_eq!(ledger.relations[2].edge_count, 1);
        assert!(ledger.relations.iter().all(|item| item.relation_type == "temporal_precedence"));
    }

    #[test]
    fn excludes_events_outside_the_declared_relation_window() {
        let mut buy = event("buy-1", "trade", 1_000);
        buy.side = Some("buy".into());
        let liquidation = event("liq-1", "liquidation", 61_001);
        let ledger = build("BTCUSDT", &[buy, liquidation], 0, 1000, false);
        assert_eq!(ledger.relations[0].edge_count, 0);
    }

    #[test]
    fn caps_returned_relation_examples_without_truncating_exact_edge_count() {
        let mut events = Vec::new();
        for index in 0..101 {
            let mut trade = event(&format!("buy-{index}"), "trade", 1_000 + index);
            trade.side = Some("buy".into());
            events.push(trade);
            events.push(event(&format!("liq-{index}"), "liquidation", 2_000 + index));
        }
        let ledger = build("BTCUSDT", &events, 0, 1000, false);
        let relation = &ledger.relations[0];
        assert_eq!(relation.edge_count, 101 * 101);
        assert_eq!(relation.edges.len(), 100);
        assert!(relation.edges_truncated);
        assert!(relation.mean_lag_ms.is_some());
    }

    #[test]
    fn durable_records_capture_memberships_hash_and_temporal_edges() {
        let mut buy = event("buy-immutable", "trade", 10_000);
        buy.side = Some("buy".into());
        let mut liquidation = event("liq-immutable", "liquidation", 10_250);
        liquidation.liquidation_side = Some("long".into());
        let records = durable_records(
            &liquidation,
            &[buy.clone(), liquidation.clone()],
            mongodb::bson::DateTime::now(),
        ).unwrap();
        assert_eq!(records.len(), 2);
        let observation = records.iter().find(|record| record.get_str("record_type").ok() == Some("observation")).unwrap();
        assert_eq!(observation.get_str("schema_version").ok(), Some("qoin-relation-ledger-v2-alpha2"));
        assert_eq!(observation.get_str("source_event_hash_sha256").unwrap().len(), 64);
        let relation = records.iter().find(|record| record.get_str("record_type").ok() == Some("relation_edge")).unwrap();
        assert_eq!(relation.get_str("relation_id").ok(), Some("buy_trades_before_liquidations"));
        assert_eq!(relation.get_i64("lag_ms").ok(), Some(250));
        assert!(relation.get_str("interpretation").unwrap().contains("does not establish causation"));
    }

    #[test]
    fn reports_empty_universe_without_inventing_members() {
        let ledger = build("BTCUSDT", &[], 2, 1000, false);
        assert_eq!(ledger.universe_count, 0);
        assert_eq!(ledger.skipped_invalid_events, 2);
        assert!(ledger.sets.iter().all(|item| item.measure.counting_measure == 0));
        assert!(ledger.relations.iter().all(|item| item.edges.is_empty()));
    }
}
