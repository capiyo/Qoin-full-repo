//! Binance USD-M Futures adapter. Not wired to signals. Disabled unless two
//! explicit environment gates are set; this is not production risk control.
use hmac::{Hmac, Mac};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{env, time::{SystemTime, UNIX_EPOCH}};
use url::form_urlencoded;
type HmacSha256 = Hmac<Sha256>;

// Hard release gate: order orchestration still lacks exchange-side protective-stop
// confirmation, full reconciliation, and recovery. Only a reviewed code change may enable it.
const LIVE_EXECUTION_GATES_IMPLEMENTED: bool = false;

#[derive(Debug, Clone, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
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
}
#[derive(Clone)]
pub struct BinanceExecutor {
    client: Client, api_key: String, api_secret: String, base_url: String,
    max_notional_usdt: f64, max_quantity: f64,
}
impl BinanceExecutor {
    pub fn from_env() -> Result<Self, String> {
        if !LIVE_EXECUTION_GATES_IMPLEMENTED {
            return Err("live execution hard-disabled: protective-stop orchestration, exchange reconciliation, and crash recovery are not release-verified".into());
        }
        if env::var("QOIN_TRADING_MODE").unwrap_or_else(|_| "disabled".into()) != "live" {
            return Err("live execution disabled: QOIN_TRADING_MODE is not live".into());
        }
        if env::var("QOIN_LIVE_TRADING_CONFIRM").unwrap_or_default() != "I_UNDERSTAND_REAL_MONEY_RISK" {
            return Err("live execution requires explicit confirmation".into());
        }
        let api_key = env::var("BINANCE_API_KEY").map_err(|_| "BINANCE_API_KEY is missing".to_string())?;
        let api_secret = env::var("BINANCE_API_SECRET").map_err(|_| "BINANCE_API_SECRET is missing".to_string())?;
        if api_key.trim().is_empty() || api_secret.trim().is_empty() { return Err("API credentials must not be empty".into()); }
        let max_notional_usdt = env::var("QOIN_MAX_ORDER_NOTIONAL_USDT").unwrap_or_else(|_| "25".into()).parse::<f64>().map_err(|_| "notional cap must be numeric".to_string())?;
        let max_quantity = env::var("QOIN_MAX_ORDER_QUANTITY").unwrap_or_else(|_| "0.001".into()).parse::<f64>().map_err(|_| "quantity cap must be numeric".to_string())?;
        if !max_notional_usdt.is_finite() || max_notional_usdt <= 0.0 || !max_quantity.is_finite() || max_quantity <= 0.0 { return Err("order limits must be finite and positive".into()); }
        Ok(Self { client: Client::builder().timeout(std::time::Duration::from_secs(8)).build().map_err(|e| e.to_string())?,
            api_key, api_secret, base_url: env::var("BINANCE_FUTURES_BASE_URL").unwrap_or_else(|_| "https://fapi.binance.com".into()),
            max_notional_usdt, max_quantity })
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
    pub async fn place_market_order(&self, order: OrderRequest) -> Result<ExecutionResponse, String> {
        let symbol = order.symbol.trim().to_ascii_uppercase(); let side = order.side.trim().to_ascii_uppercase();
        if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric()) { return Err("symbol must contain only ASCII letters and digits".into()); }
        if !symbol.ends_with("USDT") { return Err("only USDT-quoted USD-M symbols are supported".into()); }
        if side != "BUY" && side != "SELL" { return Err("side must be BUY or SELL".into()); }
        if order.client_order_id.trim().len() < 8 || order.client_order_id.len() > 36 || !order.client_order_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("client_order_id must be 8-36 safe characters".into()); }
        if !order.quantity.is_finite() || order.quantity <= 0.0 || order.quantity > self.max_quantity { return Err(format!("quantity exceeds configured maximum {}", self.max_quantity)); }
        let mark_url = format!("{}/fapi/v1/premiumIndex?symbol={}", self.base_url.trim_end_matches('/'), symbol);
        let mark_response = self.client.get(mark_url).timeout(std::time::Duration::from_secs(5)).send().await.map_err(|e| format!("mark-price request failed: {e}"))?;
        if !mark_response.status().is_success() { return Err(format!("mark-price lookup failed with HTTP {}", mark_response.status().as_u16())); }
        let mark: serde_json::Value = mark_response.json().await.map_err(|e| e.to_string())?;
        let price = mark.get("markPrice").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0).ok_or_else(|| "invalid mark price".to_string())?;
        let notional = order.quantity * price;
        if notional > self.max_notional_usdt { return Err(format!("notional {:.4} USDT exceeds cap {:.4} USDT", notional, self.max_notional_usdt)); }
        let mut params = vec![("symbol".into(), symbol), ("side".into(), side), ("type".into(), "MARKET".into()),
            ("quantity".into(), format!("{:.12}", order.quantity).trim_end_matches('0').trim_end_matches('.').to_string()),
            ("newClientOrderId".into(), order.client_order_id), ("newOrderRespType".into(), "RESULT".into())];
        if order.reduce_only { params.push(("reduceOnly".into(), "true".into())); }
        let response = self.signed_request(Method::POST, "/fapi/v1/order", params).await?;
        Ok(ExecutionResponse { mode: "live", accepted: true, exchange: "binance_usdm_futures", response })
    }
}
