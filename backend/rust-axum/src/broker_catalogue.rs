//! Public broker/exchange directory. Catalogue membership is not account connectivity.
use axum::{response::IntoResponse, Json};
use serde_json::{json, Value};

pub fn catalogue() -> Value {
    // Deliberately balanced: ten crypto venues and ten FX/CFD brokers.
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
        ("oanda","OANDA","forex_broker","forex_cfds","REST v20 + streaming","adapter_pending",true),
        ("ig","IG","forex_broker","forex_cfds","REST + streaming","adapter_pending",true),
        ("fxcm","FXCM","forex_broker","forex_cfds","REST/streaming SDK; account eligibility required","adapter_pending",true),
        ("capital_com","Capital.com","forex_broker","forex_cfds","REST + WebSocket","adapter_pending",true),
        ("deriv","Deriv","forex_broker","forex_synthetics_options","WebSocket + REST","adapter_pending",true),
        ("xtb","XTB","forex_broker","forex_cfds","xAPI socket/WebSocket","adapter_pending",true),
        ("pepperstone","Pepperstone","forex_broker","forex_cfds","cTrader Open API / platform integration","adapter_pending",true),
        ("ic_markets","IC Markets","forex_broker","forex_cfds","cTrader Open API / platform integration","adapter_pending",true),
        ("forex_com","FOREX.com","forex_broker","forex_cfds","REST API; regional eligibility required","adapter_pending",true),
        ("cmc_markets","CMC Markets","forex_broker","forex_cfds","REST API; account eligibility required","adapter_pending",true),
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
            "oanda" => "https://developer.oanda.com/rest-live-v20/introduction/",
            "ig" => "https://labs.ig.com/",
            "fxcm" => "https://fxcm-api.readthedocs.io/",
            "capital_com" => "https://open-api.capital.com/",
            "deriv" => "https://api.deriv.com/",
            "xtb" => "https://xopenhub.pro/api/xapi/",
            "pepperstone" => "https://openapi.ctrader.com/",
            "ic_markets" => "https://openapi.ctrader.com/",
            "forex_com" => "https://www.forex.com/en-us/developer/",
            "cmc_markets" => "https://www.cmcmarkets.com/en/platforms/api",
            _ => ""
        }, "execution_status": status, "selectable": selectable,
        "execution_ready": false,
        "note": if *id == "binance_usdm" {
            "A Binance USD-M executor exists in the legacy internal order path; generic multi-account adapter registration and account linking are not connected."
        } else {
            "Provider API route is listed for discovery; Qoin has not implemented or verified this provider's execution adapter or account connection."
        }
    })).collect();
    json!({
        "version": 2,
        "count": items.len(),
        "max_selected": 20,
        "market_balance": {"crypto": 10, "forex": 10},
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
    fn directory_is_balanced_and_has_no_false_readiness() {
        let value = catalogue();
        let items = value["items"].as_array().unwrap();
        assert_eq!(items.len(), 20);
        let mut ids = std::collections::HashSet::new();
        let mut crypto = 0;
        let mut forex = 0;
        for item in items {
            assert!(ids.insert(item["id"].as_str().unwrap()));
            assert_eq!(item["selectable"], true);
            assert_eq!(item["execution_ready"], false);
            assert!(!item["documentation_url"].as_str().unwrap_or("").is_empty());
            match item["provider_kind"].as_str().unwrap() {
                "exchange" => crypto += 1,
                "forex_broker" => forex += 1,
                other => panic!("unexpected provider category: {other}"),
            }
        }
        assert_eq!(crypto, 10);
        assert_eq!(forex, 10);
        assert_eq!(value["market_balance"]["crypto"], 10);
        assert_eq!(value["market_balance"]["forex"], 10);
        assert_eq!(value["max_selected"], 20);
        assert_eq!(value["execution_enabled"], false);
    }
}
