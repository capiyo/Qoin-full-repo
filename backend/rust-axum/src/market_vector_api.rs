use axum::{extract::{Query,State},http::{HeaderMap,StatusCode},Json};
use futures_util::TryStreamExt;
use mongodb::{bson::doc,options::{FindOneOptions,FindOptions}};
use qoin_market_space::MarketVector;
use serde::Deserialize;
use serde_json::{json,Value};
use super::{firebase_uid_from_headers,ensure_user_record,AppState};

#[derive(Debug,Deserialize)]
pub(super) struct MarketVectorQuery {
    pub symbol: Option<String>,
    pub venue: Option<String>,
    pub limit: Option<i64>,
}

pub(super) async fn get_latest(
    State(s):State<AppState>, headers:HeaderMap, Query(q):Query<MarketVectorQuery>
)->Result<Json<Value>,StatusCode>{
    let uid=firebase_uid_from_headers(s.firebase_verifier.as_ref(),&headers).await?;
    ensure_user_record(&s.users,&uid).await?;
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into()).trim().to_ascii_uppercase();
    if symbol.is_empty()||symbol.len()>96{return Err(StatusCode::BAD_REQUEST);}
    let mut filter=doc!{"symbol":&symbol};
    if let Some(venue)=q.venue.as_deref(){if venue.trim().is_empty()||venue.len()>64{return Err(StatusCode::BAD_REQUEST);}filter.insert("venue",venue);}
    match s.market_vectors.find_one(filter,FindOneOptions::builder().sort(doc!{"event_time_cutoff_ms":-1}).build()).await{
        Ok(Some(mut document))=>{
            document.remove("_id");document.remove("stored_at");
            let vector=mongodb::bson::from_document::<MarketVector>(document).map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
            vector.validate().map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
            Ok(Json(serde_json::to_value(vector).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?))
        }
        Ok(None)=>Ok(Json(json!({"status":"warming_up","symbol":symbol,"venue":q.venue,"schema_version":1,"observed_dimensions":0}))),
        Err(_)=>Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub(super) async fn get_history(
    State(s):State<AppState>, headers:HeaderMap, Query(q):Query<MarketVectorQuery>
)->Result<Json<Value>,StatusCode>{
    let uid=firebase_uid_from_headers(s.firebase_verifier.as_ref(),&headers).await?;
    ensure_user_record(&s.users,&uid).await?;
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into()).trim().to_ascii_uppercase();
    if symbol.is_empty()||symbol.len()>96{return Err(StatusCode::BAD_REQUEST);}
    let limit=q.limit.unwrap_or(120).clamp(1,500);
    let mut filter=doc!{"symbol":&symbol};
    if let Some(venue)=q.venue.as_deref(){if venue.trim().is_empty()||venue.len()>64{return Err(StatusCode::BAD_REQUEST);}filter.insert("venue",venue);}
    let options=FindOptions::builder().sort(doc!{"event_time_cutoff_ms":-1}).limit(limit).build();
    let mut cursor=s.market_vectors.find(filter,options).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
    let mut items=Vec::new();
    while let Some(mut document)=cursor.try_next().await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?{
        document.remove("_id");document.remove("stored_at");
        let vector=mongodb::bson::from_document::<MarketVector>(document).map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
        vector.validate().map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
        items.push(serde_json::to_value(vector).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?);
    }
    items.reverse();
    let count=items.len();
    Ok(Json(json!({"symbol":symbol,"venue":q.venue,"items":items,"count":count,"limit":limit})))
}
