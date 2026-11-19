//! Provider-neutral, deterministic order lifecycle state machine.
//!
//! This is the shared execution contract for future broker adapters. It does
//! not send orders or claim durable idempotency: adapters must persist the
//! client_order_id and processed execution IDs transactionally before enabling
//! live execution.

use std::collections::HashSet;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    PendingSubmit,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelPending,
    Cancelled,
    Rejected,
    Failed,
}

impl OrderStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Filled | Self::Cancelled | Self::Rejected | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrokerEvent {
    Accepted { broker_order_id: String },
    PartialFill { execution_id: String, quantity: Decimal, price: Decimal },
    Filled { execution_id: String, quantity: Decimal, price: Decimal },
    CancelRequested,
    Cancelled,
    Rejected { reason_code: String },
    Failed { reason_code: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleError {
    TerminalOrder,
    InvalidTransition,
    InvalidBrokerOrderId,
    InvalidExecution,
    Overfill,
    Arithmetic,
    MissingBrokerOrderId,
    MissingReason,
}

#[derive(Debug, Clone)]
pub struct OrderLifecycle {
    pub client_order_id: String,
    pub broker_order_id: Option<String>,
    pub status: OrderStatus,
    pub requested_quantity: Decimal,
    pub filled_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub last_reason_code: Option<String>,
    seen_execution_ids: HashSet<String>,
}

impl OrderLifecycle {
    pub fn new(client_order_id: String, requested_quantity: Decimal) -> Result<Self, LifecycleError> {
        if client_order_id.trim().is_empty()
            || client_order_id.len() > 128
            || requested_quantity <= Decimal::ZERO
        {
            return Err(LifecycleError::InvalidExecution);
        }
        Ok(Self {
            client_order_id,
            broker_order_id: None,
            status: OrderStatus::PendingSubmit,
            requested_quantity,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            last_reason_code: None,
            seen_execution_ids: HashSet::new(),
        })
    }

    /// Apply one broker event. Duplicate execution IDs are harmless no-ops.
    pub fn apply(&mut self, event: BrokerEvent) -> Result<bool, LifecycleError> {
        if self.status.is_terminal() {
            return Err(LifecycleError::TerminalOrder);
        }
        match event {
            BrokerEvent::Accepted { broker_order_id } => {
                if self.status != OrderStatus::PendingSubmit {
                    return Err(LifecycleError::InvalidTransition);
                }
                if broker_order_id.trim().is_empty() || broker_order_id.len() > 128 {
                    return Err(LifecycleError::InvalidBrokerOrderId);
                }
                self.broker_order_id = Some(broker_order_id);
                self.status = OrderStatus::Accepted;
            }
            BrokerEvent::PartialFill { execution_id, quantity, price } => {
                return self.apply_fill(execution_id, quantity, price, false);
            }
            BrokerEvent::Filled { execution_id, quantity, price } => {
                return self.apply_fill(execution_id, quantity, price, true);
            }
            BrokerEvent::CancelRequested => {
                if !matches!(self.status, OrderStatus::Accepted | OrderStatus::PartiallyFilled) {
                    return Err(LifecycleError::InvalidTransition);
                }
                self.status = OrderStatus::CancelPending;
            }
            BrokerEvent::Cancelled => {
                if !matches!(self.status, OrderStatus::Accepted | OrderStatus::PartiallyFilled | OrderStatus::CancelPending) {
                    return Err(LifecycleError::InvalidTransition);
                }
                self.status = OrderStatus::Cancelled;
            }
            BrokerEvent::Rejected { reason_code } => {
                if self.status != OrderStatus::PendingSubmit {
                    return Err(LifecycleError::InvalidTransition);
                }
                if reason_code.trim().is_empty() {
                    return Err(LifecycleError::MissingReason);
                }
                self.last_reason_code = Some(reason_code);
                self.status = OrderStatus::Rejected;
            }
            BrokerEvent::Failed { reason_code } => {
                if reason_code.trim().is_empty() {
                    return Err(LifecycleError::MissingReason);
                }
                self.last_reason_code = Some(reason_code);
                self.status = OrderStatus::Failed;
            }
        }
        Ok(true)
    }

    fn apply_fill(
        &mut self,
        execution_id: String,
        quantity: Decimal,
        price: Decimal,
        final_fill: bool,
    ) -> Result<bool, LifecycleError> {
        if !matches!(self.status, OrderStatus::Accepted | OrderStatus::PartiallyFilled | OrderStatus::CancelPending) {
            return Err(if self.status == OrderStatus::PendingSubmit {
                LifecycleError::MissingBrokerOrderId
            } else {
                LifecycleError::InvalidTransition
            });
        }
        if execution_id.trim().is_empty()
            || execution_id.len() > 128
            || quantity <= Decimal::ZERO
            || price <= Decimal::ZERO
        {
            return Err(LifecycleError::InvalidExecution);
        }
        if self.seen_execution_ids.contains(&execution_id) {
            return Ok(false);
        }
        let next_filled = self.filled_quantity.checked_add(quantity).ok_or(LifecycleError::Arithmetic)?;
        if next_filled > self.requested_quantity {
            return Err(LifecycleError::Overfill);
        }
        // Validate the complete transition before mutating state. A malformed
        // "filled" event must not leave a partially changed in-memory order.
        if final_fill && next_filled != self.requested_quantity {
            return Err(LifecycleError::InvalidExecution);
        }
        let previous_notional = self.average_fill_price.unwrap_or(Decimal::ZERO)
            .checked_mul(self.filled_quantity).ok_or(LifecycleError::Arithmetic)?;
        let fill_notional = price.checked_mul(quantity).ok_or(LifecycleError::Arithmetic)?;
        let total_notional = previous_notional.checked_add(fill_notional).ok_or(LifecycleError::Arithmetic)?;
        self.average_fill_price = Some(total_notional.checked_div(next_filled).ok_or(LifecycleError::Arithmetic)?);
        self.filled_quantity = next_filled;
        self.seen_execution_ids.insert(execution_id);
        self.status = if next_filled == self.requested_quantity {
            OrderStatus::Filled
        } else if self.status == OrderStatus::CancelPending {
            OrderStatus::CancelPending
        } else {
            OrderStatus::PartiallyFilled
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(value: &str) -> Decimal { value.parse().expect("valid test decimal") }

    fn accepted() -> OrderLifecycle {
        let mut order = OrderLifecycle::new("qoin-001".into(), d("10")).unwrap();
        order.apply(BrokerEvent::Accepted { broker_order_id: "broker-001".into() }).unwrap();
        order
    }

    #[test]
    fn tracks_partial_fills_and_weighted_average_price() {
        let mut order = accepted();
        order.apply(BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("4"), price: d("100") }).unwrap();
        order.apply(BrokerEvent::Filled { execution_id: "exec-2".into(), quantity: d("6"), price: d("110") }).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled_quantity, d("10"));
        assert_eq!(order.average_fill_price, Some(d("106")));
    }

    #[test]
    fn duplicate_execution_id_is_a_noop() {
        let mut order = accepted();
        let event = BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("4"), price: d("100") };
        assert!(order.apply(event.clone()).unwrap());
        assert!(!order.apply(event).unwrap());
        assert_eq!(order.filled_quantity, d("4"));
    }

    #[test]
    fn rejects_overfill_and_invalid_fill() {
        let mut order = accepted();
        assert_eq!(
            order.apply(BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("11"), price: d("100") }),
            Err(LifecycleError::Overfill)
        );
        assert_eq!(
            order.apply(BrokerEvent::PartialFill { execution_id: "exec-2".into(), quantity: Decimal::MAX, price: d("100") }),
            Err(LifecycleError::InvalidExecution)
        );
    }

    #[test]
    fn cancellation_preserves_partial_fill_and_is_terminal() {
        let mut order = accepted();
        order.apply(BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("3"), price: d("100") }).unwrap();
        order.apply(BrokerEvent::CancelRequested).unwrap();
        order.apply(BrokerEvent::Cancelled).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);
        assert_eq!(order.filled_quantity, d("3"));
        assert_eq!(order.apply(BrokerEvent::Failed { reason_code: "late".into() }), Err(LifecycleError::TerminalOrder));
    }

    #[test]
    fn late_fill_during_cancel_keeps_cancel_pending_unless_fully_filled() {
        let mut order = accepted();
        order.apply(BrokerEvent::CancelRequested).unwrap();
        order.apply(BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("3"), price: d("100") }).unwrap();
        assert_eq!(order.status, OrderStatus::CancelPending);
        order.apply(BrokerEvent::Filled { execution_id: "exec-2".into(), quantity: d("7"), price: d("101") }).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn rejects_fill_before_broker_acknowledgement() {
        let mut order = OrderLifecycle::new("qoin-002".into(), d("1")).unwrap();
        assert_eq!(
            order.apply(BrokerEvent::PartialFill { execution_id: "exec-1".into(), quantity: d("1"), price: d("1") }),
            Err(LifecycleError::MissingBrokerOrderId)
        );
    }

    #[test]
    fn filled_event_must_close_exact_remaining_quantity() {
        let mut order = accepted();
        assert_eq!(
            order.apply(BrokerEvent::Filled { execution_id: "exec-1".into(), quantity: d("2"), price: d("100") }),
            Err(LifecycleError::InvalidExecution)
        );
        assert_eq!(order.filled_quantity, Decimal::ZERO);
        assert_eq!(order.status, OrderStatus::Accepted);
    }
}
