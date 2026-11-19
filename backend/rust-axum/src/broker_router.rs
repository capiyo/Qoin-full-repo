//! Deterministic equal-share broker allocation planning.
//! This module creates an auditable allocation plan only; it never submits orders.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BrokerSelection {
    pub broker_id: String,
    pub quote_currency: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AllocationRequest {
    pub total_notional: f64,
    pub quote_currency: String,
    pub selections: Vec<BrokerSelection>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BrokerAllocation {
    pub broker_id: String,
    pub quote_currency: String,
    pub allocation: f64,
    pub share_fraction: f64,
    pub adapter_status: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AllocationPlan {
    pub total_notional: f64,
    pub quote_currency: String,
    pub broker_count: usize,
    pub allocation_per_broker: f64,
    pub allocated_total: f64,
    pub unallocated_remainder: f64,
    pub execution_enabled: bool,
    pub allocations: Vec<BrokerAllocation>,
    pub policy: &'static str,
}

fn adapter_status(broker_id: &str) -> &'static str {
    match broker_id {
        "binance" | "binance_usdm" => "adapter_present_operator_gated",
        "coinbase" | "kraken" | "interactive_brokers" | "alpaca" | "oanda"
        | "saxo" | "ig" | "fxcm" | "tradier" => "adapter_not_connected",
        _ => "unknown_broker",
    }
}

pub fn build_equal_share_plan(request: AllocationRequest) -> Result<AllocationPlan, String> {
    if !request.total_notional.is_finite() || request.total_notional <= 0.0 {
        return Err("total_notional must be finite and greater than zero".into());
    }
    let currency = request.quote_currency.trim().to_ascii_uppercase();
    if currency.len() < 3 || currency.len() > 8 || !currency.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("quote_currency must be a 3-8 character alphanumeric currency code".into());
    }
    if request.selections.is_empty() || request.selections.len() > 50 {
        return Err("select between 1 and 50 brokers".into());
    }
    let mut seen = HashSet::new();
    for selection in &request.selections {
        let broker = selection.broker_id.trim().to_ascii_lowercase();
        let selected_currency = selection.quote_currency.trim().to_ascii_uppercase();
        if broker.is_empty() || broker.len() > 64 || !broker.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-') {
            return Err("each broker_id must be a valid non-empty identifier".into());
        }
        if !seen.insert(broker.clone()) {
            return Err(format!("broker {broker} is selected more than once"));
        }
        if selected_currency != currency {
            return Err(format!("broker {broker} quote currency {selected_currency} does not match requested {currency}; automatic FX conversion is not performed"));
        }
    }
    let count = request.selections.len();
    let equal_share = request.total_notional / count as f64;
    if !equal_share.is_finite() || equal_share <= 0.0 {
        return Err("allocation per broker is below supported numeric precision".into());
    }
    let mut allocated = 0.0;
    let mut allocations = Vec::with_capacity(count);
    for (index, selection) in request.selections.into_iter().enumerate() {
        // Assign floating-point representation residue to the final child only;
        // this keeps the parent notional conserved without redistributing a failed broker.
        let amount = if index + 1 == count { request.total_notional - allocated } else { equal_share };
        allocated += amount;
        allocations.push(BrokerAllocation {
            broker_id: selection.broker_id.trim().to_ascii_lowercase(),
            quote_currency: currency.clone(),
            allocation: amount,
            share_fraction: 1.0 / count as f64,
            adapter_status: adapter_status(selection.broker_id.trim().to_ascii_lowercase().as_str()),
        });
    }
    let execution_enabled = allocations.iter().all(|allocation| allocation.adapter_status == "adapter_present_operator_gated");
    Ok(AllocationPlan {
        total_notional: request.total_notional,
        quote_currency: currency,
        broker_count: count,
        allocation_per_broker: equal_share,
        allocated_total: allocated,
        unallocated_remainder: request.total_notional - allocated,
        execution_enabled,
        allocations,
        policy: "equal_share_no_silent_omission_no_redistribution_no_automatic_fx",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(id: &str) -> BrokerSelection {
        BrokerSelection { broker_id: id.into(), quote_currency: "USD".into() }
    }

    #[test]
    fn splits_one_hundred_equally_across_fifteen_brokers_and_conserves_parent_notional() {
        let ids = ["binance","coinbase","kraken","interactive_brokers","alpaca","oanda","saxo","ig","fxcm","tradier","b11","b12","b13","b14","b15"];
        let plan = build_equal_share_plan(AllocationRequest {
            total_notional: 100.0, quote_currency: "USD".into(),
            selections: ids.iter().map(|id| selection(id)).collect(),
        }).unwrap();
        assert_eq!(plan.broker_count, 15);
        assert!((plan.allocated_total - 100.0).abs() < 1e-10);
        assert!(plan.allocations.iter().all(|item| (item.allocation - 100.0 / 15.0).abs() < 1e-10));
        assert!(!plan.execution_enabled);
        assert_eq!(plan.allocations.len(), 15);
    }

    #[test]
    fn duplicate_or_mixed_currency_brokers_are_rejected_instead_of_redistributed() {
        assert!(build_equal_share_plan(AllocationRequest {
            total_notional: 100.0, quote_currency: "USD".into(),
            selections: vec![selection("binance"), selection("binance")],
        }).is_err());
        assert!(build_equal_share_plan(AllocationRequest {
            total_notional: 100.0, quote_currency: "USD".into(),
            selections: vec![selection("binance"), BrokerSelection {
                broker_id: "oanda".into(), quote_currency: "EUR".into()
            }],
        }).is_err());
    }

    #[test]
    fn broker_adapter_gaps_are_explicit_and_never_marked_executable() {
        let plan = build_equal_share_plan(AllocationRequest {
            total_notional: 100.0, quote_currency: "USDT".into(),
            selections: vec![
                BrokerSelection { broker_id: "binance_usdm".into(), quote_currency: "USDT".into() },
                BrokerSelection { broker_id: "coinbase".into(), quote_currency: "USDT".into() },
            ],
        }).unwrap();
        assert!(!plan.execution_enabled);
        assert_eq!(plan.allocations[0].adapter_status, "adapter_present_operator_gated");
        assert_eq!(plan.allocations[1].adapter_status, "adapter_not_connected");
        assert!((plan.allocations[0].allocation - plan.allocations[1].allocation).abs() < 1e-12);
    }

    #[test]
    fn invalid_notional_and_selection_count_fail_closed() {
        for total in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(build_equal_share_plan(AllocationRequest {
                total_notional: total, quote_currency: "USD".into(), selections: vec![selection("binance")]
            }).is_err());
        }
        assert!(build_equal_share_plan(AllocationRequest {
            total_notional: 100.0, quote_currency: "USD".into(), selections: vec![]
        }).is_err());
    }
}
