//! Server-side verification of Firebase Authentication ID tokens.
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};

const FIREBASE_SECURETOKEN_JWKS: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

#[derive(Debug, Deserialize)]
struct FirebaseClaims {
    sub: String,
    aud: String,
    iss: String,
    exp: usize,
    iat: usize,
    auth_time: usize,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Clone)]
pub struct FirebaseIdentityVerifier {
    project_id: String,
    client: reqwest::Client,
    cached_keys: Arc<RwLock<Option<(Arc<JwkSet>, Instant)>>>,
    refresh_lock: Arc<Mutex<()>>,
}

impl FirebaseIdentityVerifier {
    pub fn new(project_id: String) -> Result<Self, String> {
        if project_id.trim().is_empty() {
            return Err("QOIN_FIREBASE_PROJECT_ID is required".into());
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|error| format!("Firebase verifier HTTP client: {error}"))?;
        Ok(Self {
            project_id,
            client,
            cached_keys: Arc::new(RwLock::new(None)),
            refresh_lock: Arc::new(Mutex::new(())),
        })
    }

    async fn jwks(&self) -> Result<Arc<JwkSet>, String> {
        if let Some((keys, expires_at)) = self.cached_keys.read().await.as_ref() {
            if Instant::now() < *expires_at {
                return Ok(keys.clone());
            }
        }

        let _refresh = self.refresh_lock.lock().await;
        if let Some((keys, expires_at)) = self.cached_keys.read().await.as_ref() {
            if Instant::now() < *expires_at {
                return Ok(keys.clone());
            }
        }

        let response = self.client.get(FIREBASE_SECURETOKEN_JWKS).send().await
            .map_err(|error| format!("Firebase signing-key request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Firebase signing-key service returned HTTP {}", response.status()));
        }
        let ttl_seconds = response.headers().get(reqwest::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').map(str::trim)
                .find_map(|directive| directive.strip_prefix("max-age="))
                .and_then(|value| value.parse::<u64>().ok()))
            .unwrap_or(3600)
            .saturating_sub(60)
            .max(60);
        let keys = Arc::new(response.json::<JwkSet>().await
            .map_err(|error| format!("Firebase signing-key document is invalid: {error}"))?);
        if keys.keys.is_empty() {
            return Err("Firebase signing-key document contains no keys".into());
        }
        *self.cached_keys.write().await = Some((keys.clone(), Instant::now() + Duration::from_secs(ttl_seconds)));
        Ok(keys)
    }

    pub async fn verify_id_token(&self, token: &str) -> Result<String, String> {
        if token.trim().is_empty() || token.len() > 16_384 {
            return Err("Firebase ID token is missing or malformed".into());
        }
        let header = decode_header(token).map_err(|_| "Firebase ID token header is invalid".to_string())?;
        if header.alg != Algorithm::RS256 {
            return Err("Firebase ID token must use RS256".into());
        }
        let kid = header.kid.ok_or_else(|| "Firebase ID token has no signing key ID".to_string())?;
        let keys = self.jwks().await?;
        let jwk = keys.keys.iter().find(|key| key.common.key_id.as_deref() == Some(kid.as_str()))
            .ok_or_else(|| "Firebase ID token signing key is unknown or rotated".to_string())?;
        let decoding_key = DecodingKey::from_jwk(jwk)
            .map_err(|_| "Firebase ID token signing key is invalid".to_string())?;

        let issuer = format!("https://securetoken.google.com/{}", self.project_id);
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.project_id.as_str()]);
        validation.set_issuer(&[issuer.as_str()]);
        validation.leeway = 30;
        validation.validate_exp = true;
        let token_data = decode::<FirebaseClaims>(token, &decoding_key, &validation)
            .map_err(|_| "Firebase ID token signature or claims are invalid".to_string())?;
        let claims = token_data.claims;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Server clock is before Unix epoch".to_string())?.as_secs() as usize;
        let uid = claims.sub.trim();
        if uid.is_empty() || uid.len() > 128
            || claims.user_id.as_deref().is_some_and(|user_id| user_id != claims.sub)
            || claims.iat > now.saturating_add(60)
            || claims.auth_time > claims.iat
            || claims.auth_time > now.saturating_add(60)
            || claims.aud != self.project_id
            || claims.iss != issuer
            || claims.exp <= now
        {
            return Err("Firebase ID token identity or time claims are invalid".into());
        }
        Ok(uid.to_string())
    }
}
