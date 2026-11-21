//! Server-side broker credentials loaded exclusively from deployment environment secrets.
//! No credential value is serializable or returned by an HTTP handler.
use std::{collections::{BTreeMap, HashSet}, env, net::IpAddr};
use serde::Serialize;
use zeroize::Zeroize;

const PROVIDERS: &[&str] = &[
    "binance_usdm", "coinbase_advanced_trade", "kraken_spot", "bybit", "okx",
    "bitget", "gate_io", "kucoin", "deribit", "hyperliquid", "oanda", "ig",
    "fxcm", "capital_com", "deriv", "xtb", "pepperstone", "ic_markets",
    "forex_com", "cmc_markets",
];

#[derive(Default)]
struct Secret(String);

impl Secret {
    fn expose(&self) -> &str { &self.0 }
}
impl Drop for Secret {
    fn drop(&mut self) { self.0.zeroize(); }
}

pub struct BrokerCredentials {
    api_key: Option<Secret>,
    api_secret: Option<Secret>,
    api_token: Option<Secret>,
    passphrase: Option<Secret>,
    account_ids: Vec<String>,
    base_url: Option<String>,
}

impl BrokerCredentials {
    /// These accessors are intended for provider adapters only. Never log their output.
    #[allow(dead_code)]
    pub(crate) fn api_key(&self) -> Option<&str> { self.api_key.as_ref().map(Secret::expose) }
    #[allow(dead_code)]
    pub(crate) fn api_secret(&self) -> Option<&str> { self.api_secret.as_ref().map(Secret::expose) }
    #[allow(dead_code)]
    pub(crate) fn api_token(&self) -> Option<&str> { self.api_token.as_ref().map(Secret::expose) }
    #[allow(dead_code)]
    pub(crate) fn passphrase(&self) -> Option<&str> { self.passphrase.as_ref().map(Secret::expose) }
    #[allow(dead_code)]
    pub(crate) fn account_ids(&self) -> &[String] { &self.account_ids }
    #[allow(dead_code)]
    pub(crate) fn base_url(&self) -> Option<&str> { self.base_url.as_deref() }
}

pub struct BrokerEnvironment {
    providers: BTreeMap<String, BrokerCredentials>,
}

#[derive(Serialize)]
struct ProviderStatus {
    broker_id: String,
    credentials_configured: bool,
    subaccounts_allowlisted: bool,
    adapter_registered: bool,
    execution_ready: bool,
}

/// Validate optional operator-supplied API endpoints before adapters attach credentials.
/// This blocks insecure schemes, URL user-info, redirects via query/fragment, and local hosts.
fn validate_auth_method(key: &str, secret: &str, token: Option<&str>) -> Result<(), String> {
    let has_key = !key.trim().is_empty();
    let has_secret = !secret.trim().is_empty();
    let has_token = token.is_some_and(|value| !value.trim().is_empty());
    if has_key != has_secret || (has_key && has_token) || (!has_key && !has_token) {
        return Err("configure exactly one authentication method: API_TOKEN or an API_KEY/API_SECRET pair".into());
    }
    Ok(())
}

fn validate_base_url(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw.trim())
        .map_err(|_| "BASE_URL must be a valid absolute HTTPS URL".to_string())?;
    let host = parsed.host_str().ok_or_else(|| "BASE_URL must include a host".to_string())?;
    let host_lower = host.to_ascii_lowercase();
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
        || host_lower.ends_with(".test")
        || host_lower.ends_with(".invalid")
        || host_lower.ends_with(".example")
    {
        return Err("BASE_URL must use HTTPS and cannot contain user-info, query, fragment, or a local/reserved host".into());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        let is_private = match ip {
            IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local()
                || ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast(),
            IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local()
                || ip.is_unicast_link_local() || ip.is_multicast(),
        };
        if is_private {
            return Err("BASE_URL cannot target a private, local, or non-routable IP address".into());
        }
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

impl BrokerEnvironment {
    pub fn from_env() -> Result<Self, String> {
        let mut providers = BTreeMap::new();
        for broker_id in PROVIDERS {
            let prefix = format!("QOIN_BROKER_{}", broker_id.to_ascii_uppercase());
            let key = env::var(format!("{prefix}_API_KEY")).unwrap_or_default();
            let secret = env::var(format!("{prefix}_API_SECRET")).unwrap_or_default();
            let token = env::var(format!("{prefix}_API_TOKEN")).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string());
            let account_ids_raw = env::var(format!("{prefix}_ACCOUNT_IDS")).unwrap_or_else(|_| env::var(format!("{prefix}_ACCOUNT_ID")).unwrap_or_default());
            let passphrase = env::var(format!("{prefix}_PASSPHRASE")).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string());
            let base_url_raw = env::var(format!("{prefix}_BASE_URL")).ok().filter(|v| !v.trim().is_empty());

            let any_set = !key.is_empty() || !secret.is_empty() || token.is_some() || !account_ids_raw.is_empty()
                || passphrase.is_some() || base_url_raw.is_some();
            if !any_set { continue; }

            let key = key.trim().to_string();
            let secret = secret.trim().to_string();
            validate_auth_method(&key, &secret, token.as_deref())
                .map_err(|error| format!("{prefix}: {error}"))?;

            let mut seen_accounts = HashSet::new();
            let account_ids: Vec<String> = account_ids_raw.split(',')
                .map(str::trim).filter(|value| !value.is_empty())
                .filter(|value| seen_accounts.insert((*value).to_string()))
                .map(str::to_string).collect();
            if account_ids.is_empty() || account_ids.len() > 100 || account_ids.iter().any(|id| id.len() > 256) {
                return Err(format!("{prefix}_ACCOUNT_IDS must contain 1 to 100 unique account IDs of at most 256 characters"));
            }
            if key.len() > 4096 || secret.len() > 8192 || token.as_ref().is_some_and(|v| v.len() > 8192)
                || passphrase.as_ref().is_some_and(|v| v.len() > 4096)
                || base_url_raw.as_ref().is_some_and(|v| v.len() > 2048)
            {
                return Err(format!("{prefix} contains a value exceeding its configured maximum length"));
            }
            let base_url = base_url_raw.as_deref().map(validate_base_url)
                .transpose().map_err(|error| format!("{prefix}_BASE_URL: {error}"))?;

            providers.insert((*broker_id).to_string(), BrokerCredentials {
                api_key: if key.is_empty() { None } else { Some(Secret(key)) },
                api_secret: if secret.is_empty() { None } else { Some(Secret(secret)) },
                api_token: token.map(Secret),
                passphrase: passphrase.map(Secret),
                account_ids,
                base_url,
            });
        }
        Ok(Self { providers })
    }

    pub fn credentials(&self, broker_id: &str) -> Option<&BrokerCredentials> {
        self.providers.get(&broker_id.trim().to_ascii_lowercase())
    }

    pub fn account_authorized(&self, broker_id: &str, account_id: &str) -> bool {
        self.credentials(broker_id).is_some_and(|credentials| credentials.account_ids.iter().any(|configured| configured == account_id.trim()))
    }

    pub fn configured_provider_ids(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn public_status(&self) -> serde_json::Value {
        let items: Vec<ProviderStatus> = PROVIDERS.iter().map(|broker_id| {
            ProviderStatus {
                broker_id: (*broker_id).to_string(),
                credentials_configured: self.providers.contains_key(*broker_id),
                subaccounts_allowlisted: self.providers.contains_key(*broker_id),
                // Credentials alone do not make an adapter production-ready.
                adapter_registered: false,
                execution_ready: false,
            }
        }).collect();
        serde_json::json!({
            "authentication_model":"firebase_user_identity_and_server_environment_broker_credentials",
            "credential_storage":"deployment_environment_secrets",
            "user_supplied_broker_credentials":false,
            "configured_provider_count":self.providers.len(),
            "execution_enabled":false,
            "items":items
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> BrokerCredentials {
        BrokerCredentials {
            api_key: Some(Secret("private-key".into())),
            api_secret: Some(Secret("private-secret".into())),
            api_token: None,
            passphrase: None,
            account_ids: vec!["private-account".into()],
            base_url: None,
        }
    }

    #[test]
    fn public_status_never_contains_credentials_or_account_identifiers() {
        let env = BrokerEnvironment { providers: BTreeMap::from([("oanda".to_string(), credentials())]) };
        let rendered = env.public_status().to_string();
        assert!(rendered.contains("oanda"));
        assert!(!rendered.contains("private-key"));
        assert!(!rendered.contains("private-secret"));
        assert!(!rendered.contains("private-account"));
        assert!(rendered.contains(r#""execution_ready":false"#));
    }

    #[test]
    fn only_explicitly_configured_subaccounts_are_authorized() {
        let env = BrokerEnvironment { providers: BTreeMap::from([(
            "oanda".to_string(),
            BrokerCredentials {
                account_ids: vec!["sub-100".into(), "sub-200".into()],
                ..credentials()
            }
        )]) };
        assert!(env.account_authorized("OANDA", "sub-100"));
        assert!(env.account_authorized("oanda", "sub-200"));
        assert!(!env.account_authorized("oanda", "sub-300"));
        assert!(!env.account_authorized("binance_usdm", "sub-100"));
    }

    #[test]
    fn unknown_brokers_never_resolve_credentials() {
        let env = BrokerEnvironment { providers: BTreeMap::new() };
        assert!(env.credentials("unknown").is_none());
    }

    #[test]
    fn base_url_requires_https_and_rejects_local_or_credential_bearing_urls() {
        assert_eq!(validate_base_url("https://api.oanda.com/v3/").unwrap(), "https://api.oanda.com/v3");
        assert!(validate_base_url("http://api.oanda.com").is_err());
        assert!(validate_base_url("https://user:pass@api.oanda.com").is_err());
        assert!(validate_base_url("https://localhost:8080").is_err());
        assert!(validate_base_url("https://127.0.0.1").is_err());
        assert!(validate_base_url("https://service.internal").is_err());
        assert!(validate_base_url("https://api.oanda.com?redirect=http://localhost").is_err());
    }

    #[test]
    fn environment_auth_methods_must_be_exactly_one_unambiguous_method() {
        assert!(validate_auth_method("key", "secret", None).is_ok());
        assert!(validate_auth_method("", "", Some("token")).is_ok());
        assert!(validate_auth_method("key", "", None).is_err());
        assert!(validate_auth_method("", "secret", None).is_err());
        assert!(validate_auth_method("key", "secret", Some("token")).is_err());
        assert!(validate_auth_method("", "", None).is_err());
    }
}
