//! Bounded Binance USD-M aggregate-trade history parsing and backfill limits.
//!
//! This is a market-data adapter only. It does not implement or authorize broker
//! execution. Historical rows are stored separately from the live rolling-state
//! recovery path so importing old observations cannot rewrite live V1 state.

use serde::Deserialize;
use serde_json::Value;

pub const MAX_PAGE_LIMIT: u16 = 1_000;
pub const MAX_PAGES_PER_CALL: u8 = 10;
pub const MAX_TOTAL_PAGES_PER_JOB: i64 = 1_000;
pub const MAX_EVENTS_PER_JOB: i64 = 1_000_000;
pub const LIVE_OVERLAP_GUARD_MS: i64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceBackfillRequest {
    #[serde(default)]
    pub job_id: Option<String>,
    pub symbol: String,
    pub from_ts_ms: i64,
    pub to_ts_ms: i64,
    #[serde(default)]
    pub page_limit: Option<u16>,
    #[serde(default)]
    pub max_pages: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBackfillRequest {
    pub job_id: Option<String>,
    pub symbol: String,
    pub from_ts_ms: i64,
    pub to_ts_ms: i64,
    pub page_limit: u16,
    pub max_pages: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinanceAggregateTrade {
    pub aggregate_trade_id: u64,
    pub event_id: String,
    pub symbol: String,
    pub ts_ms: i64,
    pub price: f64,
    pub size: f64,
    pub side: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAggregateTradePage {
    pub trades: Vec<BinanceAggregateTrade>,
    pub rows_seen: usize,
    pub first_aggregate_trade_id: Option<u64>,
    pub first_ts_ms: Option<i64>,
    pub last_aggregate_trade_id: Option<u64>,
    pub last_ts_ms: Option<i64>,
    pub next_from_id: Option<u64>,
    pub has_more: bool,
}

impl BinanceBackfillRequest {
    pub fn validate(self, now_ms: i64) -> Result<ValidatedBackfillRequest, String> {
        let symbol = self.symbol.trim().to_ascii_uppercase();
        if symbol.is_empty()
            || symbol.len() > 32
            || !symbol.bytes().all(|value| value.is_ascii_alphanumeric())
        {
            return Err("symbol must be a 1 to 32 character Binance USD-M symbol".into());
        }
        if self.from_ts_ms <= 0 || self.to_ts_ms < self.from_ts_ms {
            return Err("from_ts_ms and to_ts_ms must define a positive, ordered range".into());
        }
        if now_ms <= 0 || self.to_ts_ms > now_ms.saturating_sub(LIVE_OVERLAP_GUARD_MS) {
            return Err("to_ts_ms must be at least five minutes behind server time to avoid overlapping the live stream".into());
        }
        let page_limit = self.page_limit.unwrap_or(MAX_PAGE_LIMIT);
        if page_limit == 0 || page_limit > MAX_PAGE_LIMIT {
            return Err(format!("page_limit must be between 1 and {MAX_PAGE_LIMIT}"));
        }
        let max_pages = self.max_pages.unwrap_or(5);
        if max_pages == 0 || max_pages > MAX_PAGES_PER_CALL {
            return Err(format!("max_pages must be between 1 and {MAX_PAGES_PER_CALL}"));
        }
        let job_id = self.job_id.map(|value| value.trim().to_string());
        if job_id.as_ref().is_some_and(|value| {
            value.is_empty()
                || value.len() > 64
                || !value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
                })
        }) {
            return Err("job_id must contain 1 to 64 ASCII letters, digits, '-' or '_'".into());
        }
        Ok(ValidatedBackfillRequest {
            job_id,
            symbol,
            from_ts_ms: self.from_ts_ms,
            to_ts_ms: self.to_ts_ms,
            page_limit,
            max_pages,
        })
    }
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|item| {
        item.as_i64().or_else(|| {
            item.as_u64()
                .and_then(|number| i64::try_from(number).ok())
                .or_else(|| item.as_str().and_then(|text| text.parse::<i64>().ok()))
        })
    })
}

fn value_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|item| {
        item.as_f64()
            .or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
            .filter(|number| number.is_finite())
    })
}

/// Parse one provider page. Binance aggregate trade IDs and transaction times
/// must be strictly/non-decreasing respectively; malformed or reordered pages
/// fail closed rather than advancing a resumable cursor over missing evidence.
pub fn parse_aggregate_trade_page(
    symbol: &str,
    rows: &[Value],
    from_ts_ms: i64,
    to_ts_ms: i64,
    page_limit: u16,
) -> Result<ParsedAggregateTradePage, String> {
    if symbol.trim().is_empty()
        || from_ts_ms <= 0
        || to_ts_ms < from_ts_ms
        || page_limit == 0
        || page_limit > MAX_PAGE_LIMIT
        || rows.len() > usize::from(page_limit)
    {
        return Err("invalid aggregate-trade page context".into());
    }

    let mut trades = Vec::with_capacity(rows.len());
    let mut previous_id: Option<u64> = None;
    let mut previous_ts: Option<i64> = None;
    let mut first_aggregate_trade_id = None;
    let mut first_ts_ms = None;
    let mut last_aggregate_trade_id = None;
    let mut last_ts_ms = None;
    let mut beyond_range = false;

    for (index, row) in rows.iter().enumerate() {
        let id = value_u64(row.get("a"))
            .ok_or_else(|| format!("aggregate-trade row {index} has an invalid ID"))?;
        let ts_ms = value_i64(row.get("T"))
            .ok_or_else(|| format!("aggregate-trade row {index} has an invalid transaction time"))?;
        if id == 0 || ts_ms <= 0 {
            return Err(format!("aggregate-trade row {index} has a non-positive ID or timestamp"));
        }
        if previous_id.is_some_and(|previous| id <= previous) {
            return Err("aggregate-trade IDs must be strictly increasing within each page".into());
        }
        if previous_ts.is_some_and(|previous| ts_ms < previous) {
            return Err("aggregate-trade timestamps must be non-decreasing within each page".into());
        }
        previous_id = Some(id);
        previous_ts = Some(ts_ms);
        first_aggregate_trade_id.get_or_insert(id);
        first_ts_ms.get_or_insert(ts_ms);
        last_aggregate_trade_id = Some(id);
        last_ts_ms = Some(ts_ms);

        if ts_ms > to_ts_ms {
            beyond_range = true;
            continue;
        }
        if ts_ms < from_ts_ms {
            continue;
        }

        let price = value_f64(row.get("p"))
            .filter(|value| *value > 0.0)
            .ok_or_else(|| format!("aggregate-trade row {index} has an invalid price"))?;
        let size = value_f64(row.get("q"))
            .filter(|value| *value > 0.0)
            .ok_or_else(|| format!("aggregate-trade row {index} has an invalid quantity"))?;
        let buyer_is_maker = row.get("m").and_then(Value::as_bool)
            .ok_or_else(|| format!("aggregate-trade row {index} is missing buyer-maker side metadata"))?;

        trades.push(BinanceAggregateTrade {
            aggregate_trade_id: id,
            event_id: format!("agg_trade:{id}"),
            symbol: symbol.to_ascii_uppercase(),
            ts_ms,
            price,
            size,
            // If the buyer is maker, the aggressive/taker side was sell.
            side: if buyer_is_maker { "sell" } else { "buy" },
        });
    }

    // Continue when a full page ends exactly at the requested upper timestamp:
    // more aggregate trades may share that millisecond.
    let has_more = !beyond_range
        && rows.len() == usize::from(page_limit)
        && last_ts_ms.is_some_and(|timestamp| timestamp <= to_ts_ms);
    let next_from_id = if has_more {
        Some(last_aggregate_trade_id
            .ok_or_else(|| "full aggregate-trade page has no final ID".to_string())?
            .checked_add(1)
            .ok_or_else(|| "aggregate-trade cursor overflow".to_string())?)
    } else {
        None
    };

    Ok(ParsedAggregateTradePage {
        trades,
        rows_seen: rows.len(),
        first_aggregate_trade_id,
        first_ts_ms,
        last_aggregate_trade_id,
        last_ts_ms,
        next_from_id,
        has_more,
    })
}

/// Validate continuity against the persisted cursor from the preceding page.
/// Provider IDs must not move backwards and event time must remain monotonic
/// across page boundaries; otherwise a repeated or reordered response could
/// be checkpointed as progress and silently corrupt a resumable import.
pub fn validate_page_continuation(
    page: &ParsedAggregateTradePage,
    expected_from_id: Option<u64>,
    previous_ts_ms: Option<i64>,
) -> Result<(), String> {
    if let (Some(expected), Some(first)) = (expected_from_id, page.first_aggregate_trade_id) {
        if first < expected {
            return Err("aggregate-trade page begins before the persisted fromId cursor".into());
        }
    }
    if let (Some(previous), Some(first)) = (previous_ts_ms, page.first_ts_ms) {
        if first < previous {
            return Err("aggregate-trade timestamps moved backwards across pages".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: u64, ts: i64, price: &str, quantity: &str, buyer_is_maker: bool) -> Value {
        json!({"a": id, "T": ts, "p": price, "q": quantity, "m": buyer_is_maker})
    }

    #[test]
    fn request_normalizes_symbol_and_applies_bounded_defaults() {
        let request = BinanceBackfillRequest {
            job_id: Some("job-123".into()),
            symbol: " btcusdt ".into(),
            from_ts_ms: 1_700_000_000_000,
            to_ts_ms: 1_700_000_100_000,
            page_limit: None,
            max_pages: None,
        }.validate(1_700_001_000_000).unwrap();
        assert_eq!(request.symbol, "BTCUSDT");
        assert_eq!(request.page_limit, 1_000);
        assert_eq!(request.max_pages, 5);
        assert_eq!(request.job_id.as_deref(), Some("job-123"));
    }

    #[test]
    fn request_rejects_recent_upper_bound_and_unsafe_pagination() {
        let base = BinanceBackfillRequest {
            job_id: None,
            symbol: "BTCUSDT".into(),
            from_ts_ms: 1_700_000_000_000,
            to_ts_ms: 1_700_000_100_000,
            page_limit: Some(1_001),
            max_pages: Some(1),
        };
        assert!(base.clone().validate(1_700_001_000_000).is_err());
        let mut recent = base.clone();
        recent.page_limit = Some(1);
        assert!(recent.validate(1_700_000_200_000).is_err());
        let mut unsafe_job = base;
        unsafe_job.page_limit = Some(1);
        unsafe_job.job_id = Some("../secret".into());
        assert!(unsafe_job.validate(1_700_001_000_000).is_err());
    }

    #[test]
    fn parses_buyer_maker_side_and_stable_aggregate_ids() {
        let page = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(10, 1_700_000_000_000, "100.5", "0.25", true),
              row(11, 1_700_000_000_000, "100.6", "0.5", false)],
            1_700_000_000_000,
            1_700_000_000_000,
            2,
        ).unwrap();
        assert_eq!(page.first_aggregate_trade_id, Some(10));
        assert_eq!(page.first_ts_ms, Some(1_700_000_000_000));
        assert_eq!(page.trades[0].event_id, "agg_trade:10");
        assert_eq!(page.trades[0].side, "sell");
        assert_eq!(page.trades[1].side, "buy");
        assert_eq!(page.next_from_id, Some(12));
        assert!(page.has_more);
    }

    #[test]
    fn a_full_page_continues_across_equal_upper_bound_timestamps() {
        let page = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(20, 1_700_000_000_000, "100", "1", false),
              row(21, 1_700_000_000_000, "101", "1", true)],
            1_700_000_000_000,
            1_700_000_000_000,
            2,
        ).unwrap();
        assert_eq!(page.next_from_id, Some(22));
        assert!(page.has_more);
    }

    #[test]
    fn page_stops_when_provider_has_passed_requested_upper_bound() {
        let page = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(30, 1_700_000_000_000, "100", "1", false),
              row(31, 1_700_000_000_001, "101", "1", true)],
            1_700_000_000_000,
            1_700_000_000_000,
            2,
        ).unwrap();
        assert_eq!(page.trades.len(), 1);
        assert!(!page.has_more);
        assert_eq!(page.next_from_id, None);
    }


    #[test]
    fn continuation_rejects_replayed_ids_and_backwards_timestamps() {
        let replayed = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(10, 1_700_000_000_000, "100", "1", false),
              row(11, 1_700_000_000_001, "101", "1", true)],
            1_700_000_000_000,
            1_700_000_100_000,
            2,
        ).unwrap();
        assert!(validate_page_continuation(&replayed, Some(12), Some(1_700_000_000_001)).is_err());

        let time_reversed = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(12, 1_700_000_000_000, "102", "1", false)],
            1_700_000_000_000,
            1_700_000_100_000,
            2,
        ).unwrap();
        assert!(validate_page_continuation(&time_reversed, Some(12), Some(1_700_000_000_001)).is_err());
    }

    #[test]
    fn continuation_accepts_forward_ids_and_equal_timestamps() {
        let next = parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(13, 1_700_000_000_001, "102", "1", false),
              row(14, 1_700_000_000_001, "103", "1", true)],
            1_700_000_000_000,
            1_700_000_100_000,
            2,
        ).unwrap();
        assert!(validate_page_continuation(&next, Some(12), Some(1_700_000_000_001)).is_ok());
    }

    #[test]
    fn rejects_reordered_duplicate_and_malformed_provider_rows() {
        assert!(parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(2, 1_700_000_000_001, "100", "1", false),
              row(1, 1_700_000_000_002, "101", "1", true)],
            1_700_000_000_000,
            1_700_000_000_010,
            2,
        ).is_err());
        assert!(parse_aggregate_trade_page(
            "BTCUSDT",
            &[row(1, 1_700_000_000_000, "NaN", "1", false)],
            1_700_000_000_000,
            1_700_000_000_010,
            1,
        ).is_err());
    }
}
