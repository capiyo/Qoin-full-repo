use super::*;

#[derive(Deserialize)]
struct BrokerReadinessQuery { brokers: Option<String> }

async fn get_broker_readiness(
    State(s): State<AppState>,
    Query(query): Query<BrokerReadinessQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    let catalogue = broker_catalogue::catalogue();
    let items = catalogue["items"].as_array().cloned().unwrap_or_default();
    let requested: Vec<String> = query.brokers.unwrap_or_default().split(',')
        .map(|id| id.trim().to_ascii_lowercase())
        .filter(|id| !id.is_empty()).collect();
    let mut seen = HashSet::new();
    let selected: Vec<serde_json::Value> = requested.iter().filter(|id| seen.insert((*id).clone()))
        .map(|id| {
            items.iter().find(|item| item["id"].as_str() == Some(id.as_str())).cloned()
                .unwrap_or_else(|| serde_json::json!({"id":id,"known":false,"execution_ready":false,
                    "reason":"provider is not in the supported catalogue"}))
        }).collect();
    let ready_count = selected.iter().filter(|item| item["execution_ready"] == true).count();
    let requested_set: HashSet<String> = requested.iter().cloned().collect();
    let mut account_cursor = match s.broker_accounts.find(
        doc!{"owner_uid":&owner_uid,"status":"active","trading_enabled":true}, None
    ).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, "user broker account readiness lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account readiness unavailable"})));
        }
    };
    let mut connected_account_count = 0usize;
    loop {
        match account_cursor.try_next().await {
            Ok(Some(account)) => {
                let broker_id = account.get_str("broker_id").unwrap_or_default();
                if requested_set.is_empty() || requested_set.contains(broker_id) {
                    connected_account_count += 1;
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, "user broker account readiness cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account readiness unavailable"})));
            }
        }
    }
    let configured = s.broker_registry.configured_brokers().unwrap_or_default();
    let configured_set: HashSet<String> = configured.iter().cloned().collect();
    let configured_count = selected.iter().filter(|item| item["id"].as_str().is_some_and(|id| configured_set.contains(id))).count();
    let mut blockers = vec![
        "No provider-specific execution adapter is registered in this runtime.".to_string(),
        "Environment credentials do not prove broker authentication, account permissions, or subaccount ownership.".to_string(),
        "Live dispatch remains disabled until provider conformance, account mapping, and per-user risk controls pass.".to_string(),
    ];
    if s.firebase_verifier.is_none() {
        blockers.push("QOIN_FIREBASE_PROJECT_ID is not configured; server-side Firebase identity verification is unavailable.".into());
    }
    if configured.is_empty() {
        blockers.push("No provider adapters are registered in this runtime.".into());
    }
    (StatusCode::OK, Json(serde_json::json!({
        "selected_count": selected.len(),
        "execution_ready_count": ready_count,
        "connected_account_count": connected_account_count,
        "configured_adapter_count": configured_count,
        "configured_environment_provider_count": s.broker_environment.configured_provider_ids().len(),
        "configured_adapters": configured,
        "can_submit": false,
        "dispatch_enabled": false,
        "items": selected,
        "blockers": blockers,
        "message": "Selection preferences are passed to the execution-readiness API, but no selected provider is currently eligible for order submission."
    })))
}


async fn get_broker_configuration(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let owner_uid = match firebase_uid_from_headers(s.firebase_verifier.as_ref(), &headers).await {
        Ok(uid) => uid,
        Err(status) => return (status, Json(serde_json::json!({"error":"valid Firebase sign-in is required"}))),
    };
    if let Err(status) = ensure_user_record(&s.users, &owner_uid).await {
        return (status, Json(serde_json::json!({"error":"user account is unavailable"})));
    }
    (StatusCode::OK, Json(s.broker_environment.public_status()))
}


#[derive(Debug, Deserialize)]
struct ProvisionBrokerAccountRequest {
    owner_uid: String,
    broker_id: String,
    account_id: String,
    account_name: String,
    account_type: String,
    base_currency: String,
    #[serde(default)]
    trading_enabled: bool,
}

async fn provision_broker_account(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProvisionBrokerAccountRequest>,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"operator authorization required"})));
    }
    let owner_uid = request.owner_uid.trim();
    let broker_id = request.broker_id.trim().to_ascii_lowercase();
    let account_id = request.account_id.trim();
    let account_name = request.account_name.trim();
    let account_type = request.account_type.trim().to_ascii_lowercase();
    let base_currency = request.base_currency.trim().to_ascii_uppercase();
    if owner_uid.is_empty() || owner_uid.len() > 128
        || broker_id.is_empty() || broker_id.len() > 64
        || !broker_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        || account_id.is_empty() || account_id.len() > 256
        || account_name.is_empty() || account_name.len() > 80
        || !matches!(account_type.as_str(), "live" | "demo")
        || base_currency.len() < 3 || base_currency.len() > 12
        || !base_currency.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid broker account provisioning fields"})));
    }
    if !s.broker_environment.account_authorized(&broker_id, account_id) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error":"broker subaccount is not present in backend environment allowlist"})));
    }
    match s.users.find_one(doc!{"_id":owner_uid,"status":"active"}, None).await {
        Ok(Some(_)) => {},
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"target user must sign in to Qoin before account provisioning"}))),
        Err(error) => {
            tracing::error!(error=%error, "target user lookup failed during broker account provisioning");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"target user lookup unavailable"})));
        }
    }
    let now = mongodb::bson::DateTime::now();
    let account = doc!{
        "owner_uid":owner_uid,"broker_id":&broker_id,"account_id":account_id,
        "account_name":account_name,"account_type":&account_type,"base_currency":&base_currency,
        "status":"active","trading_enabled":request.trading_enabled,
        "created_at":now,"updated_at":now
    };
    match s.broker_accounts.insert_one(account, None).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({
            "owner_uid":owner_uid,"broker_id":broker_id,"account_id":account_id,
            "account_name":account_name,"account_type":account_type,"base_currency":base_currency,
            "status":"active","trading_enabled":request.trading_enabled
        }))),
        Err(error) if error.to_string().contains("E11000") => {
            (StatusCode::CONFLICT, Json(serde_json::json!({"error":"broker subaccount is already assigned to a Firebase user"})))
        }
        Err(error) => {
            tracing::error!(error=%error, "broker account provisioning failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"broker account provisioning unavailable"})))
        }
    }
}

