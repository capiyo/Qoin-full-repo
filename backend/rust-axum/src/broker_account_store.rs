//! User-owned broker connection metadata and authenticated credential encryption.
//!
//! Credentials are encrypted before persistence. The encryption key must be injected
//! at runtime from a secret manager; it is never stored in MongoDB or returned by APIs.
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload, rand_core::RngCore},
    Aes256Gcm, Nonce,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroize;

#[derive(Clone)]
pub struct CredentialCipher {
    key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredential {
    pub nonce_hex: String,
    pub ciphertext_hex: String,
    pub encryption_version: i32,
}

impl CredentialCipher {
    pub fn from_hex_key(encoded: &str) -> Result<Self, String> {
        let bytes = hex::decode(encoded.trim())
            .map_err(|_| "credential encryption key must be 64 hexadecimal characters".to_string())?;
        if bytes.len() != 32 {
            return Err("credential encryption key must decode to exactly 32 bytes".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Self { key })
    }

    pub fn encrypt(&self, plaintext: &[u8], owner_uid: &str, connection_id: &str) -> Result<EncryptedCredential, String> {
        if plaintext.is_empty() || plaintext.len() > 32 * 1024 {
            return Err("broker credential payload must contain 1 to 32768 bytes".into());
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| "credential encryption is unavailable".to_string())?;
        let mut nonce_bytes = [0u8; 12];
        let mut rng = OsRng;
        rng.fill_bytes(&mut nonce_bytes);
        let aad = format!("qoin-broker-credential:v1:{}:{}", owner_uid, connection_id);
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: plaintext, aad: aad.as_bytes() })
            .map_err(|_| "broker credential encryption failed".to_string())?;
        Ok(EncryptedCredential {
            nonce_hex: hex::encode(nonce_bytes),
            ciphertext_hex: hex::encode(ciphertext),
            encryption_version: 1,
        })
    }

    pub fn decrypt(&self, encrypted: &EncryptedCredential, owner_uid: &str, connection_id: &str) -> Result<Vec<u8>, String> {
        let nonce = hex::decode(&encrypted.nonce_hex).map_err(|_| "invalid credential nonce".to_string())?;
        let ciphertext = hex::decode(&encrypted.ciphertext_hex).map_err(|_| "invalid credential ciphertext".to_string())?;
        if nonce.len() != 12 || encrypted.encryption_version != 1 {
            return Err("unsupported encrypted credential format".into());
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| "credential decryption is unavailable".to_string())?;
        let aad = format!("qoin-broker-credential:v1:{}:{}", owner_uid, connection_id);
        cipher.decrypt(Nonce::from_slice(&nonce), Payload { msg: ciphertext.as_ref(), aad: aad.as_bytes() })
            .map_err(|_| "credential authentication failed".to_string())
    }
}


pub async fn load_owned_credentials(
    collection: &mongodb::Collection<mongodb::bson::Document>,
    cipher: &CredentialCipher,
    owner_uid: &str,
    connection_id: &str,
) -> Result<Value, String> {
    use mongodb::bson::doc;
    let record = collection.find_one(
        doc!{"owner_uid":owner_uid,"connection_id":connection_id,"status":"active"}, None
    ).await.map_err(|_| "credential store unavailable".to_string())?
        .ok_or_else(|| "owned broker credentials not found".to_string())?;
    let encrypted = EncryptedCredential {
        nonce_hex: record.get_str("nonce_hex").map_err(|_| "credential nonce missing".to_string())?.to_string(),
        ciphertext_hex: record.get_str("ciphertext_hex").map_err(|_| "credential ciphertext missing".to_string())?.to_string(),
        encryption_version: record.get_i32("encryption_version").unwrap_or(0),
    };
    let mut plaintext = cipher.decrypt(&encrypted, owner_uid, connection_id)?;
    let parsed = serde_json::from_slice(&plaintext)
        .map_err(|_| "decrypted broker credential payload is invalid".to_string());
    plaintext.zeroize();
    parsed
}

impl Drop for CredentialCipher {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBrokerConnection {
    pub broker_id: String,
    pub display_name: Option<String>,
    pub auth_method: String,
    pub credentials: Value,
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
        if !self.credentials.is_object()
            || self.credentials.as_object().is_some_and(|value| value.is_empty())
        {
            return Err("credentials must be a non-empty JSON object".into());
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
    use serde_json::json;

    #[test]
    fn credentials_are_authenticated_and_encrypted_with_unique_nonces() {
        let cipher = CredentialCipher::from_hex_key(&"ab".repeat(32)).unwrap();
        let payload = br#"{"api_key":"secret","api_secret":"private"}"#;
        let first = cipher.encrypt(payload, "owner-1", "connection-1").unwrap();
        let second = cipher.encrypt(payload, "owner-1", "connection-1").unwrap();
        assert_ne!(first.nonce_hex, second.nonce_hex);
        assert_ne!(first.ciphertext_hex, String::from_utf8_lossy(payload));
        assert_eq!(cipher.decrypt(&first, "owner-1", "connection-1").unwrap(), payload);
        assert!(cipher.decrypt(&first, "owner-2", "connection-1").is_err());
        assert!(cipher.decrypt(&first, "owner-1", "connection-2").is_err());
        let mut tampered = first.clone();
        tampered.ciphertext_hex.replace_range(0..2, "00");
        assert!(cipher.decrypt(&tampered, "owner-1", "connection-1").is_err());
    }

    #[test]
    fn rejects_bad_keys_and_oversized_credentials() {
        assert!(CredentialCipher::from_hex_key("not-a-key").is_err());
        let cipher = CredentialCipher::from_hex_key(&"cd".repeat(32)).unwrap();
        assert!(cipher.encrypt(&vec![0u8; 32 * 1024 + 1], "owner", "connection").is_err());
    }

    #[test]
    fn connection_validation_normalizes_provider_and_rejects_invalid_payloads() {
        let request = CreateBrokerConnection {
            broker_id: " OANDA ".into(),
            display_name: Some(" Main OANDA ".into()),
            auth_method: "API_KEY".into(),
            credentials: json!({"api_key":"not-empty"}),
        };
        assert_eq!(request.validate().unwrap(), ("oanda".into(), "api_key".into(), "Main OANDA".into()));
        let invalid = CreateBrokerConnection { credentials: json!({}), ..request };
        assert!(invalid.validate().is_err());
    }
}
