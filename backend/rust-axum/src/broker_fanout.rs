//! Durable parent/child order intent model for multi-account execution.
//!
//! This module does not submit orders or write to MongoDB. The coordinator must
//! persist the parent and every child (with unique indexes) before calling
//! confirm_durable_persistence and begin_dispatch. Unknown broker outcomes are
//! reconciled, never blindly resubmitted.

use crate::broker_allocation_policy::{AggregateRiskApproval, BrokerCost, ExecutionPlan};
use crate::broker_contract::{OrderSide, OrderType};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use mongodb::{
    bson::{doc, Document},
    options::IndexOptions,
    Collection, IndexModel,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BrokerAccountKey {
    pub broker_id: String,
    pub account_id: String,
}

impl BrokerAccountKey {
    fn normalized(&self) -> Result<Self, FanoutError> {
        let broker_id = self.broker_id.trim().to_ascii_lowercase();
        let account_id = self.account_id.trim().to_string();
        if broker_id.is_empty() || account_id.is_empty() {
            return Err(FanoutError::InvalidSelection);
        }
        Ok(Self { broker_id, account_id })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentOrderRequest {
    /// Must be derived from a verified server-side user identity. This module
    /// does not authenticate callers or trust a client-supplied UID.
    pub owner_scope: String,
    pub idempotency_key: String,
    pub instrument_id: String,
    pub portfolio_currency: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub reduce_only: bool,
    pub parent_notional_portfolio: Decimal,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FanoutRouteMode {
    Consolidated,
    EqualSplit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParentOrderStatus {
    Prepared,
    Dispatching,
    PartiallySubmitted,
    ReconciliationRequired,
    Completed,
    PartiallyCompleted,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChildOrderStatus {
    Prepared,
    Submitting,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelPending,
    Cancelled,
    Rejected,
    Failed,
    Unknown,
}

impl ChildOrderStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Filled | Self::Cancelled | Self::Rejected | Self::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotRoutedAccount {
    pub account: BrokerAccountKey,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeCharge {
    pub execution_id: String,
    pub amount: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParentOrderIntent {
    pub parent_order_id: String,
    pub owner_scope: String,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub instrument_id: String,
    pub portfolio_currency: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub reduce_only: bool,
    pub parent_notional_portfolio: Decimal,
    pub allocated_notional_portfolio: Decimal,
    pub unallocated_remainder_portfolio: Decimal,
    pub route_mode: FanoutRouteMode,
    pub selected_accounts: Vec<BrokerAccountKey>,
    pub routed_accounts: Vec<BrokerAccountKey>,
    pub not_routed_accounts: Vec<NotRoutedAccount>,
    pub aggregate_risk_approval: AggregateRiskApproval,
    pub status: ParentOrderStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildOrderIntent {
    pub child_order_id: String,
    /// Stable broker client-order ID derived from parent idempotency + account.
    pub client_order_id: String,
    pub parent_order_id: String,
    pub broker_id: String,
    pub account_id: String,
    pub instrument_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub reduce_only: bool,
    /// Native broker quantity; the adapter maps this to its provider-specific unit.
    pub quantity: Decimal,
    pub target_notional_portfolio: Decimal,
    pub allocated_notional_portfolio: Decimal,
    pub status: ChildOrderStatus,
    pub broker_order_id: Option<String>,
    pub requested_quantity: Decimal,
    pub filled_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub fees: Vec<FeeCharge>,
    pub execution_ids: Vec<String>,
    pub last_reason: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChildEvent {
    Acknowledged { broker_order_id: String },
    SubmissionRejected { reason: String },
    SubmissionUnknown { reason: String },
    ReconciliationAccepted { broker_order_id: String },
    ReconciliationRejected { reason: String },
    PartialFill { execution_id: String, quantity: Decimal, price: Decimal, fee: Option<FeeCharge> },
    Filled { execution_id: String, quantity: Decimal, price: Decimal, fee: Option<FeeCharge> },
    CancelRequested,
    Cancelled,
    FailureConfirmed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanoutError {
    InvalidRequest,
    InvalidSelection,
    InvalidPlan(String),
    RiskApprovalMismatch,
    NotPersisted,
    DispatchAlreadyStarted,
    DispatchNotStarted,
    ChildNotFound,
    ChildAlreadySubmitted,
    InvalidTransition,
    InvalidAcknowledgement,
    InvalidExecution,
    Overfill,
    Arithmetic,
    MissingReason,
}

impl std::fmt::Display for FanoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FanoutError {}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let encoded = hex::encode(digest.finalize());
    format!("{prefix}-{}", &encoded[..32])
}


fn request_fingerprint(
    owner_scope: &str,
    idempotency_key: &str,
    instrument_id: &str,
    portfolio_currency: &str,
    side: OrderSide,
    order_type: OrderType,
    reduce_only: bool,
    parent_notional: Decimal,
    selected: &[BrokerAccountKey],
    route_mode: FanoutRouteMode,
    allocation: &crate::broker_allocation::EqualAllocation,
) -> String {
    let mut parts = vec![
        owner_scope.to_string(),
        idempotency_key.to_string(),
        instrument_id.to_string(),
        portfolio_currency.to_string(),
        format!("{side:?}"),
        format!("{order_type:?}"),
        reduce_only.to_string(),
        parent_notional.to_string(),
        format!("{route_mode:?}"),
        allocation.unallocated_remainder_portfolio.to_string(),
    ];
    let mut accounts = selected.to_vec();
    accounts.sort_by(|left, right| (&left.broker_id, &left.account_id).cmp(&(&right.broker_id, &right.account_id)));
    for account in accounts {
        parts.push(account.broker_id);
        parts.push(account.account_id);
    }
    let mut children = allocation.children.clone();
    children.sort_by(|left, right| (&left.broker_id, &left.account_id).cmp(&(&right.broker_id, &right.account_id)));
    for child in children {
        parts.push(child.broker_id);
        parts.push(child.account_id);
        parts.push(child.instrument_id);
        parts.push(child.quantity.to_string());
        parts.push(child.target_notional_portfolio.to_string());
        parts.push(child.allocated_notional_portfolio.to_string());
    }
    let references = parts.iter().map(String::as_str).collect::<Vec<_>>();
    stable_id("qof", &references)
}

#[derive(Debug)]
pub struct NewlyPersistedFanout {
    parent_order_id: String,
    request_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanoutPersistOutcome {
    Created,
    ExistingSameRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanoutPersistenceError {
    InvalidRecords,
    Database(String),
    IdempotencyConflict,
    DuplicateChildIdentity,
    ConcurrentModification,
    InvalidStoredRecords,
}

impl std::fmt::Display for FanoutPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FanoutPersistenceError {}

/// Store the complete parent and every child as one MongoDB document. Initial
/// creation is atomic even on a standalone MongoDB deployment. Existing
/// idempotent records are never treated as permission to dispatch again.
pub async fn persist_fanout_intents(
    collection: &Collection<Document>,
    parent: &ParentOrderIntent,
    children: &[ChildOrderIntent],
) -> Result<(FanoutPersistOutcome, Option<NewlyPersistedFanout>), FanoutPersistenceError> {
    if FanoutLifecycle::new(parent.clone(), children.to_vec()).is_err() {
        return Err(FanoutPersistenceError::InvalidRecords);
    }

    for (keys, name) in [
        (doc! { "parent_order_id": 1 }, "uq_fanout_parent"),
        (doc! { "owner_scope": 1, "idempotency_key": 1 }, "uq_fanout_idempotency"),
        (doc! { "children.client_order_id": 1 }, "uq_fanout_child_client_id"),
    ] {
        let model = IndexModel::builder()
            .keys(keys)
            .options(IndexOptions::builder().unique(true).name(name.to_string()).build())
            .build();
        collection.create_index(model, None).await
            .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    }

    let mut document = mongodb::bson::to_document(parent)
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    let child_array = mongodb::bson::to_bson(children)
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    document.insert("children", child_array);

    match collection.insert_one(document, None).await {
        Ok(_) => Ok((
            FanoutPersistOutcome::Created,
            Some(NewlyPersistedFanout {
                parent_order_id: parent.parent_order_id.clone(),
                request_fingerprint: parent.request_fingerprint.clone(),
            }),
        )),
        Err(error) if error.to_string().contains("E11000") => {
            let existing = collection.find_one(doc! {
                "owner_scope": &parent.owner_scope,
                "idempotency_key": &parent.idempotency_key,
            }, None).await
                .map_err(|lookup| FanoutPersistenceError::Database(lookup.to_string()))?;
            let Some(existing) = existing else {
                return Err(FanoutPersistenceError::DuplicateChildIdentity);
            };
            if existing.get_str("request_fingerprint").ok() == Some(parent.request_fingerprint.as_str()) {
                Ok((FanoutPersistOutcome::ExistingSameRequest, None))
            } else {
                Err(FanoutPersistenceError::IdempotencyConflict)
            }
        }
        Err(error) => Err(FanoutPersistenceError::Database(error.to_string())),
    }
}

fn validate_cost_for_child(cost: &BrokerCost, child: &crate::broker_allocation::ChildAllocation) -> Result<(), FanoutError> {
    let component_total = cost.estimated_fee_portfolio
        .checked_add(cost.estimated_slippage_portfolio)
        .and_then(|value| value.checked_add(cost.estimated_conversion_cost_portfolio))
        .ok_or(FanoutError::Arithmetic)?;
    if cost.broker_id.trim().to_ascii_lowercase() != child.broker_id.trim().to_ascii_lowercase()
        || cost.account_id.trim() != child.account_id.trim()
        || cost.allocated_notional_portfolio != child.allocated_notional_portfolio
        || cost.estimated_fee_portfolio < Decimal::ZERO
        || cost.estimated_slippage_portfolio < Decimal::ZERO
        || cost.estimated_conversion_cost_portfolio < Decimal::ZERO
        || cost.estimated_margin_required_portfolio < Decimal::ZERO
        || cost.total_estimated_execution_cost_portfolio != component_total
    {
        return Err(FanoutError::InvalidPlan(
            "broker cost estimate does not match its child allocation or component totals".into()
        ));
    }
    Ok(())
}

/// Build parent and child records from a risk-approved route. A consolidated
/// plan records every selected account not used by the single chosen route;
/// an equal split must cover the complete selected account set exactly once.
pub fn build_fanout_intents(
    request: ParentOrderRequest,
    selected_accounts: &[BrokerAccountKey],
    plan: &ExecutionPlan,
    risk_approval: AggregateRiskApproval,
) -> Result<(ParentOrderIntent, Vec<ChildOrderIntent>), FanoutError> {
    let owner_scope = request.owner_scope.trim().to_string();
    let idempotency_key = request.idempotency_key.trim().to_string();
    let instrument_id = request.instrument_id.trim().to_string();
    let portfolio_currency = request.portfolio_currency.trim().to_ascii_uppercase();
    if owner_scope.is_empty() || owner_scope.len() > 256
        || idempotency_key.len() < 8 || idempotency_key.len() > 128
        || instrument_id.is_empty() || instrument_id.len() > 96
        || portfolio_currency.len() < 3 || portfolio_currency.len() > 12
        || request.parent_notional_portfolio <= Decimal::ZERO
        || request.created_at_ms <= 0
    {
        return Err(FanoutError::InvalidRequest);
    }
    if risk_approval.instrument_id != instrument_id
        || risk_approval.parent_notional_portfolio != request.parent_notional_portfolio
        || risk_approval.reduce_only != request.reduce_only
        || risk_approval.approved_at_ms < request.created_at_ms
        || risk_approval.approved_at_ms <= 0
    {
        return Err(FanoutError::RiskApprovalMismatch);
    }
    if selected_accounts.is_empty() || selected_accounts.len() > 15 {
        return Err(FanoutError::InvalidSelection);
    }
    let selected = selected_accounts.iter().map(BrokerAccountKey::normalized).collect::<Result<Vec<_>, _>>()?;
    let mut selected_set = HashSet::with_capacity(selected.len());
    for key in &selected {
        if !selected_set.insert(key.clone()) {
            return Err(FanoutError::InvalidSelection);
        }
    }

    let (route_mode, allocation) = match plan {
        ExecutionPlan::Consolidated { broker_id, account_id, allocation, cost, .. } => {
            if allocation.children.len() != 1
                || allocation.children[0].broker_id.trim().to_ascii_lowercase() != broker_id.trim().to_ascii_lowercase()
                || allocation.children[0].account_id.trim() != account_id.trim()
            {
                return Err(FanoutError::InvalidPlan("consolidated plan must contain exactly its selected account".into()));
            }
            validate_cost_for_child(cost, &allocation.children[0])?;
            (FanoutRouteMode::Consolidated, allocation)
        }
        ExecutionPlan::EqualSplit { allocation, costs, single_broker_comparator_cost_portfolio, .. } => {
            if allocation.children.len() != selected.len() || costs.len() != selected.len() {
                return Err(FanoutError::InvalidPlan("equal split must include every selected account and its cost estimate".into()));
            }
            if single_broker_comparator_cost_portfolio.is_some_and(|value| value < Decimal::ZERO) {
                return Err(FanoutError::InvalidPlan("single-broker comparator cost cannot be negative".into()));
            }
            let mut cost_accounts = HashSet::with_capacity(costs.len());
            for cost in costs {
                let child = allocation.children.iter().find(|child|
                    child.broker_id.trim().eq_ignore_ascii_case(cost.broker_id.trim())
                        && child.account_id.trim() == cost.account_id.trim()
                ).ok_or_else(|| FanoutError::InvalidPlan("cost estimate references an account outside the split".into()))?;
                validate_cost_for_child(cost, child)?;
                let key = BrokerAccountKey {
                    broker_id: cost.broker_id.trim().to_ascii_lowercase(),
                    account_id: cost.account_id.trim().to_string(),
                };
                if !cost_accounts.insert(key) {
                    return Err(FanoutError::InvalidPlan("split contains duplicate account cost estimates".into()));
                }
            }
            (FanoutRouteMode::EqualSplit, allocation)
        }
    };
    if allocation.parent_notional_portfolio != request.parent_notional_portfolio
        || allocation.children.is_empty()
        || allocation.children.iter().any(|child| child.instrument_id.trim() != instrument_id
            || child.quantity <= Decimal::ZERO
            || child.allocated_notional_portfolio <= Decimal::ZERO)
    {
        return Err(FanoutError::InvalidPlan("allocation does not match the parent instrument or notional".into()));
    }
    let allocated_plus_remainder = allocation.allocated_notional_portfolio
        .checked_add(allocation.unallocated_remainder_portfolio)
        .ok_or(FanoutError::Arithmetic)?;
    if allocation.unallocated_remainder_portfolio < Decimal::ZERO
        || allocated_plus_remainder != request.parent_notional_portfolio
        || allocation.allocated_notional_portfolio > request.parent_notional_portfolio
    {
        return Err(FanoutError::InvalidPlan("allocation exceeds the parent or loses its rounding remainder".into()));
    }

    let mut routed = Vec::with_capacity(allocation.children.len());
    let mut routed_set = HashSet::with_capacity(allocation.children.len());
    for child in &allocation.children {
        let key = BrokerAccountKey {
            broker_id: child.broker_id.trim().to_ascii_lowercase(),
            account_id: child.account_id.trim().to_string(),
        };
        if !selected_set.contains(&key) || !routed_set.insert(key.clone()) {
            return Err(FanoutError::InvalidPlan("plan routes an unselected or duplicate account".into()));
        }
        routed.push(key);
    }
    if route_mode == FanoutRouteMode::EqualSplit && routed_set != selected_set {
        return Err(FanoutError::InvalidPlan("equal split silently omits a selected account".into()));
    }

    let not_routed_accounts = selected.iter().filter(|key| !routed_set.contains(*key))
        .map(|key| NotRoutedAccount {
            account: key.clone(),
            reason: match route_mode {
                FanoutRouteMode::Consolidated => format!(
                    "Consolidated into {}/{}; this selected account receives no child order",
                    routed[0].broker_id, routed[0].account_id
                ),
                FanoutRouteMode::EqualSplit => "Not routed".into(),
            },
        }).collect::<Vec<_>>();
    let parent_order_id = stable_id("qop", &[&owner_scope, &idempotency_key]);
    let mut children = Vec::with_capacity(allocation.children.len());
    for child in &allocation.children {
        let broker_id = child.broker_id.trim().to_ascii_lowercase();
        let account_id = child.account_id.trim().to_string();
        let child_order_id = stable_id("qoc", &[
            &parent_order_id, &broker_id, &account_id, &instrument_id
        ]);
        children.push(ChildOrderIntent {
            child_order_id: child_order_id.clone(),
            client_order_id: child_order_id,
            parent_order_id: parent_order_id.clone(),
            broker_id,
            account_id,
            instrument_id: instrument_id.clone(),
            side: request.side,
            order_type: request.order_type,
            reduce_only: request.reduce_only,
            quantity: child.quantity,
            target_notional_portfolio: child.target_notional_portfolio,
            allocated_notional_portfolio: child.allocated_notional_portfolio,
            status: ChildOrderStatus::Prepared,
            broker_order_id: None,
            requested_quantity: child.quantity,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            fees: Vec::new(),
            execution_ids: Vec::new(),
            last_reason: None,
            created_at_ms: request.created_at_ms,
            updated_at_ms: request.created_at_ms,
            revision: 0,
        });
    }
    let request_fingerprint = request_fingerprint(
        &owner_scope,
        &idempotency_key,
        &instrument_id,
        &portfolio_currency,
        request.side,
        request.order_type,
        request.reduce_only,
        request.parent_notional_portfolio,
        &selected,
        route_mode,
        allocation,
    );
    let parent = ParentOrderIntent {
        parent_order_id,
        owner_scope,
        idempotency_key,
        request_fingerprint,
        instrument_id,
        portfolio_currency,
        side: request.side,
        order_type: request.order_type,
        reduce_only: request.reduce_only,
        parent_notional_portfolio: request.parent_notional_portfolio,
        allocated_notional_portfolio: allocation.allocated_notional_portfolio,
        unallocated_remainder_portfolio: allocation.unallocated_remainder_portfolio,
        route_mode,
        selected_accounts: selected,
        routed_accounts: routed,
        not_routed_accounts,
        aggregate_risk_approval: risk_approval,
        status: ParentOrderStatus::Prepared,
        created_at_ms: request.created_at_ms,
        updated_at_ms: request.created_at_ms,
        revision: 0,
    };
    Ok((parent, children))
}


/// Reload a persisted parent and all of its child records. Dispatch state is
/// reconstructed from durable statuses so a process restart cannot reset the
/// one-shot dispatch guard or authorize a duplicate submission.
pub async fn load_fanout_lifecycle(
    collection: &Collection<Document>,
    owner_scope: &str,
    parent_order_id: &str,
) -> Result<Option<FanoutLifecycle>, FanoutPersistenceError> {
    if owner_scope.trim().is_empty() || parent_order_id.trim().is_empty() {
        return Err(FanoutPersistenceError::InvalidRecords);
    }
    let Some(mut document) = collection.find_one(doc! {
        "owner_scope": owner_scope,
        "parent_order_id": parent_order_id,
    }, None).await
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?
    else {
        return Ok(None);
    };
    let child_records = document.remove("children")
        .ok_or(FanoutPersistenceError::InvalidStoredRecords)?;
    let children = mongodb::bson::from_bson::<Vec<ChildOrderIntent>>(child_records)
        .map_err(|_| FanoutPersistenceError::InvalidStoredRecords)?;
    let parent = mongodb::bson::from_document::<ParentOrderIntent>(document)
        .map_err(|_| FanoutPersistenceError::InvalidStoredRecords)?;
    FanoutLifecycle::restore(parent, children)
        .map(Some)
        .map_err(|_| FanoutPersistenceError::InvalidStoredRecords)
}

/// Persist a parent-only transition (for example, dispatch-started) with
/// optimistic concurrency. A zero match means another worker changed it;
/// callers must reload the parent and reconcile rather than overwrite state.
pub async fn persist_parent_lifecycle_update(
    collection: &Collection<Document>,
    parent: &ParentOrderIntent,
    expected_revision: i64,
) -> Result<(), FanoutPersistenceError> {
    if parent.revision != expected_revision.checked_add(1).ok_or(FanoutPersistenceError::InvalidRecords)? {
        return Err(FanoutPersistenceError::InvalidRecords);
    }
    let status = mongodb::bson::to_bson(&parent.status)
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    let result = collection.update_one(
        doc! {
            "owner_scope": &parent.owner_scope,
            "parent_order_id": &parent.parent_order_id,
            "revision": expected_revision,
        },
        doc! { "$set": {
            "status": status,
            "updated_at_ms": parent.updated_at_ms,
            "revision": parent.revision,
        }},
        None,
    ).await.map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    if result.matched_count != 1 {
        return Err(FanoutPersistenceError::ConcurrentModification);
    }
    Ok(())
}

/// Persist a child transition and its derived parent status in one atomic
/// Mongo document update. Expected revisions prevent stale workers overwriting
/// newer fills, acknowledgements or reconciliation results.
pub async fn persist_child_lifecycle_update(
    collection: &Collection<Document>,
    parent: &ParentOrderIntent,
    child: &ChildOrderIntent,
    expected_parent_revision: i64,
    expected_child_revision: i64,
) -> Result<(), FanoutPersistenceError> {
    if parent.revision != expected_parent_revision.checked_add(1).ok_or(FanoutPersistenceError::InvalidRecords)?
        || child.revision != expected_child_revision.checked_add(1).ok_or(FanoutPersistenceError::InvalidRecords)?
        || child.parent_order_id != parent.parent_order_id
    {
        return Err(FanoutPersistenceError::InvalidRecords);
    }
    let child_document = mongodb::bson::to_document(child)
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    let status = mongodb::bson::to_bson(&parent.status)
        .map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    let result = collection.update_one(
        doc! {
            "owner_scope": &parent.owner_scope,
            "parent_order_id": &parent.parent_order_id,
            "revision": expected_parent_revision,
            "children": { "$elemMatch": {
                "client_order_id": &child.client_order_id,
                "revision": expected_child_revision,
            }},
        },
        doc! { "$set": {
            "children.$": child_document,
            "status": status,
            "updated_at_ms": parent.updated_at_ms,
            "revision": parent.revision,
        }},
        None,
    ).await.map_err(|error| FanoutPersistenceError::Database(error.to_string()))?;
    if result.matched_count != 1 {
        return Err(FanoutPersistenceError::ConcurrentModification);
    }
    Ok(())
}

/// In-memory state transition guard for a coordinator that persists the
/// returned records before calling confirm_durable_persistence. It is not a
/// substitute for a database transaction or durable unique indexes.
pub struct FanoutLifecycle {
    pub parent: ParentOrderIntent,
    pub children: Vec<ChildOrderIntent>,
    persistence_confirmed: bool,
    dispatch_started: bool,
}

impl FanoutLifecycle {
    pub fn new(parent: ParentOrderIntent, children: Vec<ChildOrderIntent>) -> Result<Self, FanoutError> {
        if children.is_empty()
            || children.len() != parent.routed_accounts.len()
            || parent.request_fingerprint.is_empty()
            || parent.status != ParentOrderStatus::Prepared
            || parent.revision != 0
        {
            return Err(FanoutError::InvalidPlan("parent and child records do not match".into()));
        }
        let mut child_keys = HashSet::with_capacity(children.len());
        let mut child_ids = HashSet::with_capacity(children.len());
        for child in &children {
            let key = BrokerAccountKey {
                broker_id: child.broker_id.trim().to_ascii_lowercase(),
                account_id: child.account_id.trim().to_string(),
            };
            if child.parent_order_id != parent.parent_order_id
                || child.status != ChildOrderStatus::Prepared
                || child.quantity <= Decimal::ZERO
                || child.requested_quantity != child.quantity
                || child.instrument_id != parent.instrument_id
                || child.reduce_only != parent.reduce_only
                || child.revision != 0
                || !child_keys.insert(key)
                || !child_ids.insert(child.client_order_id.clone())
            {
                return Err(FanoutError::InvalidPlan("child identity, quantity or initial state is invalid".into()));
            }
        }
        let routed = parent.routed_accounts.iter().cloned().collect::<HashSet<_>>();
        if child_keys != routed {
            return Err(FanoutError::InvalidPlan("child records do not cover the routed accounts exactly".into()));
        }
        Ok(Self { parent, children, persistence_confirmed: false, dispatch_started: false })
    }

    /// Restore an already persisted lifecycle after process restart. Unlike
    /// new-intent construction, this accepts non-initial states but validates
    /// their internal consistency before enabling any further transition.
    pub fn restore(parent: ParentOrderIntent, children: Vec<ChildOrderIntent>) -> Result<Self, FanoutError> {
        if children.is_empty()
            || children.len() != parent.routed_accounts.len()
            || parent.selected_accounts.is_empty()
            || parent.selected_accounts.len() > 15
            || parent.parent_order_id.trim().is_empty()
            || parent.owner_scope.trim().is_empty()
            || parent.idempotency_key.trim().is_empty()
            || parent.request_fingerprint.trim().is_empty()
            || parent.instrument_id.trim().is_empty()
            || parent.portfolio_currency.trim().is_empty()
            || parent.parent_notional_portfolio <= Decimal::ZERO
            || parent.revision < 0
            || parent.created_at_ms <= 0
            || parent.updated_at_ms < parent.created_at_ms
            || parent.allocated_notional_portfolio < Decimal::ZERO
            || parent.unallocated_remainder_portfolio < Decimal::ZERO
            || parent.allocated_notional_portfolio
                .checked_add(parent.unallocated_remainder_portfolio) != Some(parent.parent_notional_portfolio)
            || parent.aggregate_risk_approval.instrument_id != parent.instrument_id
            || parent.aggregate_risk_approval.parent_notional_portfolio != parent.parent_notional_portfolio
            || parent.aggregate_risk_approval.reduce_only != parent.reduce_only
        {
            return Err(FanoutError::InvalidPlan("persisted parent record is inconsistent".into()));
        }

        let selected = parent.selected_accounts.iter()
            .map(BrokerAccountKey::normalized).collect::<Result<Vec<_>, _>>()?;
        let routed = parent.routed_accounts.iter()
            .map(BrokerAccountKey::normalized).collect::<Result<Vec<_>, _>>()?;
        let not_routed = parent.not_routed_accounts.iter().map(|item| {
            if item.reason.trim().is_empty() {
                return Err(FanoutError::InvalidPlan("persisted not-routed account has no reason".into()));
            }
            item.account.normalized()
        }).collect::<Result<Vec<_>, _>>()?;
        let selected_set = selected.iter().cloned().collect::<HashSet<_>>();
        let routed_set = routed.iter().cloned().collect::<HashSet<_>>();
        let not_routed_set = not_routed.iter().cloned().collect::<HashSet<_>>();
        if selected_set.len() != selected.len()
            || routed_set.len() != routed.len()
            || not_routed_set.len() != not_routed.len()
            || routed_set.len() != children.len()
            || !routed_set.is_disjoint(&not_routed_set)
            || routed_set.union(&not_routed_set).cloned().collect::<HashSet<_>>() != selected_set
            || (parent.route_mode == FanoutRouteMode::EqualSplit && !not_routed_set.is_empty())
            || (parent.route_mode == FanoutRouteMode::Consolidated && routed.len() != 1)
        {
            return Err(FanoutError::InvalidPlan("persisted selected, routed and not-routed accounts do not form a valid route".into()));
        }

        let mut child_keys = HashSet::with_capacity(children.len());
        let mut child_ids = HashSet::with_capacity(children.len());
        for child in &children {
            let key = BrokerAccountKey {
                broker_id: child.broker_id.trim().to_ascii_lowercase(),
                account_id: child.account_id.trim().to_string(),
            };
            if child.parent_order_id != parent.parent_order_id
                || child.broker_id.trim().is_empty()
                || child.account_id.trim().is_empty()
                || child.client_order_id.trim().is_empty()
                || child.child_order_id != child.client_order_id
                || child.instrument_id != parent.instrument_id
                || child.reduce_only != parent.reduce_only
                || child.quantity <= Decimal::ZERO
                || child.target_notional_portfolio <= Decimal::ZERO
                || child.allocated_notional_portfolio <= Decimal::ZERO
                || child.requested_quantity != child.quantity
                || child.filled_quantity < Decimal::ZERO
                || child.filled_quantity > child.requested_quantity
                || child.revision < 0
                || child.created_at_ms != parent.created_at_ms
                || child.updated_at_ms < child.created_at_ms
                || child.updated_at_ms > parent.updated_at_ms
                || (child.status == ChildOrderStatus::Filled && child.filled_quantity != child.requested_quantity)
                || (child.status == ChildOrderStatus::PartiallyFilled
                    && (child.filled_quantity <= Decimal::ZERO || child.filled_quantity >= child.requested_quantity))
                || (matches!(child.status, ChildOrderStatus::Accepted | ChildOrderStatus::PartiallyFilled | ChildOrderStatus::Filled | ChildOrderStatus::CancelPending | ChildOrderStatus::Cancelled)
                    && child.broker_order_id.as_ref().map_or(true, |id| id.trim().is_empty()))
                || (matches!(child.status, ChildOrderStatus::Unknown | ChildOrderStatus::Rejected | ChildOrderStatus::Failed)
                    && child.last_reason.as_ref().map_or(true, |reason| reason.trim().is_empty()))
                || (child.filled_quantity == Decimal::ZERO && child.average_fill_price.is_some())
                || (child.filled_quantity > Decimal::ZERO
                    && child.average_fill_price.map_or(true, |price| price <= Decimal::ZERO))
                || (matches!(child.status, ChildOrderStatus::Prepared | ChildOrderStatus::Submitting | ChildOrderStatus::Accepted | ChildOrderStatus::Unknown | ChildOrderStatus::Rejected | ChildOrderStatus::Failed)
                    && child.filled_quantity > Decimal::ZERO)
                || (child.status == ChildOrderStatus::Cancelled
                    && child.filled_quantity == child.requested_quantity)
                || !child_keys.insert(key)
                || !child_ids.insert(child.client_order_id.clone())
            {
                return Err(FanoutError::InvalidPlan("persisted child record is inconsistent".into()));
            }
            let mut execution_ids = HashSet::with_capacity(child.execution_ids.len());
            if child.execution_ids.iter().any(|id| id.trim().is_empty() || !execution_ids.insert(id.clone())) {
                return Err(FanoutError::InvalidPlan("persisted execution IDs are invalid or duplicated".into()));
            }
            if child.fees.iter().any(|fee| fee.amount < Decimal::ZERO
                || fee.currency.trim().is_empty()
                || !execution_ids.contains(&fee.execution_id))
            {
                return Err(FanoutError::InvalidPlan("persisted fee records do not match executions".into()));
            }
        }
        if child_keys != routed_set {
            return Err(FanoutError::InvalidPlan("persisted children do not cover routed accounts exactly".into()));
        }

        let dispatch_started = parent.status != ParentOrderStatus::Prepared
            || children.iter().any(|child| child.status != ChildOrderStatus::Prepared);
        let mut lifecycle = Self {
            parent,
            children,
            persistence_confirmed: true,
            dispatch_started,
        };
        let stored_status = lifecycle.parent.status;
        lifecycle.refresh_parent_status();
        if lifecycle.parent.status != stored_status {
            return Err(FanoutError::InvalidPlan("persisted parent status does not match child states".into()));
        }
        Ok(lifecycle)
    }

    /// Accept only a receipt returned after a successful atomic Mongo insert.
    /// An existing idempotent record cannot authorize a second dispatch.
    pub fn confirm_durable_persistence(&mut self, receipt: NewlyPersistedFanout) -> Result<(), FanoutError> {
        if receipt.parent_order_id != self.parent.parent_order_id
            || receipt.request_fingerprint != self.parent.request_fingerprint
        {
            return Err(FanoutError::NotPersisted);
        }
        self.persistence_confirmed = true;
        Ok(())
    }

    pub fn begin_dispatch(&mut self, now_ms: i64) -> Result<(), FanoutError> {
        if !self.persistence_confirmed {
            return Err(FanoutError::NotPersisted);
        }
        if self.dispatch_started {
            return Err(FanoutError::DispatchAlreadyStarted);
        }
        if now_ms < self.parent.created_at_ms {
            return Err(FanoutError::InvalidTransition);
        }
        self.dispatch_started = true;
        self.parent.status = ParentOrderStatus::Dispatching;
        self.parent.updated_at_ms = now_ms;
        self.parent.revision = self.parent.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        Ok(())
    }

    pub fn begin_child_submission(&mut self, child_order_id: &str, now_ms: i64) -> Result<(), FanoutError> {
        if !self.dispatch_started {
            return Err(FanoutError::DispatchNotStarted);
        }
        let child = self.children.iter_mut().find(|child| child.child_order_id == child_order_id)
            .ok_or(FanoutError::ChildNotFound)?;
        if child.status != ChildOrderStatus::Prepared {
            return Err(FanoutError::ChildAlreadySubmitted);
        }
        if now_ms < child.created_at_ms {
            return Err(FanoutError::InvalidTransition);
        }
        child.status = ChildOrderStatus::Submitting;
        child.updated_at_ms = now_ms;
        child.revision = child.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        self.parent.updated_at_ms = now_ms;
        self.parent.revision = self.parent.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        Ok(())
    }

    pub fn apply_child_event(
        &mut self,
        child_order_id: &str,
        event: ChildEvent,
        now_ms: i64,
    ) -> Result<bool, FanoutError> {
        let child = self.children.iter_mut().find(|child| child.child_order_id == child_order_id)
            .ok_or(FanoutError::ChildNotFound)?;
        if now_ms < child.updated_at_ms {
            return Err(FanoutError::InvalidTransition);
        }
        let changed = child.apply(event, now_ms)?;
        if changed {
            self.refresh_parent_status();
            self.parent.updated_at_ms = now_ms;
            self.parent.revision = self.parent.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        }
        Ok(changed)
    }

    fn refresh_parent_status(&mut self) {
        if self.children.iter().any(|child| child.status == ChildOrderStatus::Unknown) {
            self.parent.status = ParentOrderStatus::ReconciliationRequired;
            return;
        }
        if self.children.iter().all(|child| child.status == ChildOrderStatus::Filled) {
            self.parent.status = ParentOrderStatus::Completed;
            return;
        }
        if self.children.iter().all(|child| child.status.is_terminal()) {
            self.parent.status = if self.children.iter().all(|child| child.status == ChildOrderStatus::Cancelled) {
                ParentOrderStatus::Cancelled
            } else if self.children.iter().any(|child| child.filled_quantity > Decimal::ZERO) {
                ParentOrderStatus::PartiallyCompleted
            } else {
                ParentOrderStatus::Failed
            };
            return;
        }
        let has_failure = self.children.iter().any(|child| matches!(
            child.status, ChildOrderStatus::Rejected | ChildOrderStatus::Failed | ChildOrderStatus::Cancelled
        ));
        let has_execution = self.children.iter().any(|child| matches!(
            child.status, ChildOrderStatus::Accepted | ChildOrderStatus::PartiallyFilled | ChildOrderStatus::Filled
        ));
        self.parent.status = if has_failure && has_execution {
            ParentOrderStatus::PartiallySubmitted
        } else if self.dispatch_started {
            ParentOrderStatus::Dispatching
        } else {
            ParentOrderStatus::Prepared
        };
    }
}

impl ChildOrderIntent {
    fn apply(&mut self, event: ChildEvent, now_ms: i64) -> Result<bool, FanoutError> {
        if self.status.is_terminal() {
            return Err(FanoutError::InvalidTransition);
        }
        match event {
            ChildEvent::Acknowledged { broker_order_id } => {
                if self.status != ChildOrderStatus::Submitting
                    || broker_order_id.trim().is_empty()
                    || broker_order_id.len() > 128
                {
                    return Err(FanoutError::InvalidAcknowledgement);
                }
                self.broker_order_id = Some(broker_order_id);
                self.status = ChildOrderStatus::Accepted;
            }
            ChildEvent::SubmissionRejected { reason } => {
                if self.status != ChildOrderStatus::Submitting { return Err(FanoutError::InvalidTransition); }
                if reason.trim().is_empty() { return Err(FanoutError::MissingReason); }
                self.last_reason = Some(reason);
                self.status = ChildOrderStatus::Rejected;
            }
            ChildEvent::SubmissionUnknown { reason } => {
                if self.status != ChildOrderStatus::Submitting { return Err(FanoutError::InvalidTransition); }
                if reason.trim().is_empty() { return Err(FanoutError::MissingReason); }
                self.last_reason = Some(reason);
                self.status = ChildOrderStatus::Unknown;
            }
            ChildEvent::ReconciliationAccepted { broker_order_id } => {
                if self.status != ChildOrderStatus::Unknown
                    || broker_order_id.trim().is_empty()
                    || broker_order_id.len() > 128
                {
                    return Err(FanoutError::InvalidAcknowledgement);
                }
                self.broker_order_id = Some(broker_order_id);
                self.status = ChildOrderStatus::Accepted;
            }
            ChildEvent::ReconciliationRejected { reason } => {
                if self.status != ChildOrderStatus::Unknown { return Err(FanoutError::InvalidTransition); }
                if reason.trim().is_empty() { return Err(FanoutError::MissingReason); }
                self.last_reason = Some(reason);
                self.status = ChildOrderStatus::Rejected;
            }
            ChildEvent::PartialFill { execution_id, quantity, price, fee } => {
                return self.apply_fill(execution_id, quantity, price, fee, false, now_ms);
            }
            ChildEvent::Filled { execution_id, quantity, price, fee } => {
                return self.apply_fill(execution_id, quantity, price, fee, true, now_ms);
            }
            ChildEvent::CancelRequested => {
                if !matches!(self.status, ChildOrderStatus::Accepted | ChildOrderStatus::PartiallyFilled) {
                    return Err(FanoutError::InvalidTransition);
                }
                self.status = ChildOrderStatus::CancelPending;
            }
            ChildEvent::Cancelled => {
                if !matches!(self.status, ChildOrderStatus::Accepted | ChildOrderStatus::PartiallyFilled | ChildOrderStatus::CancelPending) {
                    return Err(FanoutError::InvalidTransition);
                }
                self.status = ChildOrderStatus::Cancelled;
            }
            ChildEvent::FailureConfirmed { reason } => {
                if self.status != ChildOrderStatus::Submitting { return Err(FanoutError::InvalidTransition); }
                if reason.trim().is_empty() { return Err(FanoutError::MissingReason); }
                self.last_reason = Some(reason);
                self.status = ChildOrderStatus::Failed;
            }
        }
        self.updated_at_ms = now_ms;
        self.revision = self.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        Ok(true)
    }

    fn apply_fill(
        &mut self,
        execution_id: String,
        quantity: Decimal,
        price: Decimal,
        fee: Option<FeeCharge>,
        final_fill: bool,
        now_ms: i64,
    ) -> Result<bool, FanoutError> {
        if !matches!(self.status, ChildOrderStatus::Accepted | ChildOrderStatus::PartiallyFilled | ChildOrderStatus::CancelPending) {
            return Err(FanoutError::InvalidTransition);
        }
        if execution_id.trim().is_empty() || execution_id.len() > 128
            || quantity <= Decimal::ZERO || price <= Decimal::ZERO
        {
            return Err(FanoutError::InvalidExecution);
        }
        if self.execution_ids.iter().any(|seen| seen == &execution_id) {
            return Ok(false);
        }
        if let Some(charge) = &fee {
            if charge.execution_id != execution_id || charge.amount < Decimal::ZERO || charge.currency.trim().is_empty() {
                return Err(FanoutError::InvalidExecution);
            }
        }
        let next_filled = self.filled_quantity.checked_add(quantity).ok_or(FanoutError::Arithmetic)?;
        if next_filled > self.requested_quantity { return Err(FanoutError::Overfill); }
        if final_fill && next_filled != self.requested_quantity { return Err(FanoutError::InvalidExecution); }
        let previous_value = self.average_fill_price.unwrap_or(Decimal::ZERO)
            .checked_mul(self.filled_quantity).ok_or(FanoutError::Arithmetic)?;
        let fill_value = price.checked_mul(quantity).ok_or(FanoutError::Arithmetic)?;
        let average = previous_value.checked_add(fill_value)
            .and_then(|value| value.checked_div(next_filled)).ok_or(FanoutError::Arithmetic)?;
        self.filled_quantity = next_filled;
        self.average_fill_price = Some(average);
        self.execution_ids.push(execution_id);
        if let Some(charge) = fee { self.fees.push(charge); }
        self.status = if next_filled == self.requested_quantity {
            ChildOrderStatus::Filled
        } else if self.status == ChildOrderStatus::CancelPending {
            ChildOrderStatus::CancelPending
        } else {
            ChildOrderStatus::PartiallyFilled
        };
        self.updated_at_ms = now_ms;
        self.revision = self.revision.checked_add(1).ok_or(FanoutError::Arithmetic)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker_allocation::{ChildAllocation, EqualAllocation};
    use crate::broker_allocation_policy::{AggregateRiskApproval, BrokerCost};

    fn d(value: &str) -> Decimal { value.parse().unwrap() }

    fn request() -> ParentOrderRequest {
        ParentOrderRequest {
            owner_scope: "verified-user-scope".into(),
            idempotency_key: "parent-key-0001".into(),
            instrument_id: "EURUSD".into(),
            portfolio_currency: "KES".into(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            reduce_only: false,
            parent_notional_portfolio: d("100"),
            created_at_ms: 10_000,
        }
    }

    fn risk() -> AggregateRiskApproval {
        AggregateRiskApproval {
            instrument_id: "EURUSD".into(),
            parent_notional_portfolio: d("100"),
            maximum_loss_at_stop_portfolio: d("1"),
            reduce_only: false,
            projected_gross_exposure_portfolio: d("200"),
            projected_daily_loss_portfolio: d("120"),
            remaining_daily_loss_budget_portfolio: d("10"),
            snapshot_observed_at_ms: 9_900,
            market_data_observed_at_ms: 9_950,
            approved_at_ms: 10_000,
        }
    }

    fn selected() -> Vec<BrokerAccountKey> {
        vec![
            BrokerAccountKey { broker_id: "broker-a".into(), account_id: "account-a".into() },
            BrokerAccountKey { broker_id: "broker-b".into(), account_id: "account-b".into() },
        ]
    }

    fn child(broker: &str, account: &str, quantity: &str, allocated: &str) -> ChildAllocation {
        ChildAllocation {
            broker_id: broker.into(),
            account_id: account.into(),
            instrument_id: "EURUSD".into(),
            target_notional_portfolio: d(allocated),
            quantity: d(quantity),
            allocated_notional_portfolio: d(allocated),
        }
    }

    fn split_plan() -> ExecutionPlan {
        let children = vec![
            child("broker-a", "account-a", "50", "50"),
            child("broker-b", "account-b", "50", "50"),
        ];
        ExecutionPlan::EqualSplit {
            allocation: EqualAllocation {
                parent_notional_portfolio: d("100"),
                target_notional_per_account: d("50"),
                children,
                allocated_notional_portfolio: d("100"),
                unallocated_remainder_portfolio: Decimal::ZERO,
            },
            costs: vec![
                BrokerCost { broker_id:"broker-a".into(), account_id:"account-a".into(), allocated_notional_portfolio:d("50"), estimated_fee_portfolio:Decimal::ZERO, estimated_slippage_portfolio:Decimal::ZERO, estimated_conversion_cost_portfolio:Decimal::ZERO, estimated_margin_required_portfolio:Decimal::ZERO, total_estimated_execution_cost_portfolio:Decimal::ZERO },
                BrokerCost { broker_id:"broker-b".into(), account_id:"account-b".into(), allocated_notional_portfolio:d("50"), estimated_fee_portfolio:Decimal::ZERO, estimated_slippage_portfolio:Decimal::ZERO, estimated_conversion_cost_portfolio:Decimal::ZERO, estimated_margin_required_portfolio:Decimal::ZERO, total_estimated_execution_cost_portfolio:Decimal::ZERO },
            ],
            estimated_total_cost_portfolio: Decimal::ZERO,
            single_broker_comparator_cost_portfolio: None,
            reason: "complete split".into(),
        }
    }

    fn consolidated_plan() -> ExecutionPlan {
        let child = child("broker-a", "account-a", "99", "99");
        ExecutionPlan::Consolidated {
            broker_id: "broker-a".into(),
            account_id: "account-a".into(),
            allocation: EqualAllocation {
                parent_notional_portfolio: d("100"),
                target_notional_per_account: d("100"),
                children: vec![child],
                allocated_notional_portfolio: d("99"),
                unallocated_remainder_portfolio: d("1"),
            },
            cost: BrokerCost { broker_id:"broker-a".into(), account_id:"account-a".into(), allocated_notional_portfolio:d("99"), estimated_fee_portfolio:Decimal::ZERO, estimated_slippage_portfolio:Decimal::ZERO, estimated_conversion_cost_portfolio:Decimal::ZERO, estimated_margin_required_portfolio:Decimal::ZERO, total_estimated_execution_cost_portfolio:Decimal::ZERO },
            reason: "consolidate".into(),
        }
    }

    fn build(plan: &ExecutionPlan) -> FanoutLifecycle {
        let (parent, children) = build_fanout_intents(request(), &selected(), plan, risk()).unwrap();
        FanoutLifecycle::new(parent, children).unwrap()
    }

    fn confirm_persisted(fanout: &mut FanoutLifecycle) {
        let receipt = NewlyPersistedFanout {
            parent_order_id: fanout.parent.parent_order_id.clone(),
            request_fingerprint: fanout.parent.request_fingerprint.clone(),
        };
        fanout.confirm_durable_persistence(receipt).unwrap();
    }

    #[test]
    fn equal_split_requires_every_selected_account_and_preserves_remainder() {
        let mut plan = split_plan();
        if let ExecutionPlan::EqualSplit { allocation, .. } = &mut plan {
            allocation.parent_notional_portfolio = d("101");
            allocation.allocated_notional_portfolio = d("100");
            allocation.unallocated_remainder_portfolio = d("1");
        }
        let mut req = request();
        req.parent_notional_portfolio = d("101");
        let (parent, children) = build_fanout_intents(req, &selected(), &plan, {
            let mut value = risk(); value.parent_notional_portfolio = d("101"); value
        }).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(parent.unallocated_remainder_portfolio, d("1"));
        assert_eq!(parent.allocated_notional_portfolio + parent.unallocated_remainder_portfolio, d("101"));
    }

    #[test]
    fn equal_split_rejects_a_silently_omitted_selected_account() {
        let mut plan = split_plan();
        if let ExecutionPlan::EqualSplit { allocation, costs, .. } = &mut plan {
            allocation.children.pop();
            costs.pop();
        }
        assert!(matches!(
            build_fanout_intents(request(), &selected(), &plan, risk()),
            Err(FanoutError::InvalidPlan(_))
        ));
    }

    #[test]
    fn equal_split_rejects_cost_estimate_for_the_wrong_account() {
        let mut plan = split_plan();
        if let ExecutionPlan::EqualSplit { costs, .. } = &mut plan {
            costs[0].broker_id = "unselected-broker".into();
        }
        assert!(matches!(
            build_fanout_intents(request(), &selected(), &plan, risk()),
            Err(FanoutError::InvalidPlan(_))
        ));
    }

    #[test]
    fn equal_split_rejects_underreported_cost_components() {
        let mut plan = split_plan();
        if let ExecutionPlan::EqualSplit { costs, .. } = &mut plan {
            costs[0].estimated_fee_portfolio = d("5");
            costs[0].total_estimated_execution_cost_portfolio = Decimal::ZERO;
        }
        assert!(matches!(
            build_fanout_intents(request(), &selected(), &plan, risk()),
            Err(FanoutError::InvalidPlan(_))
        ));
    }

    #[test]
    fn consolidated_route_records_selected_accounts_that_receive_no_child() {
        let (parent, children) = build_fanout_intents(request(), &selected(), &consolidated_plan(), risk()).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(parent.not_routed_accounts.len(), 1);
        assert!(parent.not_routed_accounts[0].reason.contains("receives no child order"));
    }

    #[test]
    fn child_client_order_ids_are_deterministic_and_scoped_to_account() {
        let (_, first) = build_fanout_intents(request(), &selected(), &split_plan(), risk()).unwrap();
        let (_, second) = build_fanout_intents(request(), &selected(), &split_plan(), risk()).unwrap();
        assert_eq!(first[0].client_order_id, second[0].client_order_id);
        assert_ne!(first[0].client_order_id, first[1].client_order_id);
        assert_eq!(first[0].client_order_id.len(), 36);
    }

    #[test]
    fn restored_lifecycle_preserves_dispatch_guard_and_unknown_child_state() {
        let mut fanout = build(&split_plan());
        confirm_persisted(&mut fanout);
        fanout.begin_dispatch(10_001).unwrap();
        let child_id = fanout.children[0].child_order_id.clone();
        fanout.begin_child_submission(&child_id, 10_002).unwrap();
        fanout.apply_child_event(&child_id, ChildEvent::SubmissionUnknown {
            reason: "network timeout after submit".into()
        }, 10_003).unwrap();

        let mut restored = FanoutLifecycle::restore(fanout.parent.clone(), fanout.children.clone()).unwrap();
        assert_eq!(restored.parent.status, ParentOrderStatus::ReconciliationRequired);
        assert_eq!(restored.begin_dispatch(10_004), Err(FanoutError::DispatchAlreadyStarted));
        assert_eq!(restored.begin_child_submission(&child_id, 10_004), Err(FanoutError::ChildAlreadySubmitted));
    }

    #[test]
    fn dispatch_is_blocked_until_all_intents_are_persisted() {
        let mut fanout = build(&split_plan());
        assert_eq!(fanout.begin_dispatch(10_001), Err(FanoutError::NotPersisted));
        confirm_persisted(&mut fanout);
        assert!(fanout.begin_dispatch(10_001).is_ok());
    }

    #[test]
    fn unknown_outcome_is_reconciled_and_never_blindly_resubmitted() {
        let mut fanout = build(&split_plan());
        confirm_persisted(&mut fanout);
        fanout.begin_dispatch(10_001).unwrap();
        let id = fanout.children[0].child_order_id.clone();
        fanout.begin_child_submission(&id, 10_002).unwrap();
        fanout.apply_child_event(&id, ChildEvent::SubmissionUnknown { reason: "timeout after request write".into() }, 10_003).unwrap();
        assert_eq!(fanout.parent.status, ParentOrderStatus::ReconciliationRequired);
        assert_eq!(fanout.begin_child_submission(&id, 10_004), Err(FanoutError::ChildAlreadySubmitted));
        fanout.apply_child_event(&id, ChildEvent::ReconciliationAccepted { broker_order_id: "exchange-1".into() }, 10_005).unwrap();
        assert_eq!(fanout.children[0].status, ChildOrderStatus::Accepted);
    }

    #[test]
    fn fills_are_idempotent_weighted_and_cannot_overfill() {
        let mut fanout = build(&split_plan());
        confirm_persisted(&mut fanout);
        fanout.begin_dispatch(10_001).unwrap();
        let id = fanout.children[0].child_order_id.clone();
        fanout.begin_child_submission(&id, 10_002).unwrap();
        fanout.apply_child_event(&id, ChildEvent::Acknowledged { broker_order_id: "exchange-1".into() }, 10_003).unwrap();
        let event = ChildEvent::PartialFill { execution_id: "fill-1".into(), quantity: d("20"), price: d("1.1"), fee: None };
        assert!(fanout.apply_child_event(&id, event.clone(), 10_004).unwrap());
        assert!(!fanout.apply_child_event(&id, event, 10_005).unwrap());
        assert_eq!(fanout.children[0].filled_quantity, d("20"));
        assert_eq!(fanout.children[0].average_fill_price, Some(d("1.1")));
        assert_eq!(
            fanout.apply_child_event(&id, ChildEvent::PartialFill { execution_id: "fill-2".into(), quantity: d("40"), price: d("1.2"), fee: None }, 10_006),
            Err(FanoutError::Overfill)
        );
    }

    #[test]
    fn partial_success_and_rejection_remain_visible_on_parent() {
        let mut fanout = build(&split_plan());
        confirm_persisted(&mut fanout);
        fanout.begin_dispatch(10_001).unwrap();
        let first = fanout.children[0].child_order_id.clone();
        let second = fanout.children[1].child_order_id.clone();
        fanout.begin_child_submission(&first, 10_002).unwrap();
        fanout.begin_child_submission(&second, 10_002).unwrap();
        fanout.apply_child_event(&first, ChildEvent::Acknowledged { broker_order_id: "exchange-1".into() }, 10_003).unwrap();
        fanout.apply_child_event(&second, ChildEvent::SubmissionRejected { reason: "minimum notional".into() }, 10_003).unwrap();
        assert_eq!(fanout.parent.status, ParentOrderStatus::PartiallySubmitted);
    }
}

