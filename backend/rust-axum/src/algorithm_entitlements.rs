//! Fail-closed entitlement domain rules for Qoin algorithm access.
//!
//! This module intentionally does not parse client identity, payment webhooks,
//! or bearer tokens. The caller must obtain `authenticated_user_id` from a
//! verified authentication context. A payment only grants access after a
//! trusted provider reconciliation process creates a verified grant.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrantStatus {
    Pending,
    Active,
    Expired,
    Refunded,
    Revoked,
    Chargeback,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EntitlementGrant {
    pub grant_id: String,
    pub user_id: String,
    pub algorithm_ids: Vec<String>,
    pub capabilities: Vec<String>,
    pub status: GrantStatus,
    /// Set only by trusted payment reconciliation or an authorized grant issuer.
    pub verified_at_ms: Option<i64>,
    pub effective_at_ms: i64,
    pub expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EntitlementDecision {
    pub allowed: bool,
    pub matched_grant_ids: Vec<String>,
    pub missing_capabilities: Vec<String>,
    pub reason: &'static str,
}

/// Resolve access using only the verified user identity supplied by the
/// authentication layer. Empty identity, missing verification, non-active
/// grants, future grants, expiry and mismatched users all deny access.
pub fn authorize_algorithm(
    authenticated_user_id: &str,
    algorithm_id: &str,
    required_capabilities: &[String],
    grants: &[EntitlementGrant],
    now_ms: i64,
) -> EntitlementDecision {
    let denied = |reason, missing_capabilities| EntitlementDecision {
        allowed: false,
        matched_grant_ids: Vec::new(),
        missing_capabilities,
        reason,
    };

    if authenticated_user_id.trim().is_empty() || algorithm_id.trim().is_empty() || now_ms <= 0 {
        return denied("authenticated identity, algorithm ID, and valid clock are required",
            required_capabilities.to_vec());
    }

    let mut matched_grant_ids = Vec::new();
    let mut allowed_capabilities = Vec::new();

    for grant in grants {
        if grant.user_id != authenticated_user_id
            || grant.status != GrantStatus::Active
            || grant.verified_at_ms.map(|v| v <= 0 || v > now_ms).unwrap_or(true)
            || grant.effective_at_ms > now_ms
            || grant.expires_at_ms.map(|expiry| expiry <= now_ms).unwrap_or(false)
            || grant.grant_id.trim().is_empty()
        {
            continue;
        }

        if grant.algorithm_ids.iter().any(|id| id == algorithm_id) {
            matched_grant_ids.push(grant.grant_id.clone());
            for capability in &grant.capabilities {
                if !allowed_capabilities.contains(capability) {
                    allowed_capabilities.push(capability.clone());
                }
            }
        }
    }

    if matched_grant_ids.is_empty() {
        return denied("no verified active grant covers this algorithm", required_capabilities.to_vec());
    }

    let missing_capabilities: Vec<String> = required_capabilities.iter()
        .filter(|required| !allowed_capabilities.contains(required))
        .cloned()
        .collect();

    if !missing_capabilities.is_empty() {
        return EntitlementDecision {
            allowed: false,
            matched_grant_ids,
            missing_capabilities,
            reason: "active grants do not include every required capability",
        };
    }

    EntitlementDecision {
        allowed: true,
        matched_grant_ids,
        missing_capabilities: Vec::new(),
        reason: "verified active grant covers algorithm and required capabilities",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_grant() -> EntitlementGrant {
        EntitlementGrant {
            grant_id: "grant-001".into(),
            user_id: "user-001".into(),
            algorithm_ids: vec!["qoin-vector-baseline".into()],
            capabilities: vec!["crypto".into(), "5m".into()],
            status: GrantStatus::Active,
            verified_at_ms: Some(1_000),
            effective_at_ms: 1_000,
            expires_at_ms: Some(10_000),
        }
    }

    #[test]
    fn verified_active_grant_authorizes_requested_capabilities() {
        let decision = authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into(), "5m".into()], &[active_grant()], 5_000);
        assert!(decision.allowed);
        assert_eq!(decision.matched_grant_ids, vec!["grant-001"]);
        assert!(decision.missing_capabilities.is_empty());
    }

    #[test]
    fn another_users_grant_never_authorizes() {
        let decision = authorize_algorithm("user-002", "qoin-vector-baseline",
            &["crypto".into()], &[active_grant()], 5_000);
        assert!(!decision.allowed);
        assert!(decision.matched_grant_ids.is_empty());
    }

    #[test]
    fn pending_or_unverified_payment_never_authorizes() {
        let mut grant = active_grant();
        grant.status = GrantStatus::Pending;
        assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into()], &[grant.clone()], 5_000).allowed);
        grant.status = GrantStatus::Active;
        grant.verified_at_ms = None;
        assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into()], &[grant], 5_000).allowed);
    }

    #[test]
    fn expired_and_not_yet_effective_grants_are_denied() {
        let mut grant = active_grant();
        grant.expires_at_ms = Some(5_000);
        assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into()], &[grant.clone()], 5_000).allowed);
        grant.expires_at_ms = None;
        grant.effective_at_ms = 6_000;
        assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into()], &[grant], 5_000).allowed);
    }

    #[test]
    fn missing_capability_fails_closed() {
        let decision = authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into(), "1h".into()], &[active_grant()], 5_000);
        assert!(!decision.allowed);
        assert_eq!(decision.missing_capabilities, vec!["1h".to_owned()]);
    }

    #[test]
    fn revoked_refunded_and_chargeback_grants_are_denied() {
        for status in [GrantStatus::Revoked, GrantStatus::Refunded, GrantStatus::Chargeback, GrantStatus::Expired] {
            let mut grant = active_grant();
            grant.status = status;
            assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
                &["crypto".into()], &[grant], 5_000).allowed);
        }
    }

    #[test]
    fn empty_identity_or_invalid_clock_is_denied() {
        let grant = active_grant();
        assert!(!authorize_algorithm("", "qoin-vector-baseline",
            &["crypto".into()], &[grant.clone()], 5_000).allowed);
        assert!(!authorize_algorithm("user-001", "qoin-vector-baseline",
            &["crypto".into()], &[grant], 0).allowed);
    }
}
