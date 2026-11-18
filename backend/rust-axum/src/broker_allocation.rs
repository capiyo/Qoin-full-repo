//! Equal-notional fan-out allocation for a single parent strategy order.
//!
//! Each eligible broker account receives the same target share of portfolio
//! notional. Native quantities are converted using that account's instrument
//! multiplier and currency conversion chain, then rounded DOWN to the broker's
//! quantity step. Any residual remains unallocated; it is never redistributed.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_FANOUT_ACCOUNTS: usize = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationTarget {
    pub broker_id: String,
    pub account_id: String,
    pub instrument_id: String,
    /// Portfolio-currency value of one native quantity unit at the reference price.
    pub native_unit_value_portfolio: Decimal,
    pub quantity_step: Decimal,
    pub minimum_quantity: Decimal,
    /// Optional minimum trade notional, already converted to portfolio currency.
    pub minimum_notional_portfolio: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildAllocation {
    pub broker_id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub target_notional_portfolio: Decimal,
    pub quantity: Decimal,
    pub allocated_notional_portfolio: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EqualAllocation {
    pub parent_notional_portfolio: Decimal,
    pub target_notional_per_account: Decimal,
    pub children: Vec<ChildAllocation>,
    pub allocated_notional_portfolio: Decimal,
    pub unallocated_remainder_portfolio: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationError {
    InvalidParentNotional,
    InvalidRecipientCount,
    DuplicateAccount,
    InvalidTarget(String),
    BelowMinimum(String),
    Arithmetic(String),
}

impl std::fmt::Display for AllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParentNotional => write!(f, "parent notional must be positive"),
            Self::InvalidRecipientCount => write!(f, "fan-out requires between 1 and 15 eligible accounts"),
            Self::DuplicateAccount => write!(f, "each broker account may appear only once in an allocation"),
            Self::InvalidTarget(message) => write!(f, "invalid allocation target: {message}"),
            Self::BelowMinimum(account) => write!(f, "equal share is below broker minimum for account {account}"),
            Self::Arithmetic(message) => write!(f, "allocation arithmetic failed: {message}"),
        }
    }
}
impl std::error::Error for AllocationError {}

/// Allocate a total portfolio-currency notional equally across all recipients.
/// This is a pre-dispatch primitive: callers must still obtain fresh account,
/// quote, instrument and capability snapshots and pass the shared risk gate
/// for EVERY child order before any child is submitted.
pub fn allocate_equal_notional(
    parent_notional_portfolio: Decimal,
    targets: &[AllocationTarget],
) -> Result<EqualAllocation, AllocationError> {
    if parent_notional_portfolio <= Decimal::ZERO {
        return Err(AllocationError::InvalidParentNotional);
    }
    if targets.is_empty() || targets.len() > MAX_FANOUT_ACCOUNTS {
        return Err(AllocationError::InvalidRecipientCount);
    }

    let canonical_instrument = targets[0].instrument_id.trim();
    if canonical_instrument.is_empty() {
        return Err(AllocationError::InvalidTarget("instrument identifier is required".into()));
    }
    let mut accounts = HashSet::with_capacity(targets.len());
    for target in targets {
        if target.broker_id.trim().is_empty()
            || target.account_id.trim().is_empty()
            || target.instrument_id.trim().is_empty()
        {
            return Err(AllocationError::InvalidTarget("broker, account and instrument identifiers are required".into()));
        }
        if target.instrument_id.trim() != canonical_instrument {
            return Err(AllocationError::InvalidTarget("all recipients must target the same canonical instrument".into()));
        }
        let account_key = (target.broker_id.trim().to_ascii_lowercase(), target.account_id.trim().to_string());
        if !accounts.insert(account_key) {
            return Err(AllocationError::DuplicateAccount);
        }
        if target.native_unit_value_portfolio <= Decimal::ZERO
            || target.quantity_step <= Decimal::ZERO
            || target.minimum_quantity < Decimal::ZERO
            || target.minimum_notional_portfolio.is_some_and(|value| value < Decimal::ZERO)
        {
            return Err(AllocationError::InvalidTarget(target.account_id.clone()));
        }
    }

    let divisor = Decimal::from(targets.len() as u32);
    let target_share = parent_notional_portfolio
        .checked_div(divisor)
        .ok_or_else(|| AllocationError::Arithmetic("could not divide parent notional".into()))?;

    let mut children = Vec::with_capacity(targets.len());
    let mut allocated_total = Decimal::ZERO;
    for target in targets {
        let raw_quantity = target_share
            .checked_div(target.native_unit_value_portfolio)
            .ok_or_else(|| AllocationError::Arithmetic(format!("quantity conversion failed for {}", target.account_id)))?;
        let step_count = raw_quantity
            .checked_div(target.quantity_step)
            .ok_or_else(|| AllocationError::Arithmetic(format!("quantity step conversion failed for {}", target.account_id)))?
            .floor();
        let quantity = step_count
            .checked_mul(target.quantity_step)
            .ok_or_else(|| AllocationError::Arithmetic(format!("quantity rounding overflow for {}", target.account_id)))?;
        if quantity < target.minimum_quantity || quantity <= Decimal::ZERO {
            return Err(AllocationError::BelowMinimum(target.account_id.clone()));
        }
        let allocated_notional = quantity
            .checked_mul(target.native_unit_value_portfolio)
            .ok_or_else(|| AllocationError::Arithmetic(format!("notional overflow for {}", target.account_id)))?;
        if target.minimum_notional_portfolio.is_some_and(|minimum| allocated_notional < minimum) {
            return Err(AllocationError::BelowMinimum(target.account_id.clone()));
        }
        allocated_total = allocated_total
            .checked_add(allocated_notional)
            .ok_or_else(|| AllocationError::Arithmetic("aggregate allocation overflow".into()))?;
        children.push(ChildAllocation {
            broker_id: target.broker_id.trim().to_ascii_lowercase(),
            account_id: target.account_id.trim().to_string(),
            instrument_id: target.instrument_id.trim().to_string(),
            target_notional_portfolio: target_share,
            quantity,
            allocated_notional_portfolio: allocated_notional,
        });
    }

    if allocated_total > parent_notional_portfolio {
        return Err(AllocationError::Arithmetic("rounded child allocations exceed parent notional".into()));
    }
    Ok(EqualAllocation {
        parent_notional_portfolio,
        target_notional_per_account: target_share,
        children,
        allocated_notional_portfolio: allocated_total,
        unallocated_remainder_portfolio: parent_notional_portfolio - allocated_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal { Decimal::from_str(value).expect("valid decimal") }
    fn targets(count: usize, unit_value: &str, step: &str) -> Vec<AllocationTarget> {
        (0..count).map(|index| AllocationTarget {
            broker_id: format!("broker-{index}"),
            account_id: format!("account-{index}"),
            instrument_id: "EURUSD".into(),
            native_unit_value_portfolio: d(unit_value),
            quantity_step: d(step),
            minimum_quantity: d(step),
            minimum_notional_portfolio: None,
        }).collect()
    }

    #[test]
    fn three_accounts_receive_equal_target_notional() {
        let result = allocate_equal_notional(d("3000"), &targets(3, "100", "0.01")).unwrap();
        assert_eq!(result.target_notional_per_account, d("1000"));
        assert_eq!(result.children.len(), 3);
        assert!(result.children.iter().all(|child| child.target_notional_portfolio == d("1000")));
        assert_eq!(result.unallocated_remainder_portfolio, Decimal::ZERO);
    }

    #[test]
    fn fifteen_accounts_receive_equal_target_notional() {
        let result = allocate_equal_notional(d("15000"), &targets(15, "100", "0.01")).unwrap();
        assert_eq!(result.target_notional_per_account, d("1000"));
        assert_eq!(result.children.len(), 15);
        assert!(result.children.iter().all(|child| child.allocated_notional_portfolio == d("1000")));
    }

    #[test]
    fn quantity_step_residual_is_not_redistributed() {
        let result = allocate_equal_notional(d("100"), &targets(3, "3", "1")).unwrap();
        assert_eq!(result.target_notional_per_account, d("100") / d("3"));
        assert_eq!(result.allocated_notional_portfolio, d("99"));
        assert_eq!(result.unallocated_remainder_portfolio, d("1"));
    }

    #[test]
    fn heterogeneous_currency_conversions_produce_equal_budget_notional() {
        let mut recipients = targets(2, "100", "0.01");
        recipients[1].native_unit_value_portfolio = d("250");
        let result = allocate_equal_notional(d("1000"), &recipients).unwrap();
        assert_eq!(result.children[0].allocated_notional_portfolio, d("500"));
        assert_eq!(result.children[1].allocated_notional_portfolio, d("500"));
    }

    #[test]
    fn rejects_mixed_instruments_in_one_parent_allocation() {
        let mut recipients = targets(2, "10", "0.01");
        recipients[1].instrument_id = "GBPUSD".into();
        assert!(matches!(
            allocate_equal_notional(d("100"), &recipients),
            Err(AllocationError::InvalidTarget(_))
        ));
    }

    #[test]
    fn duplicate_broker_aliases_cannot_route_the_same_account_twice() {
        let mut recipients = targets(2, "10", "0.01");
        recipients[1].broker_id = " BROKER-0 ".into();
        recipients[1].account_id = " account-0 ".into();
        assert_eq!(allocate_equal_notional(d("100"), &recipients), Err(AllocationError::DuplicateAccount));
    }

    #[test]
    fn rejects_more_than_fifteen_recipients() {
        assert_eq!(allocate_equal_notional(d("100"), &targets(16, "1", "1")), Err(AllocationError::InvalidRecipientCount));
    }

    #[test]
    fn rejects_duplicate_broker_account_pairs() {
        let mut recipients = targets(2, "10", "1");
        recipients[1].broker_id = recipients[0].broker_id.clone();
        recipients[1].account_id = recipients[0].account_id.clone();
        assert_eq!(allocate_equal_notional(d("100"), &recipients), Err(AllocationError::DuplicateAccount));
    }
}
