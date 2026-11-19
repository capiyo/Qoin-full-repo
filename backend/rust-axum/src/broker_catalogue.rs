//! Public broker/exchange directory. Catalogue membership is not account connectivity.
use axum::{response::IntoResponse, Json};
use serde_json::{json, Value};

pub fn catalogue() -> Value {
    let providers = [
        ("binance_usdm","Binance USD-M","exchange","crypto","REST + WebSocket","execution_adapter_exists_not_connected",true),
        ("coinbase_advanced_trade","Coinbase Advanced Trade","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("kraken_spot","Kraken Spot","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("bybit","Bybit","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("okx","OKX","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("bitget","Bitget","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("gate_io","Gate.io","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("kucoin","KuCoin","exchange","crypto","REST + WebSocket","adapter_pending",true),
        ("deribit","Deribit","exchange","crypto_derivatives","REST + WebSocket","adapter_pending",true),
        ("hyperliquid","Hyperliquid","exchange","crypto_perpetuals","JSON-RPC + WebSocket","adapter_pending",true),
        ("interactive_brokers","Interactive Brokers","broker","multi_asset","Client Portal REST/WebSocket + TWS socket","adapter_pending",true),
        ("alpaca","Alpaca","broker","us_equities_options_crypto","REST + WebSocket","adapter_pending",true),
        ("oanda","OANDA","broker","forex_cfds","REST v20 + streaming","adapter_pending",true),
        ("saxo","Saxo","broker","multi_asset","REST + WebSocket OpenAPI","adapter_pending",true),
        ("ig","IG","broker","forex_cfds","REST + streaming","adapter_pending",true),
        ("fxcm","FXCM","broker","forex_cfds","REST/streaming SDK; account eligibility required","adapter_pending",true),
        ("capital_com","Capital.com","broker","forex_cfds","REST + WebSocket","adapter_pending",true),
        ("deriv","Deriv","broker","forex_synthetics_options","WebSocket + REST","adapter_pending",true),
        ("trading_212","Trading 212","broker","equities_etfs","REST","adapter_pending",true),
        ("xtb","XTB","broker","multi_asset_cfds","xAPI socket/WebSocket","adapter_pending",true),
    ];
    let items: Vec<Value> = providers.iter().map(|(id,name,kind,assets,api,status,selectable)| json!({
        "id": id, "name": name, "provider_kind": kind, "asset_classes": assets,
        "api": api, "documentation_url": match *id {
            "binance_usdm" => "https://developers.binance.com/docs/derivatives/usds-margined-futures/general-info",
            "coinbase_advanced_trade" => "https://docs.cdp.coinbase.com/advanced-trade/docs/welcome",
            "kraken_spot" => "https://docs.kraken.com/api/",
            "bybit" => "https://bybit-exchange.github.io/docs/v5/intro",
            "okx" => "https://www.okx.com/docs-v5/en/",
            "bitget" => "https://www.bitget.com/api-doc/common/intro",
            "gate_io" => "https://www.gate.com/docs/developers/apiv4/en/",
            "kucoin" => "https://www.kucoin.com/docs-new",
            "deribit" => "https://docs.deribit.com/",
            "hyperliquid" => "https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api",
            "interactive_brokers" => "https://www.interactivebrokers.com/campus/ibkr-api-page/",
            "alpaca" => "https://docs.alpaca.markets/",
            "oanda" => "https://developer.oanda.com/rest-live-v20/introduction/",
            "saxo" => "https://www.developer.saxo/openapi/learn",
            "ig" => "https://labs.ig.com/",
            "fxcm" => "https://fxcm-api.readthedocs.io/",
            "capital_com" => "https://open-api.capital.com/",
            "deriv" => "https://api.deriv.com/",
            "trading_212" => "https://docs.trading212.com/",
            "xtb" => "https://xopenhub.pro/api/xapi/",
            _ => ""
        }, "execution_status": status, "selectable": selectable,
        "execution_ready": false,
        "note": if *id == "binance_usdm" {
            "A Binance USD-M executor exists in the legacy internal order path; generic multi-account adapter registration and account linking are not connected."
        } else {
            "Provider has a documented API/platform route; Qoin has not implemented or verified an execution adapter or account connection for this provider."
        }
    })).collect();
    json!({
        "version": 1,
        "count": items.len(),
        "max_selected": 20,
        "execution_enabled": false,
        "items": items,
        "warning": "Directory entries are selectable connection preferences, not proof of account connectivity or execution readiness. Qoin is research-only."
    })
}

pub async fn get_brokers() -> impl IntoResponse {
    Json(catalogue())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_has_twenty_unique_selectable_providers_and_no_false_readiness() {
        let value = catalogue();
        let items = value["items"].as_array().unwrap();
        assert_eq!(items.len(), 20);
        let mut ids = std::collections::HashSet::new();
        for item in items {
            assert!(ids.insert(item["id"].as_str().unwrap()));
            assert_eq!(item["selectable"], true);
            assert_eq!(item["execution_ready"], false);
            assert!(!item["documentation_url"].as_str().unwrap_or("").is_empty());
        }
        assert_eq!(value["max_selected"], 20);
        assert_eq!(value["execution_enabled"], false);
    }
}
