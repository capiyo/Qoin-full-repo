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
            if (1..=RELATION_WINDOW_MS).contains(&lag_ms) {
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


/// Stable SHA-256 fingerprint of the canonical serialized source event.
pub fn source_hash(event: &MarketEvent) -> Result<String, String> {
    let serialized = serde_json::to_vec(event).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(serialized)))
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
    let source_hash = source_hash(event)?;
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
        "funding" | "mark_price" => {
            if event.funding_rate.is_some() {
                memberships.push("funding_events");
            }
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
        "schema_version": "qoin-relation-ledger-v2-alpha3",
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
        "trade_size_measure": if event.event_type == "trade" { event.size } else { None },
        "trade_size_observed": event.event_type == "trade" && event.size.is_some(),
        "trade_size_missing": event.event_type == "trade" && event.size.is_none(),
        "trade_size_unit": "instrument_quantity_units",
        "immutability": "append_only_supersede_to_correct"
    }];

    const MAX_EDGES_PER_RELATION: usize = 100;
    let mut candidate_counts=[0usize;3];
    let mut persisted_counts=[0usize;3];
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
            ("funding" | "mark_price", _, "open_interest", _) => ("funding_before_open_interest", event, neighbour),
            ("open_interest", _, "funding" | "mark_price", _) => ("funding_before_open_interest", neighbour, event),
            _ => continue,
        };
        let lag_ms = target.ts_ms - source.ts_ms;
        if !(1..=RELATION_WINDOW_MS).contains(&lag_ms) { continue; }
        let relation_index=match relation_id {
            "buy_trades_before_liquidations"=>0,
            "sell_trades_before_liquidations"=>1,
            _=>2,
        };
        candidate_counts[relation_index]+=1;
        if persisted_counts[relation_index]>=MAX_EDGES_PER_RELATION { continue; }
        persisted_counts[relation_index]+=1;
        let source_key = event_key(source);
        let target_key = event_key(target);
        let record_id = format!("relation|{relation_id}|{source_key}|{target_key}");
        records.push(doc! {
            "_id": &record_id,
            "record_id": &record_id,
            "record_type": "relation_edge",
            "schema_version": "qoin-relation-ledger-v2-alpha3",
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
            "ingested_at": ingested_at.clone(),
            "interpretation": "Observed temporal ordering only; this relation does not establish causation.",
            "immutability": "append_only_supersede_to_correct"
        });
    }
    if let Some(observation)=records.first_mut() {
        observation.insert("relation_edge_cap_per_type",MAX_EDGES_PER_RELATION as i64);
        observation.insert("relation_edge_candidate_counts",doc!{
            "buy_trades_before_liquidations":candidate_counts[0] as i64,
            "sell_trades_before_liquidations":candidate_counts[1] as i64,
            "funding_before_open_interest":candidate_counts[2] as i64
        });
        observation.insert("relation_edge_persisted_counts",doc!{
            "buy_trades_before_liquidations":persisted_counts[0] as i64,
            "sell_trades_before_liquidations":persisted_counts[1] as i64,
            "funding_before_open_interest":persisted_counts[2] as i64
        });
        observation.insert("relation_edges_truncated",candidate_counts.iter().zip(persisted_counts.iter()).any(|(candidate,persisted)|candidate>persisted));
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
    let (trades, _trade_members) = make_set("trades", "Trade observations", &ordered, |event| event.event_type == "trade");
    let (buy_trades, buy_members) = make_set("buy_trades", "Buy-aggressor trades", &ordered, |event| event.event_type == "trade" && event.side.as_deref() == Some("buy"));
    let (sell_trades, sell_members) = make_set("sell_trades", "Sell-aggressor trades", &ordered, |event| event.event_type == "trade" && event.side.as_deref() == Some("sell"));
    let (books, _) = make_set("book_events", "Order-book observations", &ordered, |event| event.event_type == "book");
    let (funding, funding_members) = make_set("funding_events", "Funding observations", &ordered, |event| matches!(event.event_type.as_str(), "funding" | "mark_price") && event.funding_rate.is_some());
    let (positive_funding, _) = make_set("positive_funding", "Positive funding observations", &ordered, |event| matches!(event.event_type.as_str(), "funding" | "mark_price") && event.funding_rate.is_some_and(|value| value > 0.0));
    let (negative_funding, _) = make_set("negative_funding", "Negative funding observations", &ordered, |event| matches!(event.event_type.as_str(), "funding" | "mark_price") && event.funding_rate.is_some_and(|value| value < 0.0));
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
        status: "derived_projection_with_append_only_records",
        persistence: "append_only_mongodb_records_for_ingested_and_backfilled_events",
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
            "The finite-set summary is derived on read; immutable observation and relation records are persisted separately.",
            "Historical raw events require the authenticated resumable backfill endpoint before durable history is complete.",
            "No V2 output changes the Version 1 vector decision or authorizes an order.",
        ],
    }
}


/// V2 decision: a deterministic empirical-measure model over the bounded event
/// universe. Counting measure and trade-size measure remain distinct; missing
/// sizes are excluded, not imputed. Temporal edges are evidence of ordering,
/// never interpreted as causal effects.
pub fn evaluate_measure_ledger_v2(symbol: &str, events: &[MarketEvent]) -> Result<serde_json::Value, String> {
    let ledger = build(symbol, events, 0, events.len(), false);
    let find = |id: &str| ledger.sets.iter().find(|set| set.set_id == id)
        .map(|set| set.measure.counting_measure as f64).unwrap_or(0.0);
    let buy_count=find("buy_trades");
    let sell_count=find("sell_trades");
    let trade_count=find("trades");
    let buy_volume=ledger.sets.iter().find(|set|set.set_id=="buy_trades").map(|set|set.measure.trade_size_measure).unwrap_or(0.0);
    let sell_volume=ledger.sets.iter().find(|set|set.set_id=="sell_trades").map(|set|set.measure.trade_size_measure).unwrap_or(0.0);
    let volume_observed=ledger.sets.iter().find(|set|set.set_id=="trades").map(|set|set.measure.trade_size_observed_count).unwrap_or(0);
    let volume_missing=ledger.sets.iter().find(|set|set.set_id=="trades").map(|set|set.measure.trade_size_missing_count).unwrap_or(0);
    let long_liq=find("long_liquidations");
    let short_liq=find("short_liquidations");
    let liquidation_count=long_liq+short_liq;
    let positive_funding=find("positive_funding");
    let negative_funding=find("negative_funding");
    let funding_count=positive_funding+negative_funding;
    let buy_relation=ledger.relations.iter().find(|r|r.relation_id=="buy_trades_before_liquidations").map(|r|r.edge_count as f64).unwrap_or(0.0);
    let sell_relation=ledger.relations.iter().find(|r|r.relation_id=="sell_trades_before_liquidations").map(|r|r.edge_count as f64).unwrap_or(0.0);
    let count_imbalance=if trade_count>0.0 {(buy_count-sell_count)/trade_count} else {0.0};
    let volume_imbalance=if buy_volume+sell_volume>0.0 {(buy_volume-sell_volume)/(buy_volume+sell_volume)} else {0.0};
    let liquidation_imbalance=if liquidation_count>0.0 {(short_liq-long_liq)/liquidation_count} else {0.0};
    let funding_imbalance=if funding_count>0.0 {(positive_funding-negative_funding)/funding_count} else {0.0};
    let relation_imbalance=if buy_relation+sell_relation>0.0 {(buy_relation-sell_relation)/(buy_relation+sell_relation)} else {0.0};
    // Renormalize over observed measures only. Missing dimensions do not become zero evidence.
    let mut weighted=0.0;
    let mut weight_total=0.0;
    let mut components=serde_json::Map::new();
    let mut add=|name:&str,value:f64,weight:f64| {
        if value.is_finite() {
            weighted+=value*weight; weight_total+=weight;
            components.insert(name.into(),serde_json::json!({"value":value,"weight":weight,"weighted_value":value*weight}));
        }
    };
    if trade_count>0.0 { add("counting_measure_trade_imbalance",count_imbalance,0.25); }
    if volume_observed>0 { add("trade_size_measure_imbalance",volume_imbalance,0.35); }
    if liquidation_count>0.0 { add("liquidation_counting_measure_imbalance",liquidation_imbalance,0.20); }
    if funding_count>0.0 { add("funding_measure_imbalance",funding_imbalance,0.10); }
    if buy_relation+sell_relation>0.0 { add("temporal_relation_measure_imbalance",relation_imbalance,0.10); }
    drop(add);
    let score=if weight_total>0.0 {weighted/weight_total} else {0.0};
    let latest=events.iter().max_by_key(|event|event.ts_ms);
    let latest_book=events.iter().filter(|event|event.event_type=="book"&&event.bid.is_some()&&event.ask.is_some()).max_by_key(|event|event.ts_ms);
    let spread_bps=latest_book.and_then(|event| {
        let bid=event.bid?; let ask=event.ask?; let mid=(bid+ask)/2.0;
        (bid.is_finite()&&ask.is_finite()&&mid>0.0&&ask>=bid).then_some((ask-bid)/mid*10_000.0)
    });
    let quote_age_ms=latest.zip(latest_book).map(|(last,book)|last.ts_ms.saturating_sub(book.ts_ms));
    let spread_ok=spread_bps.map(|spread|spread<=30.0).unwrap_or(true);
    let quote_fresh=quote_age_ms.map(|age|age<=30_000).unwrap_or(true);
    let sample_ready=trade_count>=10.0 && ledger.universe_count>=20 && weight_total>0.0;
    let action=if !sample_ready||!spread_ok||!quote_fresh {"wait"} else if score>=0.15 {"buy"} else if score<=-0.15 {"sell"} else {"wait"};
    let reason=if !sample_ready {"Insufficient finite event measure for V2; the algorithm abstains."}
        else if !spread_ok {"Observed spread exceeds the V2 risk gate; the algorithm abstains."}
        else if !quote_fresh {"The latest order-book observation is stale; the algorithm abstains."}
        else if action=="wait" {"The normalized empirical-measure score remains inside the neutral region."}
        else {"The normalized empirical-measure score crossed the V2 directional threshold."};
    let feature_values=serde_json::json!({
        "counting_measure_trade_imbalance":count_imbalance,
        "trade_size_measure_imbalance":if volume_observed>0 {Some(volume_imbalance)} else {None},
        "liquidation_counting_measure_imbalance":if liquidation_count>0.0 {Some(liquidation_imbalance)} else {None},
        "funding_measure_imbalance":if funding_count>0.0 {Some(funding_imbalance)} else {None},
        "temporal_relation_measure_imbalance":if buy_relation+sell_relation>0.0 {Some(relation_imbalance)} else {None}
    });
    Ok(serde_json::json!({
        "symbol":symbol,"strategy_key":"qoin-measure-ledger-v2","strategy_version":"measure-ledger-v2",
        "selected_algorithm_id":"qoin-measure-ledger-v2","selected_algorithm_version":"measure-ledger-v2",
        "model_version":"qoin-finite-measure-ledger-v2","action":action,"decision":action.to_uppercase(),
        "reason":reason,"score":score,"magnitude":score.abs(),"alignment":score.abs(),
        "decision_vector":[score.max(0.0),(-score).max(0.0)],
        "features":feature_values,"measure_components":components,"measure_weight_total":weight_total,
        "diagnostics":{"sample_ready":sample_ready,"trade_count":trade_count,"universe_count":ledger.universe_count,
            "spread_bps":spread_bps,"spread_ok":spread_ok,"quote_age_ms":quote_age_ms,"quote_fresh":quote_fresh,
            "threshold":0.15,"observed_trade_sizes":volume_observed,"missing_trade_sizes":volume_missing},
        "input_event":{"event_ts_ms":latest.map(|event|event.ts_ms),"event_id":latest.map(|event|event.event_id.clone()),
            "venue":latest.map(|event|event.venue.clone()),"state_event_count":ledger.universe_count},
        "ledger":ledger,"research_only":true
    }))
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
    fn v2_uses_empirical_count_and_trade_size_measures_and_abstains_on_small_samples() {
        let mut events=Vec::new();
        for index in 0..12 {
            let mut trade=event(&format!("buy-{index}"),"trade",1_000+index);
            trade.side=Some("buy".into()); trade.size=Some(2.0);
            events.push(trade);
        }
        for index in 0..12 {
            let mut trade=event(&format!("sell-{index}"),"trade",2_000+index);
            trade.side=Some("sell".into()); trade.size=Some(1.0);
            events.push(trade);
        }
        let decision=evaluate_measure_ledger_v2("BTCUSDT",&events).unwrap();
        assert_eq!(decision["strategy_key"],"qoin-measure-ledger-v2");
        assert_eq!(decision["action"],"buy");
        assert_eq!(decision["diagnostics"]["trade_count"].as_f64(),Some(24.0));
        assert!(decision["ledger"]["sets"].is_array());
        assert!(decision["measure_components"]["trade_size_measure_imbalance"].is_object());
        let small=evaluate_measure_ledger_v2("BTCUSDT",&events[..4]).unwrap();
        assert_eq!(small["action"],"wait");
        assert_eq!(small["diagnostics"]["sample_ready"],false);
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
    fn equal_event_timestamps_do_not_claim_temporal_precedence() {
        let mut buy = event("buy-same-time", "trade", 5_000);
        buy.side = Some("buy".into());
        let liquidation = event("liq-same-time", "liquidation", 5_000);
        let ledger = build("BTCUSDT", &[buy.clone(), liquidation.clone()], 0, 100, false);
        assert_eq!(ledger.relations[0].edge_count, 0);
        let records = durable_records(&liquidation, &[buy, liquidation.clone()], mongodb::bson::DateTime::now()).unwrap();
        assert_eq!(records.iter().filter(|record| record.get_str("record_type").ok() == Some("relation_edge")).count(), 0);
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
        assert_eq!(observation.get_str("schema_version").ok(), Some("qoin-relation-ledger-v2-alpha3"));
        assert_eq!(observation.get_str("source_event_hash_sha256").unwrap().len(), 64);
        let relation = records.iter().find(|record| record.get_str("record_type").ok() == Some("relation_edge")).unwrap();
        assert_eq!(relation.get_str("relation_id").ok(), Some("buy_trades_before_liquidations"));
        assert_eq!(relation.get_i64("lag_ms").ok(), Some(250));
        assert!(relation.get_str("interpretation").unwrap().contains("does not establish causation"));
    }

    #[test]
    fn durable_relation_edge_fanout_is_bounded_and_auditable() {
        let mut buy=event("bounded-buy","trade",10_000);
        buy.side=Some("buy".into());
        let mut neighbours=Vec::new();
        for index in 0..150 {
            neighbours.push(event(&format!("bounded-liq-{index}"),"liquidation",10_001+index));
        }
        let records=durable_records(&buy,&neighbours,mongodb::bson::DateTime::now()).unwrap();
        let observation=records.iter().find(|record|record.get_str("record_type").ok()==Some("observation")).unwrap();
        let edges=records.iter().filter(|record|record.get_str("record_type").ok()==Some("relation_edge")).count();
        assert_eq!(edges,100);
        assert_eq!(observation.get_bool("relation_edges_truncated").ok(),Some(true));
        assert_eq!(observation.get_document("relation_edge_candidate_counts").unwrap().get_i64("buy_trades_before_liquidations").ok(),Some(150));
        assert_eq!(observation.get_document("relation_edge_persisted_counts").unwrap().get_i64("buy_trades_before_liquidations").ok(),Some(100));
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
