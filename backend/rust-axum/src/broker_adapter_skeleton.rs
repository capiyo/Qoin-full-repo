//! Explicit fail-closed adapter skeletons for the supported broker directory.
//!
//! These types make provider coverage compile-time visible without pretending
//! that a catalogue entry is a working integration. Every operation fails
//! closed until provider-specific authentication, account discovery, metadata,
//! pricing, margin, bracket orders, cancellation and reconciliation are built
//! and conformance-tested. They are intentionally NOT registered at runtime.

use crate::broker_contract::{
    BrokerAccountSnapshot, BrokerAdapter, BrokerCapabilities,
    BrokerOrderReceipt, BrokerQuote, ContractError, InstrumentSpec, NormalizedOrder,
};
use async_trait::async_trait;
use rust_decimal::Decimal;

const NOT_IMPLEMENTED: &str =
    "provider adapter scaffold only; provider API integration and conformance tests are required";

macro_rules! provider_adapter {
    ($type_name:ident, $id:literal) => {
        pub struct $type_name;

        #[async_trait]
        impl BrokerAdapter for $type_name {
            fn broker_id(&self) -> &'static str { $id }

            async fn capabilities(
                &self, _owner_uid: &str, _account_id: &str, _instrument_id: &str,
            ) -> Result<BrokerCapabilities, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn account_snapshot(
                &self, _owner_uid: &str, _account_id: &str,
            ) -> Result<BrokerAccountSnapshot, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn instrument_spec(
                &self, _owner_uid: &str, _account_id: &str, _instrument_id: &str,
            ) -> Result<InstrumentSpec, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn latest_quote(
                &self, _owner_uid: &str, _account_id: &str, _instrument_id: &str,
            ) -> Result<BrokerQuote, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn required_margin_account_currency(
                &self, _owner_uid: &str, _order: &NormalizedOrder, _reference_price: Decimal,
            ) -> Result<Decimal, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn submit_order(
                &self, _owner_uid: &str, _order: NormalizedOrder, _protective_stop_price: Option<Decimal>,
            ) -> Result<BrokerOrderReceipt, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn order_status(
                &self, _owner_uid: &str, _account_id: &str, _broker_order_id: &str,
            ) -> Result<BrokerOrderReceipt, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }

            async fn cancel_order(
                &self, _owner_uid: &str, _account_id: &str, _broker_order_id: &str,
            ) -> Result<BrokerOrderReceipt, ContractError> {
                Err(ContractError::Unsupported(NOT_IMPLEMENTED.into()))
            }
        }
    };
}

provider_adapter!(BinanceUsdmAdapter, "binance_usdm");
provider_adapter!(CoinbaseAdvancedTradeAdapter, "coinbase_advanced_trade");
provider_adapter!(KrakenSpotAdapter, "kraken_spot");
provider_adapter!(BybitAdapter, "bybit");
provider_adapter!(OkxAdapter, "okx");
provider_adapter!(BitgetAdapter, "bitget");
provider_adapter!(GateIoAdapter, "gate_io");
provider_adapter!(KucoinAdapter, "kucoin");
provider_adapter!(DeribitAdapter, "deribit");
provider_adapter!(HyperliquidAdapter, "hyperliquid");
provider_adapter!(OandaAdapter, "oanda");
provider_adapter!(IgAdapter, "ig");
provider_adapter!(FxcmAdapter, "fxcm");
provider_adapter!(CapitalComAdapter, "capital_com");
provider_adapter!(DerivAdapter, "deriv");
provider_adapter!(XtbAdapter, "xtb");
provider_adapter!(PepperstoneAdapter, "pepperstone");
provider_adapter!(IcMarketsAdapter, "ic_markets");
provider_adapter!(ForexComAdapter, "forex_com");
provider_adapter!(CmcMarketsAdapter, "cmc_markets");

/// Stable list used by tests and readiness diagnostics; this is not a live registry.
pub const PROVIDER_ADAPTER_IDS: [&str; 20] = [
    "binance_usdm", "coinbase_advanced_trade", "kraken_spot", "bybit", "okx",
    "bitget", "gate_io", "kucoin", "deribit", "hyperliquid", "oanda", "ig",
    "fxcm", "capital_com", "deriv", "xtb", "pepperstone", "ic_markets",
    "forex_com", "cmc_markets",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_directory_covers_all_twenty_catalogue_ids_without_claiming_readiness() {
        assert_eq!(PROVIDER_ADAPTER_IDS.len(), 20);
        let unique: std::collections::HashSet<_> = PROVIDER_ADAPTER_IDS.iter().copied().collect();
        assert_eq!(unique.len(), 20);
        assert!(PROVIDER_ADAPTER_IDS.iter().all(|id| !id.trim().is_empty()));
    }

    #[tokio::test]
    async fn provider_scaffold_cannot_submit_orders() {
        let adapter = BinanceUsdmAdapter;
        assert_eq!(adapter.broker_id(), "binance_usdm");
        let result = adapter.capabilities("owner", "account", "BTC-USD").await;
        assert!(matches!(result, Err(ContractError::Unsupported(_))));
    }
}
