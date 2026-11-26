# Broker credential security — active backend implementation

Updated: 2026-11-26  
Active implementation: `backend/rust-axum/src/credential_vault.rs` and the owner-scoped broker connection handlers in `backend/rust-axum/src/main.rs`.

## Current transport and storage behavior

1. The client sends broker credentials to the authenticated broker-connection endpoint over HTTPS using the user's Firebase ID token.
2. Axum verifies the Firebase identity and derives `owner_uid` from the verified token. A client-supplied UID is never accepted as ownership evidence.
3. Axum validates the credential shape and encrypts the credential JSON server-side using AES-256-GCM before MongoDB persistence.
4. MongoDB stores a versioned envelope containing `version`, `key_id`, `nonce` and `ciphertext`. The nonce is randomly generated for every encryption; the key ID is authenticated as AES-GCM additional authenticated data.
5. List, rotate and revoke operations filter by both connection ID and verified Firebase UID. API responses never contain the envelope or key identifier.
6. New and rotated connections always return to `pending_validation`, with `trading_enabled=false` and `execution_ready=false`.

This implementation encrypts credentials at rest. HTTPS/TLS is mandatory for transport. It does not claim client-side sealed-box encryption. The older libsodium sealed-box design described in earlier repository documentation is not the active path on this branch and must not be treated as an interoperable ciphertext format.

## Generate the active AES-256-GCM key

Generate 32 random bytes and encode them as base64:

```sh
openssl rand -base64 32
```

Set the output only in the deployment secret manager:

- `QOIN_CREDENTIAL_ENCRYPTION_KEY`: base64-encoded 32-byte key.
- `QOIN_CREDENTIAL_ENCRYPTION_KEY_ID`: stable identifier such as `v1`.

Never place the generated key in Git, frontend environment variables, React Native configuration, Tauri assets, logs, build artifacts or MongoDB.

If the active key is absent, credential create/rotate requests fail closed with service unavailable. A malformed key configuration prevents Axum startup.

## Key rotation

The vault supports a keyring with one active key and previous decryption keys.

1. Generate a new 32-byte key and a new key ID.
2. Deploy the new key as `QOIN_CREDENTIAL_ENCRYPTION_KEY` and the old key in `QOIN_CREDENTIAL_ENCRYPTION_PREVIOUS_KEYS`, a JSON object mapping old key IDs to base64 keys.
3. Re-encrypt each existing envelope with the active key using a controlled, audited migration. The current API supports decrypting envelopes in-process, but does not yet provide an operator migration endpoint.
4. Verify all persisted envelopes reference the new key ID and test backup restoration.
5. Retain the previous key through the rollback window; remove it only after the migration and recovery checks are complete.

Never delete an old key while any retained database backup or active record may still contain ciphertext encrypted under it.

## Current limits — do not overclaim

- The vault has unit tests for round-trip encryption, tamper rejection, unknown key IDs, payload size validation and previous-key decryption.
- Broker connection create/list/rotate/revoke is implemented with Firebase UID ownership filters and audit records.
- Provider-side credential validation is not implemented for the scaffolded providers.
- No concrete broker adapter currently consumes these per-user encrypted credential envelopes. Account discovery, instrument discovery and connection health remain incomplete.
- A successful AES-GCM decrypt is not proof that a broker accepts the credential.
- The API request body necessarily contains plaintext credentials in process memory during TLS request handling. Keep TLS mandatory, apply ingress body-size limits, avoid request-body logging, and zeroize application buffers after encryption.
- Audit collection writes are separate MongoDB writes; transactional atomicity between credential mutation and audit append remains a production hardening task.

## Required provider adapter behavior

A provider adapter may only retrieve credentials through a future server-side owner-scoped credential repository method. It must:
- query by both verified `owner_uid` and connection ID;
- decrypt only inside the trusted backend process;
- zeroize plaintext buffers;
- never log or return secret values;
- validate credentials against the official provider API;
- preserve `pending_validation` until validation and account ownership checks succeed;
- keep execution disabled unless the independent operator gate, user/account permissions, risk preflight, fresh market data and reconciliation controls all pass.
