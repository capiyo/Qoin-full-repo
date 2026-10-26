//! Optional, loopback-only JEPA research sidecar client.
//!
//! Predictions are stored as research observations only. They never replace
//! the Version 1 vector decision or authorize an order.

use std::{
    collections::VecDeque,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::SymbolState;

pub const FEATURE_NAMES: &[&str] = &[
    "order_flow_imbalance",
    "spread_bps_value",
    "spread_bps_missing",
    "ewma_abs_return",
    "funding_rate_value",
    "funding_rate_missing",
    "open_interest_value",
    "open_interest_missing",
    "event_intensity",
    "liquidation_events",
    "state_quality",
];

#[derive(Clone)]
pub struct JepaClient {
    http: reqwest::Client,
    base_url: String,
    context_steps: usize,
    min_interval_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JepaPrediction {
    pub model_version: String,
    pub horizon_steps: usize,
    pub feature_names: Vec<String>,
    pub predicted_latent_state: Vec<f64>,
    pub decision: Option<serde_json::Value>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JepaPredictionRecord {
    pub symbol: String,
    pub event_ts_ms: i64,
    pub predicted_at_ms: i64,
    pub model_version: String,
    pub horizon_steps: usize,
    pub feature_names: Vec<String>,
    pub predicted_latent_state: Vec<f64>,
    pub note: String,
}

#[derive(Default)]
pub struct FeatureHistory {
    rows: VecDeque<(i64, Vec<f64>)>,
    last_prediction_ts_ms: Option<i64>,
}

impl FeatureHistory {
    /// Add a strictly newer observation and return a bounded context when due.
    pub fn push_if_due(
        &mut self,
        ts_ms: i64,
        features: Vec<f64>,
        context_steps: usize,
        min_interval_ms: i64,
    ) -> Option<Vec<Vec<f64>>> {
        if context_steps == 0 || ts_ms <= 0 || features.len() != FEATURE_NAMES.len()
            || features.iter().any(|value| !value.is_finite())
        {
            return None;
        }
        if self.rows.back().is_some_and(|(last_ts, _)| ts_ms <= *last_ts) {
            return None;
        }
        self.rows.push_back((ts_ms, features));
        while self.rows.len() > context_steps {
            self.rows.pop_front();
        }
        if self.rows.len() < context_steps {
            return None;
        }
        if self.last_prediction_ts_ms
            .is_some_and(|last| ts_ms.saturating_sub(last) < min_interval_ms)
        {
            return None;
        }
        self.last_prediction_ts_ms = Some(ts_ms);
        Some(self.rows.iter().map(|(_, row)| row.clone()).collect())
    }
}

pub fn features_from_state(state: &SymbolState) -> Vec<f64> {
    let spread_missing = state.spread_bps.is_none();
    let funding_missing = state.funding_rate.is_none();
    let open_interest_missing = state.open_interest.is_none();
    vec![
        state.order_flow_imbalance,
        state.spread_bps.unwrap_or(0.0),
        if spread_missing { 1.0 } else { 0.0 },
        state.ewma_abs_return,
        state.funding_rate.unwrap_or(0.0),
        if funding_missing { 1.0 } else { 0.0 },
        state.open_interest.unwrap_or(0.0),
        if open_interest_missing { 1.0 } else { 0.0 },
        state.event_intensity,
        state.liquidation_events as f64,
        state.state_quality,
    ]
}

impl JepaClient {
    pub fn new(base_url: &str, context_steps: usize, min_interval_ms: i64) -> Result<Self, String> {
        if context_steps == 0 || context_steps > 4096 {
            return Err("QOIN_JEPA_CONTEXT_STEPS must be between 1 and 4096".into());
        }
        if min_interval_ms < 0 {
            return Err("QOIN_JEPA_MIN_INTERVAL_MS must be non-negative".into());
        }
        let parsed = Url::parse(base_url).map_err(|_| "QOIN_JEPA_URL is not a valid URL")?;
        let local_host = matches!(
            parsed.host_str(),
            Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
        );
        if parsed.scheme() != "http" || !local_host {
            return Err("JEPA research service must use HTTP on loopback only".into());
        }
        let normalized_url = base_url.trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(250))
            .build()
            .map_err(|_| "could not initialize JEPA HTTP client")?;
        Ok(Self {
            http,
            base_url: normalized_url,
            context_steps,
            min_interval_ms,
        })
    }

    pub fn context_steps(&self) -> usize {
        self.context_steps
    }

    pub fn min_interval_ms(&self) -> i64 {
        self.min_interval_ms
    }

    pub async fn predict(&self, rows: &[Vec<f64>]) -> Result<JepaPrediction, String> {
        if rows.len() != self.context_steps {
            return Err(format!("expected {} rows, received {}", self.context_steps, rows.len()));
        }
        if rows.iter().any(|row| {
            row.len() != FEATURE_NAMES.len() || row.iter().any(|value| !value.is_finite())
        }) {
            return Err("JEPA input feature width or numeric quality is invalid".into());
        }
        let response = self.http
            .post(format!("{}/predict", self.base_url))
            .json(&serde_json::json!({
                "feature_names": FEATURE_NAMES,
                "rows": rows,
                "horizon": 1
            }))
            .send()
            .await
            .map_err(|error| format!("JEPA service request failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("JEPA service returned an error: {error}"))?;
        let prediction: JepaPrediction = response
            .json()
            .await
            .map_err(|error| format!("JEPA response schema invalid: {error}"))?;
        if prediction.feature_names.iter().map(String::as_str).collect::<Vec<_>>() != FEATURE_NAMES
            || prediction.predicted_latent_state.is_empty()
            || prediction.predicted_latent_state.iter().any(|value| !value.is_finite())
        {
            return Err("JEPA response feature schema or latent output is invalid".into());
        }
        if prediction.decision.is_some() {
            return Err("JEPA service must return latent predictions only, never decisions".into());
        }
        Ok(prediction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(value: f64) -> Vec<f64> {
        let mut row = vec![value; FEATURE_NAMES.len()];
        row[2] = 0.0;
        row[5] = 1.0;
        row[7] = 1.0;
        row
    }

    #[test]
    fn history_waits_for_full_context_and_rate_limits() {
        let mut history = FeatureHistory::default();
        assert!(history.push_if_due(1000, features(1.0), 2, 500).is_none());
        let first = history.push_if_due(1100, features(2.0), 2, 500);
        assert_eq!(first.as_ref().map(Vec::len), Some(2));
        assert!(history.push_if_due(1200, features(3.0), 2, 500).is_none());
        assert!(history.push_if_due(1150, features(4.0), 2, 500).is_none());
        let next = history.push_if_due(1700, features(5.0), 2, 500);
        assert_eq!(next.as_ref().map(Vec::len), Some(2));
        assert_eq!(next.expect("due context")[1][0], 5.0);
    }

    #[test]
    fn history_rejects_invalid_feature_rows() {
        let mut history = FeatureHistory::default();
        let mut row = features(1.0);
        row[0] = f64::NAN;
        assert!(history.push_if_due(1000, row, 1, 0).is_none());
        assert!(history.push_if_due(1000, vec![1.0], 1, 0).is_none());
    }

    #[test]
    fn client_rejects_non_loopback_service_urls() {
        assert!(JepaClient::new("http://example.com:8765", 32, 1000).is_err());
        assert!(JepaClient::new("https://127.0.0.1:8765", 32, 1000).is_err());
        assert!(JepaClient::new("http://127.0.0.1:8765", 32, 1000).is_ok());
    }
}
