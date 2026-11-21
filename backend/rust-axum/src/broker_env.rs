//! Server-side broker credentials loaded exclusively from deployment environment secrets.
//! No credential value is serializable or returned by an HTTP handler.
use std::{collections::BTreeMap, env};
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
    api_key: Secret,
    api_secret: Secret,
    passphrase: Option<Secret>,
    account_ids: Vec<String>,
    base_url: Option<String>,
}

impl BrokerCredentials {
    /// These accessors are intended for provider adapters only. Never log their output.
    #[allow(dead_code)]
    pub(crate) fn api_key(&self) -> &str { self.api_key.expose() }
    #[allow(dead_code)]
    pub(crate) fn api_secret(&self) -> &str { self.api_secret.expose() }
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
    account_configured: bool,
    adapter_registered: bool,
    execution_ready: bool,
}

impl BrokerEnvironment {
    pub fn from_env() -> Result<Self, String> {
        let mut providers = BTreeMap::new();
        for broker_id in PROVIDERS {
            let prefix = format!("QOIN_BROKER_{}", broker_id.to_ascii_uppercase());
            let key = env::var(format!("{prefix}_API_KEY")).unwrap_or_default();
            let secret = env::var(format!("{prefix}_API_SECRET")).unwrap_or_default();
            let account_ids_raw = env::var(format!("{prefix}_ACCOUNT_IDS")).unwrap_or_else(|_| env::var(format!("{prefix}_ACCOUNT_ID")).unwrap_or_default());
            let passphrase = env::var(format!("{prefix}_PASSPHRASE")).ok().filter(|v| !v.trim().is_empty());
            let base_url = env::var(format!("{prefix}_BASE_URL")).ok().filter(|v| !v.trim().is_empty());

            let any_set = !key.is_empty() || !secret.is_empty() || !account_ids_raw.is_empty()
                || passphrase.is_some() || base_url.is_some();
            if !any_set { continue; }
            let account_ids: Vec<String> = account_ids_raw.split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).collect();
            if key.trim().is_empty() || secret.trim().is_empty() || account_ids.is_empty() {
                return Err(format!("{prefix} requires API_KEY, API_SECRET and at least one ACCOUNT_IDS entry"));
            }
            if account_ids.len() > 100 || account_ids.iter().any(|id| id.len() > 256) {
                return Err(format!("{prefix}_ACCOUNT_IDS must contain 1 to 100 account IDs of at most 256 characters"));
            }
            if key.len() > 4096 || secret.len() > 8192
                || passphrase.as_ref().is_some_and(|v| v.len() > 4096)
                || base_url.as_ref().is_some_and(|v| v.len() > 2048)
            {
                return Err(format!("{prefix} contains a value exceeding its configured maximum length"));
            }
            providers.insert((*broker_id).to_string(), BrokerCredentials {
                api_key: Secret(key),
                api_secret: Secret(secret),
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
                account_configured: self.providers.contains_key(*broker_id),
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

    #[test]
    fn public_status_never_contains_credentials_or_account_identifiers() {
        let env = BrokerEnvironment { providers: BTreeMap::from([(
            "oanda".to_string(),
            BrokerCredentials {
                api_key: Secret("private-key".into()),
                api_secret: Secret("private-secret".into()),
                passphrase: None,
                account_ids: vec!["private-account".into()],
                base_url: None,
            }
        )]) };
        let rendered = env.public_status().to_string();
        assert!(rendered.contains("oanda"));
        assert!(!rendered.contains("private-key"));
        assert!(!rendered.contains("private-secret"));
        assert!(!rendered.contains("private-account"));
        assert!(rendered.contains("\"execution_ready\":false"));
    }

    #[test]
    fn only_explicitly_configured_subaccounts_are_authorized() {
        let env = BrokerEnvironment { providers: BTreeMap::from([(
            "oanda".to_string(),
            BrokerCredentials {
                api_key: Secret("key".into()),
                api_secret: Secret("secret".into()),
                passphrase: None,
                account_ids: vec!["sub-100".into(), "sub-200".into()],
                base_url: None,
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
}
