//! Authenticated, versioned AES-256-GCM envelope encryption for user broker credentials.
//! The active key is deployment-managed; previous keys may be retained during rotation.
//! Ciphertext is never returned by API handlers and plaintext buffers are zeroized.
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredentialEnvelope {
    pub version: u8,
    pub key_id: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone)]
pub struct CredentialVault {
    active_key_id: String,
    keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
}

impl CredentialVault {
    pub fn from_env() -> Result<Option<Self>, String> {
        let active_raw = env::var("QOIN_CREDENTIAL_ENCRYPTION_KEY").unwrap_or_default();
        let active_id = env::var("QOIN_CREDENTIAL_ENCRYPTION_KEY_ID")
            .unwrap_or_else(|_| "v1".to_owned());
        let previous_raw = env::var("QOIN_CREDENTIAL_ENCRYPTION_PREVIOUS_KEYS").unwrap_or_default();
        if active_raw.trim().is_empty() {
            if !previous_raw.trim().is_empty() {
                return Err("previous credential keys require an active encryption key".into());
            }
            return Ok(None);
        }
        let active = decode_key(&active_raw)?;
        let mut keys = BTreeMap::new();
        keys.insert(active_id.clone(), Zeroizing::new(active));
        if !previous_raw.trim().is_empty() {
            let previous: BTreeMap<String, String> = serde_json::from_str(&previous_raw)
                .map_err(|_| "QOIN_CREDENTIAL_ENCRYPTION_PREVIOUS_KEYS must be a JSON object of key IDs to base64 keys".to_string())?;
            for (key_id, encoded) in previous {
                if key_id.trim().is_empty() || key_id == active_id {
                    return Err("previous credential key IDs must be non-empty and distinct from the active key ID".into());
                }
                keys.insert(key_id, Zeroizing::new(decode_key(&encoded)?));
            }
        }
        Ok(Some(Self::new(active_id, keys)?))
    }

    pub fn new(
        active_key_id: String,
        keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
    ) -> Result<Self, String> {
        if active_key_id.trim().is_empty() || active_key_id.len() > 64 {
            return Err("credential encryption key ID must contain 1 to 64 characters".into());
        }
        if !keys.contains_key(&active_key_id) {
            return Err("active credential encryption key is missing from the keyring".into());
        }
        Ok(Self { active_key_id, keys })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedCredentialEnvelope, String> {
        if plaintext.is_empty() || plaintext.len() > 16_384 {
            return Err("credential payload must contain 1 to 16384 bytes".into());
        }
        let key = self.keys.get(&self.active_key_id)
            .ok_or_else(|| "active credential encryption key is unavailable".to_string())?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|_| "credential encryption key is invalid".to_string())?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, Payload {
            msg: plaintext,
            aad: self.active_key_id.as_bytes(),
        }).map_err(|_| "credential encryption failed".to_string())?;
        Ok(EncryptedCredentialEnvelope {
            version: 1,
            key_id: self.active_key_id.clone(),
            nonce: STANDARD.encode(nonce.as_slice()),
            ciphertext: STANDARD.encode(ciphertext),
        })
    }

    pub fn decrypt(&self, envelope: &EncryptedCredentialEnvelope) -> Result<Zeroizing<Vec<u8>>, String> {
        if envelope.version != 1 || envelope.key_id.trim().is_empty() {
            return Err("credential envelope version or key ID is invalid".into());
        }
        let key = self.keys.get(&envelope.key_id)
            .ok_or_else(|| "credential envelope references an unavailable key ID".to_string())?;
        let nonce_bytes = STANDARD.decode(&envelope.nonce)
            .map_err(|_| "credential envelope nonce is malformed".to_string())?;
        if nonce_bytes.len() != 12 {
            return Err("credential envelope nonce must be 96 bits".into());
        }
        let ciphertext = STANDARD.decode(&envelope.ciphertext)
            .map_err(|_| "credential envelope ciphertext is malformed".to_string())?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|_| "credential decryption key is invalid".to_string())?;
        let plaintext = cipher.decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload { msg: &ciphertext, aad: envelope.key_id.as_bytes() },
        ).map_err(|_| "credential envelope authentication failed".to_string())?;
        Ok(Zeroizing::new(plaintext))
    }

    pub fn active_key_id(&self) -> &str {
        &self.active_key_id
    }
}

fn decode_key(encoded: &str) -> Result<[u8; 32], String> {
    let decoded = STANDARD.decode(encoded.trim())
        .map_err(|_| "credential encryption key must be base64 encoded".to_string())?;
    let key: [u8; 32] = decoded.try_into()
        .map_err(|_| "credential encryption key must decode to exactly 32 bytes".to_string())?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Zeroizing<[u8; 32]> {
        Zeroizing::new([byte; 32])
    }

    #[test]
    fn encrypts_and_authenticates_credentials() {
        let vault = CredentialVault::new("current".into(), BTreeMap::from([
            ("current".into(), key(7)),
        ])).unwrap();
        let secret = b"api-key=private;api-secret=never-return";
        let envelope = vault.encrypt(secret).unwrap();
        let serialized = serde_json::to_string(&envelope).unwrap();
        assert!(!serialized.contains("private"));
        assert_eq!(vault.decrypt(&envelope).unwrap().as_slice(), secret);
    }

    #[test]
    fn rejects_ciphertext_tampering() {
        let vault = CredentialVault::new("current".into(), BTreeMap::from([
            ("current".into(), key(8)),
        ])).unwrap();
        let mut envelope = vault.encrypt(b"secret").unwrap();
        envelope.ciphertext.push('A');
        assert!(vault.decrypt(&envelope).is_err());
    }

    #[test]
    fn decrypts_previous_key_during_rotation_but_encrypts_with_active_key() {
        let vault = CredentialVault::new("new".into(), BTreeMap::from([
            ("old".into(), key(1)),
            ("new".into(), key(2)),
        ])).unwrap();
        let old_vault = CredentialVault::new("old".into(), BTreeMap::from([
            ("old".into(), key(1)),
        ])).unwrap();
        let old = old_vault.encrypt(b"legacy-secret").unwrap();
        let new = vault.encrypt(b"new-secret").unwrap();
        assert_eq!(new.key_id, "new");
        assert_eq!(vault.decrypt(&old).unwrap().as_slice(), b"legacy-secret");
        assert_eq!(vault.decrypt(&new).unwrap().as_slice(), b"new-secret");
    }

    #[test]
    fn rejects_unknown_key_ids_and_invalid_payload_sizes() {
        let vault = CredentialVault::new("current".into(), BTreeMap::from([
            ("current".into(), key(9)),
        ])).unwrap();
        assert!(vault.encrypt(b"").is_err());
        let mut envelope = vault.encrypt(b"secret").unwrap();
        envelope.key_id = "retired-key".into();
        assert!(vault.decrypt(&envelope).is_err());
    }

    #[test]
    fn rejects_missing_active_key() {
        assert!(CredentialVault::new("missing".into(), BTreeMap::new()).is_err());
    }
}
