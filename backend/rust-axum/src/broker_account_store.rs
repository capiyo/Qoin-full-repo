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

pub async fn load_owned_credentials(
    collection: &mongodb::Collection<mongodb::bson::Document>,
    cipher: &CredentialCipher,
    owner_uid: &str,
    connection_id: &str,
) -> Result<Value, String> {
    use mongodb::bson::doc;
    let record = collection.find_one(
        doc!{"owner_uid":owner_uid,"connection_id":connection_id,"status":"active",
             "encryption_algorithm":"libsodium_crypto_box_seal","encryption_version":1},
        None,
    ).await.map_err(|_| "credential store unavailable".to_string())?
        .ok_or_else(|| "owned broker credentials not found".to_string())?;
    let ciphertext = record.get_str("ciphertext_hex")
        .map_err(|_| "sealed credential payload missing".to_string())?;
    cipher.decrypt_json(ciphertext)
}

impl Drop for CredentialCipher {
    fn drop(&mut self) {
        self.secret_key.0.zeroize();
    }
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
