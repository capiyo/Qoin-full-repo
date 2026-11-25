//! Conservative classification of server-observed market-data freshness.
//!
//! This measures ingestion recency, not exchange completeness or provider uptime.

pub const DEFAULT_MAX_INGEST_AGE_MS: i64 = 5_000;
pub const MAX_INGEST_AGE_MS: i64 = 600_000;
pub const CLOCK_SKEW_TOLERANCE_MS: i64 = 5_000;

pub fn classify(
    now_ms: i64,
    received_at_ms: Option<i64>,
    event_ts_ms: Option<i64>,
    max_ingest_age_ms: i64,
) -> &'static str {
    if max_ingest_age_ms < 100 || max_ingest_age_ms > MAX_INGEST_AGE_MS || now_ms <= 0 {
        return "invalid_clock_or_threshold";
    }
    let (Some(received_at_ms), Some(event_ts_ms)) = (received_at_ms, event_ts_ms) else {
        return "unmeasured_legacy_rows";
    };
    if received_at_ms <= 0 || event_ts_ms <= 0 {
        return "invalid_timestamps";
    }
    if received_at_ms > now_ms.saturating_add(CLOCK_SKEW_TOLERANCE_MS) {
        return "server_clock_ahead";
    }
    if event_ts_ms > received_at_ms.saturating_add(CLOCK_SKEW_TOLERANCE_MS) {
        return "source_clock_ahead";
    }
    if now_ms.saturating_sub(received_at_ms) > max_ingest_age_ms {
        return "stale";
    }
    "fresh"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_and_stale_use_server_receipt_time() {
        assert_eq!(classify(10_000, Some(9_000), Some(8_500), 5_000), "fresh");
        assert_eq!(classify(20_000, Some(9_000), Some(8_500), 5_000), "stale");
    }

    #[test]
    fn legacy_rows_are_not_assumed_fresh() {
        assert_eq!(classify(10_000, None, Some(9_000), 5_000), "unmeasured_legacy_rows");
        assert_eq!(classify(10_000, Some(9_000), None, 5_000), "unmeasured_legacy_rows");
    }

    #[test]
    fn clock_skew_is_reported_instead_of_marked_fresh() {
        assert_eq!(classify(10_000, Some(20_000), Some(9_000), 5_000), "server_clock_ahead");
        assert_eq!(classify(10_000, Some(9_000), Some(20_000), 5_000), "source_clock_ahead");
    }

    #[test]
    fn invalid_timestamps_and_thresholds_fail_closed() {
        assert_eq!(classify(10_000, Some(0), Some(9_000), 5_000), "invalid_timestamps");
        assert_eq!(classify(10_000, Some(9_000), Some(8_000), 50), "invalid_clock_or_threshold");
        assert_eq!(classify(0, Some(9_000), Some(8_000), 5_000), "invalid_clock_or_threshold");
    }
}
