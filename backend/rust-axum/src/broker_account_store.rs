//! Frontend-to-backend broker credential encryption using libsodium sealed boxes.
//! The frontend encrypts with the public key; only the backend holds the private key.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroize;

#[derive(Clone)]
pub struct CredentialCipher {
    public_key: sodiumoxide::crypto::box_::PublicKey,
    secret_key: sodiumoxide::crypto::box_::SecretKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredential {
    pub ciphertext_hex: String,
    pub encryption_version: i32,
}

impl CredentialCipher {
    /// Load the optional broker credential-encryption keypair.
    ///
    /// Compose and local .env files commonly expose configured-but-empty variables.
    /// Treat two absent/blank values as disabled, while rejecting partial keypairs.
    pub fn from_optional_hex_keys(
        public_hex: Option<&str>,
        secret_hex: Option<&str>,
    ) -> Result<Option<Self>, String> {
        let public_hex = public_hex.map(str::trim).filter(|value| !value.is_empty());
        let secret_hex = secret_hex.map(str::trim).filter(|value| !value.is_empty());

        match (public_hex, secret_hex) {
            (None, None) => Ok(None),
            (Some(public_hex), Some(secret_hex)) => {
                Self::from_hex_keys(public_hex, secret_hex).map(Some)
            }
            _ => Err(
                "both QOIN_CREDENTIAL_SEAL_PUBLIC_KEY_HEX and QOIN_CREDENTIAL_SEAL_SECRET_KEY_HEX must be non-empty when broker credential encryption is enabled".to_string()
            ),
        }
    }

    pub fn from_hex_keys(public_hex: &str, secret_hex: &str) -> Result<Self, String> {
        sodiumoxide::init().map_err(|_| "libsodium initialization failed".to_string())?;
        let public = hex::decode(public_hex.trim())
            .map_err(|_| "credential public key must be 64 hexadecimal characters".to_string())?;
        let secret = hex::decode(secret_hex.trim())
            .map_err(|_| "credential private key must be 64 hexadecimal characters".to_string())?;
        let public_key = sodiumoxide::crypto::box_::PublicKey::from_slice(&public)
            .ok_or_else(|| "credential public key must decode to 32 bytes".to_string())?;
        let secret_key = sodiumoxide::crypto::box_::SecretKey::from_slice(&secret)
            .ok_or_else(|| "credential private key must decode to 32 bytes".to_string())?;
        Ok(Self { public_key, secret_key })
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key.0)
    }

    pub fn decrypt_hex(&self, ciphertext_hex: &str) -> Result<Vec<u8>, String> {
        if ciphertext_hex.is_empty() || ciphertext_hex.len() > 90_000 {
            return Err("sealed broker credential payload must contain 1 to 45000 bytes".into());
        }
        let ciphertext = hex::decode(ciphertext_hex)
            .map_err(|_| "sealed broker credential payload is not valid hexadecimal".to_string())?;
        sodiumoxide::crypto::sealedbox::open(&ciphertext, &self.public_key, &self.secret_key)
            .map_err(|_| "broker credential authentication failed".to_string())
    }

    pub fn decrypt_json(&self, ciphertext_hex: &str) -> Result<Value, String> {
        let mut plaintext = self.decrypt_hex(ciphertext_hex)?;
        let parsed = serde_json::from_slice(&plaintext)
            .map_err(|_| "decrypted broker credential payload is invalid".to_string());
        plaintext.zeroize();
        parsed
    }
}

/// Credential decryption is allowed only after a provider-specific validation
/// flow has marked the owner-scoped connection active, validated and executable.
pub fn connection_allows_credential_use(
    status: &str,
    validation_status: &str,
    executable: bool,
) -> bool {
    status == "active" && validation_status == "validated" && executable
}

pub async fn load_owned_credentials(
    connections: &mongodb::Collection<mongodb::bson::Document>,
    collection: &mongodb::Collection<mongodb::bson::Document>,
    cipher: &CredentialCipher,
    owner_uid: &str,
    connection_id: &str,
) -> Result<Value, String> {
    use mongodb::bson::doc;
    let connection = connections.find_one(
        doc!{"owner_uid":owner_uid,"connection_id":connection_id},
        None,
    ).await.map_err(|_| "broker connection store unavailable".to_string())?
        .ok_or_else(|| "owned broker connection not found".to_string())?;
    if !connection_allows_credential_use(
        connection.get_str("status").unwrap_or(""),
        connection.get_str("validation_status").unwrap_or(""),
        connection.get_bool("executable").unwrap_or(false),
    ) {
        return Err("broker connection has not passed provider validation or is not executable".into());
    }

    let record = collection.find_one(
        doc!{"owner_uid":owner_uid,"connection_id":connection_id,"status":"active",
             "encryption_algorithm":"libsodium_crypto_box_seal","encryption_version":1},
        None,
    ).await.map_err(|_| "credential store unavailable".to_string())?
        .ok_or_else(|| "owned broker credentials not found".to_string())?;
    let ciphertext = record.get_str("ciphertext_hex")
        .map_err(|_| "sealed credential payload missing".to_string())?;
    let credentials = cipher.decrypt_json(ciphertext)?;
    if !credentials.is_object() || credentials.as_object().is_some_and(|object| object.is_empty()) {
        return Err("decrypted broker credential payload must be a non-empty JSON object".into());
    }
    Ok(credentials)
}

#[derive(Debug, Deserialize)]
pub struct CreateBrokerConnection {
    pub broker_id: String,
    pub display_name: Option<String>,
    pub auth_method: String,
    pub sealed_credentials_hex: String,
}

impl CreateBrokerConnection {
    pub fn validate(&self) -> Result<(String, String, String), String> {
        let broker_id = self.broker_id.trim().to_ascii_lowercase();
        if broker_id.is_empty() || broker_id.len() > 64
            || !broker_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        {
            return Err("broker_id must be a valid provider identifier".into());
        }
        let auth_method = self.auth_method.trim().to_ascii_lowercase();
        if !matches!(auth_method.as_str(), "api_key" | "oauth2") {
            return Err("auth_method must be api_key or oauth2".into());
        }
        if self.sealed_credentials_hex.is_empty() || self.sealed_credentials_hex.len() > 90_000
            || !self.sealed_credentials_hex.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("sealed_credentials_hex must be a valid sealed-box hexadecimal payload".into());
        }
        let display_name = self.display_name.as_deref().unwrap_or(&broker_id).trim().to_string();
        if display_name.is_empty() || display_name.len() > 80 {
            return Err("display_name must contain 1 to 80 characters".into());
        }
        Ok((broker_id, auth_method, display_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_require_a_validated_executable_connection() {
        assert!(!connection_allows_credential_use("pending_validation", "not_attempted", false));
        assert!(!connection_allows_credential_use("active", "pending", false));
        assert!(!connection_allows_credential_use("disconnected", "validated", true));
        assert!(!connection_allows_credential_use("active", "validated", false));
        assert!(connection_allows_credential_use("active", "validated", true));
    }

    #[test]
    fn empty_optional_keypair_disables_credential_encryption() {
        assert!(CredentialCipher::from_optional_hex_keys(None, None)
            .unwrap()
            .is_none());
        assert!(CredentialCipher::from_optional_hex_keys(Some(""), Some("  "))
            .unwrap()
            .is_none());
    }

    #[test]
    fn optional_keypair_rejects_partial_configuration() {
        assert!(CredentialCipher::from_optional_hex_keys(Some("00"), None).is_err());
        assert!(CredentialCipher::from_optional_hex_keys(None, Some("00")).is_err());
        assert!(CredentialCipher::from_optional_hex_keys(Some("00"), Some("")).is_err());
    }

    #[test]
    fn optional_keypair_accepts_valid_key_material() {
        sodiumoxide::init().unwrap();
        let (public, secret) = sodiumoxide::crypto::box_::gen_keypair();
        let public_hex = hex::encode(public.0);
        let secret_hex = hex::encode(secret.0);
        let cipher = CredentialCipher::from_optional_hex_keys(
            Some(&public_hex),
            Some(&secret_hex),
        )
        .unwrap()
        .expect("a complete keypair enables credential encryption");
        assert_eq!(cipher.public_key_hex(), public_hex);
    }

    #[test]
    fn libsodium_sealed_box_round_trip_and_rejects_tampering() {
        sodiumoxide::init().unwrap();
        let (public, secret) = sodiumoxide::crypto::box_::gen_keypair();
        let cipher = CredentialCipher { public_key: public, secret_key: secret };
        let payload = br#"{"api_key":"secret","api_secret":"private"}"#;
        let sealed = sodiumoxide::crypto::sealedbox::seal(payload, &cipher.public_key);
        let encoded = hex::encode(&sealed);
        assert_eq!(cipher.decrypt_hex(&encoded).unwrap(), payload);
        let mut tampered = sealed;
        tampered[0] ^= 1;
        assert!(cipher.decrypt_hex(&hex::encode(tampered)).is_err());
    }

    #[test]
    fn rejects_invalid_key_material_and_payloads() {
        assert!(CredentialCipher::from_hex_keys("bad", "bad").is_err());
        let (public, secret) = sodiumoxide::crypto::box_::gen_keypair();
        let cipher = CredentialCipher { public_key: public, secret_key: secret };
        assert!(cipher.decrypt_hex("not-hex").is_err());
        assert!(cipher.decrypt_hex("").is_err());
    }

    #[test]
    fn validates_frontend_sealed_payload_contract() {
        let request = CreateBrokerConnection {
            broker_id: " OANDA ".into(),
            display_name: Some(" Main OANDA ".into()),
            auth_method: "API_KEY".into(),
            sealed_credentials_hex: "ab".repeat(80),
        };
        assert_eq!(request.validate().unwrap(), ("oanda".into(), "api_key".into(), "Main OANDA".into()));
        let invalid = CreateBrokerConnection { sealed_credentials_hex: String::new(), ..request };
        assert!(invalid.validate().is_err());
    }
}
