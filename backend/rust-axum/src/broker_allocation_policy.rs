//! Capital-aware broker routing policy.
//!
//! The policy compares a feasible single-account order with an equal split
//! across every selected account. It only selects multi-account execution when
//! every child passes adapter, account, margin, instrument-minimum and sizing
//! checks AND its estimated all-in cost is lower by the configured hurdle.
//! Cost inputs must come from fresh broker/market snapshots; this module does
//! not fetch prices, estimate fills, or submit orders.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::broker_allocation::{
    allocate_equal_notional, AllocationError, AllocationTarget, EqualAllocation,
    MAX_FANOUT_ACCOUNTS,
};

const BASIS_POINTS: i64 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerExecutionQuote {
    pub target: AllocationTarget,
    /// False until a real, authenticated execution adapter is available.
    pub adapter_ready: bool,
    /// Account health, permissions and trading status from a fresh snapshot.
    pub account_healthy: bool,
    /// Available margin converted into the portfolio currency.
    pub available_margin_portfolio: Decimal,
    /// Estimated margin required per unit of allocated notional, in bps.
    pub margin_requirement_bps: Decimal,
    /// Estimated all-in fee, including minimum commissions, in bps.
    pub fee_bps: Decimal,
    /// Expected spread/slippage estimate in bps.
    pub slippage_bps: Decimal,
    /// Currency conversion cost in bps, excluding the conversion rate itself.
    pub conversion_cost_bps: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AllocationPolicy {
    /// Extra hurdle for multi-broker execution (operational and reconciliation
    /// cost expressed in portfolio currency), supplied by deployment policy.
    pub multi_broker_overhead_portfolio: Decimal,
    /// Require this additional estimated saving after overhead. Zero means
    /// any strictly positive saving is enough.
    pub minimum_saving_portfolio: Decimal,
}

impl Default for AllocationPolicy {
    fn default() -> Self {
        Self {
            multi_broker_overhead_portfolio: Decimal::ZERO,
            minimum_saving_portfolio: Decimal::ZERO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerCost {
    pub broker_id: String,
    pub account_id: String,
    pub allocated_notional_portfolio: Decimal,
    pub estimated_fee_portfolio: Decimal,
    pub estimated_slippage_portfolio: Decimal,
    pub estimated_conversion_cost_portfolio: Decimal,
    pub estimated_margin_required_portfolio: Decimal,
    pub total_estimated_execution_cost_portfolio: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionPlan {
    Consolidated {
        broker_id: String,
        account_id: String,
        allocation: EqualAllocation,
        cost: BrokerCost,
        reason: String,
    },
    EqualSplit {
        allocation: EqualAllocation,
        costs: Vec<BrokerCost>,
        estimated_total_cost_portfolio: Decimal,
        single_broker_comparator_cost_portfolio: Decimal,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    InvalidPolicy(String),
    NoFeasibleAccount,
    NoFeasibleMultiBrokerPlan,
    Allocation(String),
    Arithmetic(String),
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPolicy(message) => write!(f, "invalid allocation policy: {message}"),
            Self::NoFeasibleAccount => write!(f, "no selected broker account can execute the full order"),
            Self::NoFeasibleMultiBrokerPlan => write!(f, "selected accounts cannot all accept an equal allocation"),
            Self::Allocation(message) => write!(f, "allocation failed: {message}"),
            Self::Arithmetic(message) => write!(f, "routing arithmetic failed: {message}"),
        }
    }
}
impl std::error::Error for RoutingError {}

fn validate_quote(quote: &BrokerExecutionQuote) -> Result<(), RoutingError> {
    let values = [
        ("available margin", quote.available_margin_portfolio),
        ("margin requirement", quote.margin_requirement_bps),
        ("fee", quote.fee_bps),
        ("slippage", quote.slippage_bps),
        ("conversion cost", quote.conversion_cost_bps),
    ];
    for (name, value) in values {
        if value < Decimal::ZERO {
            return Err(RoutingError::InvalidPolicy(format!("{name} cannot be negative")));
        }
    }
    Ok(())
}

fn cost_for(
    quote: &BrokerExecutionQuote,
    notional: Decimal,
) -> Result<BrokerCost, RoutingError> {
    let bps = quote.fee_bps
        .checked_add(quote.slippage_bps)
        .and_then(|v| v.checked_add(quote.conversion_cost_bps))
        .ok_or_else(|| RoutingError::Arithmetic("cost basis-point sum overflow".into()))?;
    let total = notional.checked_mul(bps)
        .and_then(|v| v.checked_div(Decimal::from(BASIS_POINTS)))
        .ok_or_else(|| RoutingError::Arithmetic("estimated execution cost overflow".into()))?;
    let margin = notional.checked_mul(quote.margin_requirement_bps)
        .and_then(|v| v.checked_div(Decimal::from(BASIS_POINTS)))
        .ok_or_else(|| RoutingError::Arithmetic("margin estimate overflow".into()))?;
    let fee = notional.checked_mul(quote.fee_bps)
        .and_then(|v| v.checked_div(Decimal::from(BASIS_POINTS)))
        .ok_or_else(|| RoutingError::Arithmetic("fee estimate overflow".into()))?;
    let slippage = notional.checked_mul(quote.slippage_bps)
        .and_then(|v| v.checked_div(Decimal::from(BASIS_POINTS)))
        .ok_or_else(|| RoutingError::Arithmetic("slippage estimate overflow".into()))?;
    let conversion = notional.checked_mul(quote.conversion_cost_bps)
        .and_then(|v| v.checked_div(Decimal::from(BASIS_POINTS)))
        .ok_or_else(|| RoutingError::Arithmetic("conversion cost overflow".into()))?;
    if margin > quote.available_margin_portfolio {
        return Err(RoutingError::NoFeasibleAccount);
    }
    Ok(BrokerCost {
        broker_id: quote.target.broker_id.clone(),
        account_id: quote.target.account_id.clone(),
        allocated_notional_portfolio: notional,
        estimated_fee_portfolio: fee,
        estimated_slippage_portfolio: slippage,
        estimated_conversion_cost_portfolio: conversion,
        estimated_margin_required_portfolio: margin,
        total_estimated_execution_cost_portfolio: total,
    })
}

/// Select a single suitable account unless splitting all selected accounts
/// produces a lower estimated all-in cost after the configured hurdle.
///
/// The caller must still perform a fresh, atomic preflight of every child
/// (account, quote, capabilities, instrument, portfolio risk and margin) and
/// persist parent + child intents before dispatch. A plan is not an order.
pub fn choose_execution_plan(
    parent_notional_portfolio: Decimal,
    selected: &[BrokerExecutionQuote],
    policy: AllocationPolicy,
) -> Result<ExecutionPlan, RoutingError> {
    if parent_notional_portfolio <= Decimal::ZERO {
        return Err(RoutingError::InvalidPolicy("parent notional must be positive".into()));
    }
    if policy.multi_broker_overhead_portfolio < Decimal::ZERO
        || policy.minimum_saving_portfolio < Decimal::ZERO
    {
        return Err(RoutingError::InvalidPolicy("overhead and minimum saving must be non-negative".into()));
    }
    if selected.is_empty() || selected.len() > MAX_FANOUT_ACCOUNTS {
        return Err(RoutingError::InvalidPolicy("select between 1 and 15 broker accounts".into()));
    }
    for quote in selected {
        validate_quote(quote)?;
    }

    let mut best_single: Option<(BrokerCost, EqualAllocation)> = None;
    for quote in selected.iter().filter(|q| q.adapter_ready && q.account_healthy) {
        let Ok(allocation) = allocate_equal_notional(
            parent_notional_portfolio,
            std::slice::from_ref(&quote.target),
        ) else { continue };
        let actual = allocation.allocated_notional_portfolio;
        let Ok(cost) = cost_for(quote, actual) else { continue };
        let replace = best_single.as_ref().is_none_or(|(best, _)| {
            cost.total_estimated_execution_cost_portfolio < best.total_estimated_execution_cost_portfolio
                || (cost.total_estimated_execution_cost_portfolio == best.total_estimated_execution_cost_portfolio
                    && (&cost.broker_id, &cost.account_id) < (&best.broker_id, &best.account_id))
        });
        if replace {
            best_single = Some((cost, allocation));
        }
    }
    let (single_cost, single_allocation) = best_single.ok_or(RoutingError::NoFeasibleAccount)?;

    // One account is already the simplest valid plan. Do not manufacture a
    // multi-broker route when it cannot diversify execution.
    if selected.len() == 1 {
        return Ok(ExecutionPlan::Consolidated {
            broker_id: single_cost.broker_id.clone(),
            account_id: single_cost.account_id.clone(),
            allocation: single_allocation,
            cost: single_cost,
            reason: "Only one selected account is available; consolidate execution".into(),
        });
    }

    let all_ready = selected.iter().all(|q| q.adapter_ready && q.account_healthy);
    let multi = if all_ready {
        allocate_equal_notional(
            parent_notional_portfolio,
            &selected.iter().map(|q| q.target.clone()).collect::<Vec<_>>(),
        ).map_err(|error: AllocationError| RoutingError::Allocation(error.to_string())).ok()
    } else {
        None
    };

    if let Some(allocation) = multi {
        let mut costs = Vec::with_capacity(selected.len());
        let mut total_cost = Decimal::ZERO;
        let mut all_margin_feasible = true;
        for (quote, child) in selected.iter().zip(allocation.children.iter()) {
            match cost_for(quote, child.allocated_notional_portfolio) {
                Ok(cost) => {
                    total_cost = total_cost.checked_add(cost.total_estimated_execution_cost_portfolio)
                        .ok_or_else(|| RoutingError::Arithmetic("aggregate execution cost overflow".into()))?;
                    costs.push(cost);
                }
                Err(_) => { all_margin_feasible = false; break; }
            }
        }
        if all_margin_feasible {
            let multi_total = total_cost.checked_add(policy.multi_broker_overhead_portfolio)
                .ok_or_else(|| RoutingError::Arithmetic("multi-broker overhead overflow".into()))?;
            let required = single_cost.total_estimated_execution_cost_portfolio
                .checked_sub(policy.minimum_saving_portfolio)
                .ok_or_else(|| RoutingError::Arithmetic("saving hurdle overflow".into()))?;
            if multi_total < required {
                return Ok(ExecutionPlan::EqualSplit {
                    allocation,
                    costs,
                    estimated_total_cost_portfolio: multi_total,
                    single_broker_comparator_cost_portfolio: single_cost.total_estimated_execution_cost_portfolio,
                    reason: "All selected accounts passed sizing and margin checks, and estimated execution cost beats the best single-account route after the configured overhead and saving hurdle".into(),
                });
            }
        }
    }

    Ok(ExecutionPlan::Consolidated {
        broker_id: single_cost.broker_id.clone(),
        account_id: single_cost.account_id.clone(),
        allocation: single_allocation,
        cost: single_cost,
        reason: "Consolidation is preferred: multi-account sizing, readiness, margin, or estimated cost did not justify splitting".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal { Decimal::from_str(value).unwrap() }

    fn quote(index: usize, fee: &str, slippage: &str, ready: bool) -> BrokerExecutionQuote {
        BrokerExecutionQuote {
            target: AllocationTarget {
                broker_id: format!("broker-{index}"),
                account_id: format!("account-{index}"),
                instrument_id: "instrument".into(),
                native_unit_value_portfolio: d("1"),
                quantity_step: d("0.01"),
                minimum_quantity: d("0.01"),
                minimum_notional_portfolio: Some(d("1")),
            },
            adapter_ready: ready,
            account_healthy: true,
            available_margin_portfolio: d("100000"),
            margin_requirement_bps: d("1000"),
            fee_bps: d(fee),
            slippage_bps: d(slippage),
            conversion_cost_bps: d("0"),
        }
    }

    #[test]
    fn small_capital_consolidates_when_each_equal_share_is_below_minimum() {
        let quotes = (0..15).map(|i| quote(i, "1", "1", true)).collect::<Vec<_>>();
        let plan = choose_execution_plan(d("100"), &quotes, AllocationPolicy::default()).unwrap();
        assert!(matches!(plan, ExecutionPlan::Consolidated { .. }));
    }

    #[test]
    fn multi_broker_route_requires_lower_estimated_cost() {
        let quotes = vec![quote(0, "10", "10", true), quote(1, "1", "1", true)];
        let plan = choose_execution_plan(d("1000"), &quotes, AllocationPolicy::default()).unwrap();
        assert!(matches!(plan, ExecutionPlan::EqualSplit { .. }));
    }

    #[test]
    fn multi_broker_route_is_rejected_when_one_adapter_is_not_ready() {
        let quotes = vec![quote(0, "10", "10", true), quote(1, "1", "1", false)];
        let plan = choose_execution_plan(d("1000"), &quotes, AllocationPolicy::default()).unwrap();
        assert!(matches!(plan, ExecutionPlan::Consolidated { broker_id, .. } if broker_id == "broker-0"));
    }

    #[test]
    fn operational_overhead_can_make_single_account_better() {
        let quotes = vec![quote(0, "10", "10", true), quote(1, "1", "1", true)];
        let policy = AllocationPolicy { multi_broker_overhead_portfolio: d("3"), minimum_saving_portfolio: d("0") };
        let plan = choose_execution_plan(d("1000"), &quotes, policy).unwrap();
        assert!(matches!(plan, ExecutionPlan::Consolidated { .. }));
    }

    #[test]
    fn margin_shortfall_disqualifies_multi_broker_plan_but_not_other_account() {
        let mut quotes = vec![quote(0, "10", "10", true), quote(1, "1", "1", true)];
        quotes[1].available_margin_portfolio = d("1");
        let plan = choose_execution_plan(d("1000"), &quotes, AllocationPolicy::default()).unwrap();
        assert!(matches!(plan, ExecutionPlan::Consolidated { broker_id, .. } if broker_id == "broker-0"));
    }

    #[test]
    fn invalid_negative_cost_estimate_fails_closed() {
        let mut quotes = vec![quote(0, "1", "1", true)];
        quotes[0].fee_bps = d("-1");
        assert!(matches!(choose_execution_plan(d("100"), &quotes, AllocationPolicy::default()), Err(RoutingError::InvalidPolicy(_))));
    }
}
