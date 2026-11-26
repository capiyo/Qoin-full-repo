use super::*;
use qoin_set_theoretic_attention::{
    set_attention_weights, EvidenceSet, RelationshipEventType, RelationshipLedgerEvent,
    RelationshipRecord, Vocabulary,
};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct RelationshipLedgerQuery { limit: Option<i64> }

#[derive(Deserialize)]
struct DiscoverRelationshipsRequest {
    query: EvidenceSet,
    candidates: Vec<EvidenceSet>,
    model_version: String,
    temperature: f64,
    recorded_at_ms: i64,
}

async fn discover_relationships(
    State(state): State<AppState>,
    Json(request): Json<DiscoverRelationshipsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if request.candidates.is_empty() || request.candidates.len() > 500
        || request.recorded_at_ms <= 0 || request.model_version.trim().is_empty()
        || request.model_version.len() > 128
    {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"provide 1-500 candidate sets, a model_version, and a positive recorded_at_ms"
        }))));
    }
    request.query.validate().map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    for candidate in &request.candidates {
        candidate.validate().map_err(|error| (
            StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
        ))?;
    }
    let total_atoms = request.query.atoms.len()
        + request.candidates.iter().map(|candidate| candidate.atoms.len()).sum::<usize>();
    if total_atoms > 10_000 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({
            "error":"total evidence atoms per discovery request cannot exceed 10000"
        }))));
    }
    let latest_observation = request.candidates.iter()
        .map(|candidate| candidate.observed_at_ms)
        .chain(std::iter::once(request.query.observed_at_ms))
        .max().unwrap_or(0);
    if request.recorded_at_ms < latest_observation {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"recorded_at_ms cannot precede the evidence observation timestamps"
        }))));
    }

    let mut evidence_sets = Vec::with_capacity(request.candidates.len() + 1);
    evidence_sets.push(request.query.clone());
    evidence_sets.extend(request.candidates.iter().cloned());
    let vocabulary = Vocabulary::from_sets(&evidence_sets).map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    let query_embedding = vocabulary.embed(&request.query).map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    let key_embeddings = request.candidates.iter()
        .map(|candidate| vocabulary.embed(candidate))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
    let weights = set_attention_weights(&query_embedding, &key_embeddings, request.temperature)
        .map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;

    let mut persisted = 0usize;
    let mut already_present = 0usize;
    for (candidate, attention_weight) in request.candidates.iter().zip(weights) {
        let relationship = RelationshipRecord::discover(
            &request.query, candidate, request.model_version.clone(), attention_weight,
            request.temperature,
        ).map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
        let identity = serde_json::to_vec(&(
            &relationship,
            request.recorded_at_ms,
        )).map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":format!("could not encode relationship identity: {error}")}))))?;
        let event_id = format!("v2rel:{}", hex::encode(Sha256::digest(identity)));
        let event = RelationshipLedgerEvent {
            event_id,
            event_type: RelationshipEventType::Discovered,
            recorded_at_ms: request.recorded_at_ms,
            relationship,
            outcome: None,
        };
        event.validate().map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
        let document = mongodb::bson::to_document(&event).map_err(|error| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":format!("could not serialize ledger event: {error}")})),
        ))?;
        match state.relationship_ledger.insert_one(document, None).await {
            Ok(_) => persisted += 1,
            Err(error) if error.to_string().contains("E11000") => already_present += 1,
            Err(error) => {
                tracing::error!(error=%error, persisted, already_present,
                    "V2 relationship discovery ledger append failed");
                return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"relationship ledger append failed",
                    "persisted_before_failure":persisted,
                    "already_present":already_present
                }))));
            }
        }
    }
    let status = if persisted > 0 { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(serde_json::json!({
        "model_version":request.model_version,
        "candidate_count":request.candidates.len(),
        "persisted":persisted,
        "already_present":already_present,
        "ledger":"append-only",
        "decision_authority":"version-1-vector-engine"
    }))))
}

async fn append_relationship_event(
    State(state): State<AppState>,
    Json(event): Json<RelationshipLedgerEvent>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    event.validate().map_err(|error| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": error.to_string()})),
    ))?;
    let event_id = event.event_id.clone();
    let document = mongodb::bson::to_document(&event).map_err(|error| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": format!("could not serialize ledger event: {error}")})),
    ))?;
    match state.relationship_ledger.insert_one(document, None).await {
        Ok(_) => Ok((StatusCode::CREATED, Json(serde_json::json!({
            "event_id": event_id,
            "persisted": true,
            "append_only": true
        })))),
        Err(error) if error.to_string().contains("E11000") => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"relationship ledger event_id already exists"})),
        )),
        Err(error) => {
            tracing::error!(error=%error, event_id=%event_id, "relationship ledger append failed");
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"relationship ledger unavailable"})),
            ))
        }
    }
}

async fn list_relationship_events(
    State(state): State<AppState>,
    Query(query): Query<RelationshipLedgerQuery>,
) -> Result<Json<Vec<RelationshipLedgerEvent>>, (StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let options = FindOptions::builder()
        .sort(doc!{"recorded_at_ms":-1,"event_id":1})
        .limit(limit)
        .build();
    let mut cursor = state.relationship_ledger.find(None, Some(options)).await.map_err(|error| {
        tracing::error!(error=%error, "relationship ledger query failed");
        (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"relationship ledger unavailable"})))
    })?;
    let documents = cursor.try_collect::<Vec<Document>>().await.map_err(|error| {
        tracing::error!(error=%error, "relationship ledger read failed");
        (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"relationship ledger read failed"})))
    })?;
    let events = documents.into_iter()
        .map(mongodb::bson::from_document::<RelationshipLedgerEvent>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            tracing::error!(error=%error, "relationship ledger contains an invalid persisted event");
            (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"relationship ledger contains invalid data"})))
        })?;
    Ok(Json(events))
}

