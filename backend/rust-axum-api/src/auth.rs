use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::{sync::Arc, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use tokio::sync::RwLock;

const FIREBASE_JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

#[derive(Debug, Clone, Deserialize)]
pub struct FirebaseClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    pub auth_time: u64,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub uid: String,
}

struct CachedJwks {
    expires_at: Instant,
    keys: Arc<JwkSet>,
}

pub struct FirebaseVerifier {
    project_id: String,
    issuer: String,
    http: reqwest::Client,
    cache: RwLock<Option<CachedJwks>>,
}

impl FirebaseVerifier {
    pub fn new(project_id: String) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("qoin-research-api")
            .build()?;
        let issuer = format!("https://securetoken.google.com/{project_id}");
        Ok(Self { project_id, issuer, http, cache: RwLock::new(None) })
    }

    async fn fetch_jwks(&self) -> Result<Arc<JwkSet>, String> {
        let response = self.http.get(FIREBASE_JWKS_URL).send().await
            .map_err(|_| "identity provider key service unavailable".to_owned())?
            .error_for_status()
            .map_err(|_| "identity provider key service returned an error".to_owned())?;
        let max_age = response.headers().get(reqwest::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').map(str::trim)
                .find_map(|part| part.strip_prefix("max-age=")?.parse::<u64>().ok()))
            .unwrap_or(3600)
            .clamp(60, 3600);
        let keys = Arc::new(response.json::<JwkSet>().await
            .map_err(|_| "identity provider returned invalid signing keys".to_owned())?);
        if keys.keys.is_empty() {
            return Err("identity provider returned no signing keys".to_owned());
        }
        *self.cache.write().await = Some(CachedJwks {
            expires_at: Instant::now() + Duration::from_secs(max_age),
            keys: Arc::clone(&keys),
        });
        Ok(keys)
    }

    async fn jwks(&self) -> Result<Arc<JwkSet>, String> {
        if let Some(cached) = self.cache.read().await.as_ref() {
            if cached.expires_at > Instant::now() {
                return Ok(Arc::clone(&cached.keys));
            }
        }
        self.fetch_jwks().await
    }

    pub async fn verify(&self, token: &str) -> Result<AuthenticatedUser, String> {
        let header = decode_header(token).map_err(|_| "invalid bearer token".to_owned())?;
        if header.alg != Algorithm::RS256 {
            return Err("unsupported bearer token algorithm".to_owned());
        }
        let kid = header.kid.ok_or_else(|| "bearer token has no signing key id".to_owned())?;
        let mut keys = self.jwks().await?;
        if keys.find(&kid).is_none() {
            // Google rotates signing keys. Refresh once on an unknown kid rather
            // than rejecting newly issued tokens until the old cache expires.
            keys = self.fetch_jwks().await?;
        }
        let jwk = keys.find(&kid).ok_or_else(|| "bearer token signing key is unknown".to_owned())?;
        let key = DecodingKey::from_jwk(jwk)
            .map_err(|_| "bearer token signing key is invalid".to_owned())?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.project_id.as_str()]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.leeway = 60;
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        let claims = decode::<FirebaseClaims>(token, &key, &validation)
            .map_err(|_| "bearer token validation failed".to_owned())?.claims;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is invalid".to_owned())?.as_secs();
        if claims.sub.trim().is_empty() || claims.sub.len() > 128 {
            return Err("bearer token subject is invalid".to_owned());
        }
        if claims.iat > now.saturating_add(60) || claims.auth_time > now.saturating_add(60)
            || claims.auth_time > claims.iat {
            return Err("bearer token timestamps are invalid".to_owned());
        }
        Ok(AuthenticatedUser { uid: claims.sub })
    }
}
