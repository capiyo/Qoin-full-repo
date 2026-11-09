//! Binance USD-M Futures adapter. Not wired to signals. Disabled unless two
//! explicit environment gates are set; this is not production risk control.
use hmac::{Hmac, Mac};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, time::{SystemTime, UNIX_EPOCH}};
use url::{form_urlencoded, Url};
type HmacSha256 = Hmac<Sha256>;

pub fn protective_stop_client_order_id(client_order_id: &str) -> String {
    let digest = Sha256::digest(client_order_id.as_bytes());
    format!("qsl-{}", hex::encode(&digest[..8]))
}

fn emergency_close_client_order_id(client_order_id: &str) -> String {
    let digest = Sha256::digest(client_order_id.as_bytes());
    format!("qfc-{}", hex::encode(&digest[..8]))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    /// Required for opening orders; omitted for reduce-only closes.
    #[serde(default)]
    pub stop_price: f64,
    #[serde(default)]
    pub reduce_only: bool,
    pub client_order_id: String,
}
#[derive(Debug, Serialize)]
pub struct ExecutionResponse {
    pub mode: &'static str,
    pub accepted: bool,
    pub exchange: &'static str,
    pub response: serde_json::Value,
    pub protective_stop: serde_json::Value,
}
#[derive(Clone)]
pub struct BinanceExecutor {
    client: Client, api_key: String, api_secret: String, base_url: String,
    max_notional_usdt: f64, max_quantity: f64, mode: &'static str,
}
impl BinanceExecutor {
    pub fn from_env() -> Result<Self, String> {
        let configured_mode = env::var("QOIN_TRADING_MODE").unwrap_or_else(|_| "disabled".into());
        let mode = match configured_mode.as_str() {
            "live" => {
                if env::var("QOIN_LIVE_TRADING_CONFIRM").unwrap_or_default() != "I_UNDERSTAND_REAL_MONEY_RISK" {
                    return Err("live execution requires explicit confirmation".into());
                }
                "live"
            }
            "testnet" => {
                if env::var("QOIN_TESTNET_TRADING_CONFIRM").unwrap_or_default() != "I_UNDERSTAND_TESTNET_ORDERS" {
                    return Err("testnet execution requires explicit confirmation".into());
                }
                "testnet"
            }
            _ => return Err("execution disabled: set QOIN_TRADING_MODE to testnet or live".into()),
        };
        let api_key = env::var("BINANCE_API_KEY").map_err(|_| "BINANCE_API_KEY is missing".to_string())?;
        let api_secret = env::var("BINANCE_API_SECRET").map_err(|_| "BINANCE_API_SECRET is missing".to_string())?;
        if api_key.trim().is_empty() || api_secret.trim().is_empty() { return Err("API credentials must not be empty".into()); }
        let max_notional_usdt = env::var("QOIN_MAX_ORDER_NOTIONAL_USDT").unwrap_or_else(|_| "25".into()).parse::<f64>().map_err(|_| "notional cap must be numeric".to_string())?;
        let max_quantity = env::var("QOIN_MAX_ORDER_QUANTITY").unwrap_or_else(|_| "0.001".into()).parse::<f64>().map_err(|_| "quantity cap must be numeric".to_string())?;
        if !max_notional_usdt.is_finite() || max_notional_usdt <= 0.0 || !max_quantity.is_finite() || max_quantity <= 0.0 { return Err("order limits must be finite and positive".into()); }
        let default_base_url = if mode == "testnet" { "https://testnet.binancefuture.com" } else { "https://fapi.binance.com" };
        let base_url = env::var("BINANCE_FUTURES_BASE_URL").ok()
            .filter(|value| !value.trim().is_empty()).unwrap_or_else(|| default_base_url.into());
        let parsed_base_url = Url::parse(&base_url).map_err(|_| "BINANCE_FUTURES_BASE_URL must be a valid HTTPS URL".to_string())?;
        if parsed_base_url.scheme() != "https" || parsed_base_url.host_str().is_none() {
            return Err("BINANCE_FUTURES_BASE_URL must use HTTPS and include a host".into());
        }
        let expected_host = if mode == "testnet" { "testnet.binancefuture.com" } else { "fapi.binance.com" };
        if parsed_base_url.host_str() != Some(expected_host) {
            return Err(format!("{} mode must use https://{}", mode, expected_host));
        }
        Ok(Self { client: Client::builder().timeout(std::time::Duration::from_secs(8)).build().map_err(|e| e.to_string())?,
            api_key, api_secret, base_url: base_url.trim_end_matches('/').to_string(),
            max_notional_usdt, max_quantity, mode })
    }
    fn sign(&self, query: &str) -> Result<String, String> {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).map_err(|e| e.to_string())?;
        mac.update(query.as_bytes()); Ok(hex::encode(mac.finalize().into_bytes()))
    }
    async fn signed_request(&self, method: Method, path: &str, mut params: Vec<(String, String)>) -> Result<serde_json::Value, String> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis();
        params.push(("timestamp".into(), timestamp.to_string())); params.push(("recvWindow".into(), "5000".into()));
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        for (k, v) in params { serializer.append_pair(&k, &v); }
        let query = serializer.finish(); let signature = self.sign(&query)?;
        let url = format!("{}{}?{}&signature={}", self.base_url.trim_end_matches('/'), path, query, signature);
        let response = self.client.request(method, url).header("X-MBX-APIKEY", &self.api_key).send().await.map_err(|e| format!("Binance request failed: {e}"))?;
        let status = response.status(); let body = response.json::<serde_json::Value>().await.map_err(|e| format!("Binance response was not JSON: {e}"))?;
        if !status.is_success() { return Err(format!("Binance rejected request (HTTP {}): {}", status.as_u16(), body)); }
        Ok(body)
    }
    pub async fn get_order_by_client_id(&self, symbol: &str, client_order_id: &str) -> Result<serde_json::Value, String> {
        let symbol = symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("symbol must contain only ASCII letters and digits".into());
        }
        if client_order_id.trim().is_empty() || client_order_id.len() > 36
            || !client_order_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err("client_order_id is invalid".into());
        }
        self.signed_request(Method::GET, "/fapi/v1/order", vec![
            ("symbol".into(), symbol),
            ("origClientOrderId".into(), client_order_id.to_string()),
        ]).await
    }

    pub async fn place_market_order(&self, order: OrderRequest) -> Result<ExecutionResponse, String> {
        let symbol = order.symbol.trim().to_ascii_uppercase(); let side = order.side.trim().to_ascii_uppercase();
        if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric()) { return Err("symbol must contain only ASCII letters and digits".into()); }
        if !symbol.ends_with("USDT") { return Err("only USDT-quoted USD-M symbols are supported".into()); }
        if side != "BUY" && side != "SELL" { return Err("side must be BUY or SELL".into()); }
        if order.client_order_id.trim().len() < 8 || order.client_order_id.len() > 36 || !order.client_order_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("client_order_id must be 8-36 safe characters".into()); }
        if !order.quantity.is_finite() || order.quantity <= 0.0 || order.quantity > self.max_quantity { return Err(format!("quantity exceeds configured maximum {}", self.max_quantity)); }
        if !order.reduce_only && (!order.stop_price.is_finite() || order.stop_price <= 0.0) { return Err("stop_price must be finite and positive".into()); }
        let mark_url = format!("{}/fapi/v1/premiumIndex?symbol={}", self.base_url.trim_end_matches('/'), symbol);
        let mark_response = self.client.get(mark_url).timeout(std::time::Duration::from_secs(5)).send().await.map_err(|e| format!("mark-price request failed: {e}"))?;
        if !mark_response.status().is_success() { return Err(format!("mark-price lookup failed with HTTP {}", mark_response.status().as_u16())); }
        let mark: serde_json::Value = mark_response.json().await.map_err(|e| e.to_string())?;
        let price = mark.get("markPrice").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0).ok_or_else(|| "invalid mark price".to_string())?;
        if !order.reduce_only {
            let stop_is_valid = if side == "BUY" { order.stop_price < price } else { order.stop_price > price };
            if !stop_is_valid { return Err("protective stop must be below mark for BUY and above mark for SELL".into()); }
        }
        let notional = order.quantity * price;
        if !notional.is_finite() || notional > self.max_notional_usdt { return Err(format!("notional {:.4} USDT exceeds cap {:.4} USDT", notional, self.max_notional_usdt)); }
        let quantity = format!("{:.12}", order.quantity).trim_end_matches('0').trim_end_matches('.').to_string();
        let mut params = vec![("symbol".into(), symbol.clone()), ("side".into(), side.clone()), ("type".into(), "MARKET".into()),
            ("quantity".into(), quantity.clone()),
            ("newClientOrderId".into(), order.client_order_id.clone()), ("newOrderRespType".into(), "RESULT".into())];
        if order.reduce_only { params.push(("reduceOnly".into(), "true".into())); }
        let response = self.signed_request(Method::POST, "/fapi/v1/order", params).await?;
        let executed_quantity = response.get("executedQty").and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| "exchange accepted entry but returned no valid executed quantity; reconcile by client order ID".to_string())?;
        let executed_quantity_text = format!("{:.12}", executed_quantity).trim_end_matches('0').trim_end_matches('.').to_string();
        if order.reduce_only {
            return Ok(ExecutionResponse {
                mode: self.mode, accepted: true, exchange: "binance_usdm_futures",
                response, protective_stop: serde_json::Value::Null,
            });
        }
        let stop_side = if side == "BUY" { "SELL" } else { "BUY" };
        let stop_params = vec![
            ("symbol".into(), symbol.clone()), ("side".into(), stop_side.into()),
            ("type".into(), "STOP_MARKET".into()),
            ("stopPrice".into(), format!("{:.12}", order.stop_price).trim_end_matches('0').trim_end_matches('.').to_string()),
            ("quantity".into(), executed_quantity_text.clone()), ("reduceOnly".into(), "true".into()),
            ("workingType".into(), "MARK_PRICE".into()),
            ("newClientOrderId".into(), protective_stop_client_order_id(&order.client_order_id)),
            ("newOrderRespType".into(), "RESULT".into()),
        ];
        let protective_stop = match self.signed_request(Method::POST, "/fapi/v1/order", stop_params).await {
            Ok(stop) => stop,
            Err(stop_error) => {
                // Never leave a newly opened position intentionally unprotected.
                // Attempt an immediate reduce-only close for the filled quantity.
                let close_params = vec![
                    ("symbol".into(), symbol.clone()), ("side".into(), stop_side.into()),
                    ("type".into(), "MARKET".into()), ("quantity".into(), executed_quantity_text),
                    ("reduceOnly".into(), "true".into()),
                    ("newClientOrderId".into(), emergency_close_client_order_id(&order.client_order_id)),
                    ("newOrderRespType".into(), "RESULT".into()),
                ];
                return match self.signed_request(Method::POST, "/fapi/v1/order", close_params).await {
                    Ok(close) => Err(format!("protective stop rejected; reduce-only emergency close submitted: {close} (stop error: {stop_error})")),
                    Err(close_error) => Err(format!("CRITICAL: protective stop rejected and emergency close failed; immediate manual reconciliation required (stop error: {stop_error}; close error: {close_error})")),
                };
            }
        };
        Ok(ExecutionResponse { mode: self.mode, accepted: true, exchange: "binance_usdm_futures", response, protective_stop })
    }
}


#[cfg(test)]
mod tests {
    use super::{emergency_close_client_order_id, protective_stop_client_order_id};

    #[test]
    fn child_order_ids_are_stable_bounded_and_distinct() {
        let parent = "qoin-entry-order-1234567890";
        let stop = protective_stop_client_order_id(parent);
        let close = emergency_close_client_order_id(parent);
        assert_eq!(stop, protective_stop_client_order_id(parent));
        assert_eq!(close, emergency_close_client_order_id(parent));
        assert_ne!(stop, close);
        assert!(stop.len() <= 36);
        assert!(close.len() <= 36);
        assert_ne!(stop, protective_stop_client_order_id("qoin-entry-order-1234567891"));
    }
}
