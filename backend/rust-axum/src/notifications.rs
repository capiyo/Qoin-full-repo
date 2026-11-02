use axum::{http::{HeaderMap,StatusCode},extract::State,Json};use mongodb::bson::{doc,DateTime};use serde::{Deserialize,Serialize};
use jsonwebtoken::{encode,Algorithm,EncodingKey,Header};
use chrono::Utc;
use std::env;use serde_json::{json,Value};use crate::{auth::authenticated_user,AppState};
#[derive(Deserialize)]pub struct RegisterToken{fcm_token:String,platform:String}
#[derive(Deserialize)]pub struct MarkRead{notification_ids:Option<Vec<String>>}
#[derive(Deserialize,serde::Serialize)]pub struct Preferences{market_alerts:bool,security_alerts:bool,system_alerts:bool}
pub async fn register_token(State(s):State<AppState>,h:HeaderMap,Json(p):Json<RegisterToken>)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;if p.fcm_token.trim().len()<40||!matches!(p.platform.as_str(),"web"|"android"|"ios"){return Err(StatusCode::BAD_REQUEST)}let c=s.db.collection::<mongodb::bson::Document>("fcm_tokens");c.update_one(doc!{"fcm_token":&p.fcm_token},doc!{"$set":{"user_id":&uid,"platform":&p.platform,"updated_at":DateTime::now()},"$setOnInsert":{"created_at":DateTime::now()}},mongodb::options::UpdateOptions::builder().upsert(true).build()).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;Ok(Json(json!({"success":true})))}
pub async fn list(State(s):State<AppState>,h:HeaderMap)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let c=s.db.collection::<mongodb::bson::Document>("notifications");let mut cur=c.find(doc!{"user_id":&uid},mongodb::options::FindOptions::builder().sort(doc!{"created_at":-1}).limit(50).build()).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;let mut items=Vec::new();use futures_util::TryStreamExt;while let Some(mut d)=cur.try_next().await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?{let id=d.get_object_id("_id").map(|v|v.to_hex()).unwrap_or_default();d.remove("_id");d.remove("user_id");if let Ok(created)=d.get_datetime("created_at"){d.insert("created_at",created.to_string());}let mut v=mongodb::bson::from_document::<Value>(d).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;if let Some(o)=v.as_object_mut(){o.insert("id".into(),json!(id));}items.push(v)}Ok(Json(json!({"notifications":items})))}
pub async fn unread_summary(State(s):State<AppState>,h:HeaderMap)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let n=s.db.collection::<mongodb::bson::Document>("notifications").count_documents(doc!{"user_id":&uid,"is_read":false},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;Ok(Json(json!({"notifications":n,"comments":0})))}
pub async fn mark_read(State(s):State<AppState>,h:HeaderMap,Json(p):Json<MarkRead>)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let mut f=doc!{"user_id":&uid,"is_read":false};if let Some(ids)=p.notification_ids{let parsed:Result<Vec<_>,_>=ids.iter().map(|x|mongodb::bson::oid::ObjectId::parse_str(x)).collect();f.insert("_id",doc!{"$in":parsed.map_err(|_|StatusCode::BAD_REQUEST)?});}let r=s.db.collection::<mongodb::bson::Document>("notifications").update_many(f,doc!{"$set":{"is_read":true,"read_at":DateTime::now()}},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;Ok(Json(json!({"success":true,"modified_count":r.modified_count})))}
pub async fn get_preferences(State(s):State<AppState>,h:HeaderMap)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let id=mongodb::bson::oid::ObjectId::parse_str(uid).map_err(|_|StatusCode::UNAUTHORIZED)?;let u=s.users.find_one(doc!{"_id":id},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::NOT_FOUND)?;Ok(Json(u.get_document("notification_preferences").map(|v|mongodb::bson::from_document(v.clone()).unwrap_or_else(|_|json!({}))).unwrap_or_else(|_|json!({"market_alerts":true,"security_alerts":true,"system_alerts":true}))))}
pub async fn update_preferences(State(s):State<AppState>,h:HeaderMap,Json(p):Json<Preferences>)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let id=mongodb::bson::oid::ObjectId::parse_str(uid).map_err(|_|StatusCode::UNAUTHORIZED)?;let d=mongodb::bson::to_document(&p).map_err(|_|StatusCode::BAD_REQUEST)?;s.users.update_one(doc!{"_id":id},doc!{"$set":{"notification_preferences":d,"updated_at":DateTime::now()}},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;Ok(Json(json!({"success":true})))}

#[derive(Deserialize)]
pub struct InternalNotification { user_id:String, notification_type:String, title:String, body:String, #[serde(default)] data:Value }
#[derive(Serialize)]
struct ServiceAccountClaims { iss:String, scope:String, aud:String, iat:usize, exp:usize }
#[derive(Deserialize)]
struct OAuthToken { access_token:String }
async fn send_fcm(project:&str,device_token:&str,title:&str,body:&str,kind:&str,data:&Value)->Result<(),()> {
 let email=env::var("FIREBASE_CLIENT_EMAIL").map_err(|_|())?;
 let private_key=env::var("FIREBASE_PRIVATE_KEY").map_err(|_|())?.replace("\\n","\n").trim_matches('"').to_owned();
 let key=EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|_|())?;
 let now=Utc::now().timestamp() as usize;
 let assertion=encode(&Header::new(Algorithm::RS256),&ServiceAccountClaims{iss:email,scope:"https://www.googleapis.com/auth/firebase.messaging".into(),aud:"https://oauth2.googleapis.com/token".into(),iat:now,exp:now+3600},&key).map_err(|_|())?;
 let client=reqwest::Client::new();
 let oauth=client.post("https://oauth2.googleapis.com/token").form(&[("grant_type","urn:ietf:params:oauth:grant-type:jwt-bearer"),("assertion",assertion.as_str())]).send().await.map_err(|_|())?;
 if !oauth.status().is_success(){return Err(())}
 let oauth_token:OAuthToken=oauth.json().await.map_err(|_|())?;
 let endpoint=format!("https://fcm.googleapis.com/v1/projects/{project}/messages:send");
 let payload=json!({"message":{"token":device_token,"notification":{"title":title,"body":body},"data":{"type":kind,"payload":serde_json::to_string(data).unwrap_or_default()},"android":{"priority":"HIGH"},"apns":{"headers":{"apns-priority":"10"}}}});
 let response=client.post(endpoint).bearer_auth(oauth_token.access_token).json(&payload).send().await.map_err(|_|())?;
 if response.status().is_success(){Ok(())}else{Err(())}
}
pub async fn send_internal(State(s):State<AppState>,h:HeaderMap,Json(p):Json<InternalNotification>)->Result<Json<Value>,StatusCode>{
 let expected=env::var("QOIN_INTERNAL_API_KEY").map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
 let supplied=h.get("x-qoin-internal-key").and_then(|v|v.to_str().ok()).ok_or(StatusCode::UNAUTHORIZED)?;
 if expected.len()<32||supplied!=expected{return Err(StatusCode::UNAUTHORIZED)}
 if p.user_id.is_empty()||p.title.trim().is_empty()||p.body.trim().is_empty(){return Err(StatusCode::BAD_REQUEST)}
 let now=DateTime::now();
 let record=doc!{"user_id":&p.user_id,"notification_type":&p.notification_type,"title":&p.title,"body":&p.body,"data":mongodb::bson::to_bson(&p.data).map_err(|_|StatusCode::BAD_REQUEST)?,"is_read":false,"created_at":now};
 s.db.collection::<mongodb::bson::Document>("notifications").insert_one(record,None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
 let user_oid=mongodb::bson::oid::ObjectId::parse_str(&p.user_id).map_err(|_|StatusCode::BAD_REQUEST)?;
 let user=s.users.find_one(doc!{"_id":user_oid},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::NOT_FOUND)?;
 let preferences=user.get_document("notification_preferences").ok();
 let preference_key=match p.notification_type.as_str(){"market"|"market_alert"=>"market_alerts","security"|"security_alert"=>"security_alerts",_=>"system_alerts"};
 if preferences.and_then(|v|v.get_bool(preference_key).ok()).is_some_and(|enabled|!enabled){return Ok(Json(json!({"success":true,"stored":true,"attempted":0,"delivered":0,"push_suppressed":true})))}
 let tokens=s.db.collection::<mongodb::bson::Document>("fcm_tokens");
 let mut cursor=tokens.find(doc!{"user_id":&p.user_id},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
 use futures_util::TryStreamExt;
 let mut attempted=0usize;let mut delivered=0usize;
 let project=env::var("QOIN_FIREBASE_PROJECT_ID").unwrap_or_else(|_|"funzy-d56d7".to_owned());
 while let Some(device)=cursor.try_next().await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?{
  let device_token=device.get_str("fcm_token").unwrap_or("");
  if device_token.is_empty(){continue}attempted+=1;
  if send_fcm(&project,device_token,&p.title,&p.body,&p.notification_type,&p.data).await.is_ok(){delivered+=1}
 }
 Ok(Json(json!({"success":true,"stored":true,"attempted":attempted,"delivered":delivered})))
}
