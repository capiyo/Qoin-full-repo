use axum::{extract::{Path,State},http::{header::AUTHORIZATION,HeaderMap,StatusCode},Json};use bcrypt::{hash,verify,DEFAULT_COST};use chrono::Utc;use jsonwebtoken::{decode,decode_header,encode,Algorithm,DecodingKey,EncodingKey,Header,Validation};use mongodb::bson::{doc,DateTime};use serde::{Deserialize,Serialize};use serde_json::{json,Value};use std::env;use crate::AppState;
#[derive(Debug,Deserialize)]struct FirebaseClaims{sub:String,phone_number:Option<String>}
#[derive(Debug,Serialize,Deserialize)]struct SessionClaims{sub:String,username:String,exp:usize,iss:String}
#[derive(Debug,Deserialize)]pub struct FirebaseTokenRequest{id_token:String}
#[derive(Debug,Deserialize)]pub struct RegisterRequest{id_token:String,username:String}
#[derive(Debug,Deserialize)]pub struct PinLoginRequest{phone:String,pin:String}
#[derive(Debug,Deserialize)]pub struct PinRequest{pin:String}
fn valid_username(name:&str)->bool{
 let lower=name.to_ascii_lowercase();
 if !(3..=20).contains(&name.len())||!name.chars().all(|c|c.is_ascii_alphabetic()){return false}
 let blocked=["admin","administrator","superadmin","root","sysadmin","moderator","mod","staff","support","helpdesk","webmaster","master","system","test","user","guest","anonymous","default","example","null","undefined","service","api","bot","cron","daemon","clash","fanclash","clashfan"];
 if blocked.iter().any(|v|lower==*v)||["admin","root","super","mod"].iter().any(|v|lower.contains(v)){return false}
 if lower.bytes().windows(3).any(|w|w[1]==w[0]+1&&w[2]==w[1]+1)||["qwerty","asdfgh","zxcvbn","qwert","asdf","zxcv"].iter().any(|v|lower.contains(v)){return false}
 if lower.bytes().all(|b|b==lower.as_bytes()[0]){return false}
 true
}
fn normalize_phone(p:&str)->String{let d:String=p.chars().filter(char::is_ascii_digit).collect();if p.trim().starts_with('+'){format!("+{d}")}else if d.starts_with("00"){format!("+{}",&d[2..])}else{d}}
fn session_token(id:&str,name:&str)->Result<String,StatusCode>{let s=env::var("QOIN_JWT_SECRET").map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;if s.len()<32{return Err(StatusCode::SERVICE_UNAVAILABLE)}let now=Utc::now().timestamp() as usize;encode(&Header::default(),&SessionClaims{sub:id.into(),username:name.into(),exp:now+43200,iss:"qoin-api".into()},&EncodingKey::from_secret(s.as_bytes())).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)}
pub fn authenticated_user(h:&HeaderMap)->Result<String,StatusCode>{let v=h.get(AUTHORIZATION).and_then(|v|v.to_str().ok()).ok_or(StatusCode::UNAUTHORIZED)?;let t=v.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;let s=env::var("QOIN_JWT_SECRET").map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;if s.len()<32{return Err(StatusCode::SERVICE_UNAVAILABLE)}let mut validation=Validation::new(Algorithm::HS256);validation.set_issuer(&["qoin-api"]);decode::<SessionClaims>(t,&DecodingKey::from_secret(s.as_bytes()),&validation).map(|d|d.claims.sub).map_err(|_|StatusCode::UNAUTHORIZED)}
async fn verify_firebase(t:&str)->Result<FirebaseClaims,StatusCode>{let project=env::var("QOIN_FIREBASE_PROJECT_ID").map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;let h=decode_header(t).map_err(|_|StatusCode::UNAUTHORIZED)?;if h.alg!=Algorithm::RS256{return Err(StatusCode::UNAUTHORIZED)}let kid=h.kid.ok_or(StatusCode::UNAUTHORIZED)?;let response=reqwest::Client::new().get("https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com").send().await.map_err(|_|StatusCode::BAD_GATEWAY)?;if !response.status().is_success(){return Err(StatusCode::BAD_GATEWAY)}let keys:jsonwebtoken::jwk::JwkSet=response.json().await.map_err(|_|StatusCode::BAD_GATEWAY)?;let jwk=keys.find(&kid).ok_or(StatusCode::UNAUTHORIZED)?;let key=DecodingKey::from_jwk(jwk).map_err(|_|StatusCode::UNAUTHORIZED)?;let mut v=Validation::new(Algorithm::RS256);v.set_audience(&[project.as_str()]);let issuer=format!("https://securetoken.google.com/{project}");v.set_issuer(&[issuer.as_str()]);let c=decode::<FirebaseClaims>(t,&key,&v).map_err(|_|StatusCode::UNAUTHORIZED)?.claims;if c.sub.is_empty()||c.phone_number.as_deref().unwrap_or("").is_empty(){return Err(StatusCode::UNAUTHORIZED)}Ok(c)}
fn public_user(u:&mongodb::bson::Document)->Value{json!({"id":u.get_object_id("_id").map(|v|v.to_hex()).unwrap_or_default(),"username":u.get_str("username").unwrap_or(""),"phone":u.get_str("phone").unwrap_or(""),"entitlements":u.get_document("entitlements").ok().cloned().unwrap_or_default()})}
async fn issue(s:&AppState,u:mongodb::bson::Document)->Result<Json<Value>,StatusCode>{let id=u.get_object_id("_id").map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.to_hex();let name=u.get_str("username").unwrap_or("Trader").to_owned();let token=session_token(&id,&name)?;let _=s.users.update_one(doc!{"_id":u.get_object_id("_id").map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?},doc!{"$set":{"last_login":DateTime::now()}},None).await;Ok(Json(json!({"success":true,"token":token,"user":public_user(&u)})))}
pub async fn firebase_login(State(s):State<AppState>,Json(p):Json<FirebaseTokenRequest>)->Result<Json<Value>,StatusCode>{let c=verify_firebase(&p.id_token).await?;let phone=normalize_phone(c.phone_number.as_deref().unwrap_or(""));let u=s.users.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;match u{Some(u)=>issue(&s,u).await,None=>Err(StatusCode::NOT_FOUND)}}
pub async fn register(State(s):State<AppState>,Json(p):Json<RegisterRequest>)->Result<(StatusCode,Json<Value>),StatusCode>{let c=verify_firebase(&p.id_token).await?;let phone=normalize_phone(c.phone_number.as_deref().unwrap_or(""));let name=p.username.trim();if !valid_username(name){return Err(StatusCode::BAD_REQUEST)}if s.users.find_one(doc!{"$or":[{"phone":&phone},{"username":name}]},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.is_some(){return Err(StatusCode::CONFLICT)}let now=DateTime::now();s.users.insert_one(doc!{"username":name,"phone":&phone,"firebase_uid":c.sub,"auth_methods":["firebase_phone"],"created_at":now,"updated_at":now,"last_login":now,"entitlements":{"forex":false,"crypto":false,"stocks":false,"strategies":[]},"notification_preferences":{"market_alerts":true,"security_alerts":true,"system_alerts":true}},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;let u=s.users.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;Ok((StatusCode::CREATED,issue(&s,u).await?))}
pub async fn pin_login(State(s):State<AppState>,Json(p):Json<PinLoginRequest>)->Result<Json<Value>,StatusCode>{
 if p.pin.len()!=4||!p.pin.bytes().all(|b|b.is_ascii_digit()){return Err(StatusCode::BAD_REQUEST)}
 let phone=normalize_phone(&p.phone);
 let attempts=s.db.collection::<mongodb::bson::Document>("auth_pin_attempts");
 let now=DateTime::now();
 if let Some(record)=attempts.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?{
  if let Ok(locked)=record.get_datetime("locked_until"){if locked.timestamp_millis()>now.timestamp_millis(){return Err(StatusCode::TOO_MANY_REQUESTS)}else{attempts.update_one(doc!{"phone":&phone},doc!{"$set":{"failed_count":0},"$unset":{"locked_until":""}},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;}}
 }
 let u=s.users.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::UNAUTHORIZED)?;
 let valid=u.get_str("pin_hash").ok().and_then(|stored|verify(&p.pin,stored).ok()).unwrap_or(false);
 if !valid{
  let prior=attempts.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
  let count=prior.and_then(|d|d.get_i32("failed_count").ok()).unwrap_or(0)+1;
  let mut update=doc!{"$set":{"failed_count":count,"updated_at":now}};
  if count>=5{update.insert("$set",doc!{"failed_count":count,"updated_at":now,"locked_until":DateTime::from_millis(now.timestamp_millis()+15*60*1000)});}
  attempts.update_one(doc!{"phone":&phone},update,mongodb::options::UpdateOptions::builder().upsert(true).build()).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
  return Err(StatusCode::UNAUTHORIZED)
 }
 attempts.delete_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
 issue(&s,u).await
}
pub async fn set_pin(State(s):State<AppState>,h:HeaderMap,Json(p):Json<PinRequest>)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;if p.pin.len()!=4||!p.pin.bytes().all(|b|b.is_ascii_digit()){return Err(StatusCode::BAD_REQUEST)}let id=mongodb::bson::oid::ObjectId::parse_str(uid).map_err(|_|StatusCode::UNAUTHORIZED)?;let pin=hash(&p.pin,DEFAULT_COST).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;let r=s.users.update_one(doc!{"_id":id},doc!{"$set":{"pin_hash":pin,"updated_at":DateTime::now()}},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;if r.matched_count==0{return Err(StatusCode::NOT_FOUND)}let u=s.users.find_one(doc!{"_id":id},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::NOT_FOUND)?;issue(&s,u).await}
pub async fn me(State(s):State<AppState>,h:HeaderMap)->Result<Json<Value>,StatusCode>{let uid=authenticated_user(&h)?;let id=mongodb::bson::oid::ObjectId::parse_str(uid).map_err(|_|StatusCode::UNAUTHORIZED)?;let u=s.users.find_one(doc!{"_id":id},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.ok_or(StatusCode::NOT_FOUND)?;Ok(Json(json!({"user":public_user(&u)})))}
pub async fn check_user(State(s):State<AppState>,Path(phone):Path<String>)->Result<Json<Value>,StatusCode>{let phone=normalize_phone(&phone);let u=s.users.find_one(doc!{"phone":&phone},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;Ok(Json(match u{Some(u)=>json!({"success":true,"exists":true,"has_pin":u.get_str("pin_hash").is_ok(),"user":{"id":u.get_object_id("_id").map(|v|v.to_hex()).unwrap_or_default(),"username":u.get_str("username").unwrap_or(""),"phone":phone}}),None=>json!({"success":true,"exists":false,"has_pin":false})}))}

pub async fn username_taken(State(s):State<AppState>,Path(username):Path<String>)->Result<Json<Value>,StatusCode>{
 let user=s.users.find_one(doc!{"username":&username},None).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
 Ok(Json(json!({"success":true,"user":user.map(|u|json!({"id":u.get_object_id("_id").map(|v|v.to_hex()).unwrap_or_default(),"username":u.get_str("username").unwrap_or("")}))})))
}
pub async fn set_pin_for_user(State(s):State<AppState>,Path(user_id):Path<String>,headers:HeaderMap,Json(payload):Json<PinRequest>)->Result<Json<Value>,StatusCode>{
 let authenticated=authenticated_user(&headers)?;
 if authenticated!=user_id{return Err(StatusCode::FORBIDDEN)}
 set_pin(State(s),headers,Json(payload)).await
}
