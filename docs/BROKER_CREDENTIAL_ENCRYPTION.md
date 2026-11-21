# Broker credential encryption

Qoin uses one credential-transport and storage encryption protocol: **libsodium sealed boxes** (crypto_box_seal / crypto_box_seal_open, X25519 + XSalsa20-Poly1305).

## Flow

1. The backend loads one matching 32-byte public/private keypair from its secret manager.
2. GET /api/v1/broker-credentials/public-key returns only the public key and protocol version.
3. Web and React Native clients use the shared @qoin/core createQoinBrokerConnection function. It fetches the public key, seals the JSON credential object in the client, and sends only sealed_credentials_hex over HTTPS.
4. Axum stores the opaque sealed ciphertext. It never receives plaintext credentials on the create-connection route.
5. Broker adapters retrieve credentials through the owner-scoped load_owned_credentials helper. Decrypted values remain server-side and must never be logged or returned to a client.

Sealed boxes are anonymous encryption, not sender authentication. Firebase ID-token verification and owner-scoped database queries remain mandatory for all connection writes and reads. HTTPS is also mandatory; sealed boxes do not replace transport security.

## Generate the backend keypair

Run from frontend/packages/core after installing workspace dependencies:

```sh
node --input-type=module -e 'import sodium from "libsodium-wrappers"; await sodium.ready; const pair = sodium.crypto_box_keypair(); console.log("QOIN_CREDENTIAL_SEAL_PUBLIC_KEY_HEX=" + sodium.to_hex(pair.publicKey)); console.log("QOIN_CREDENTIAL_SEAL_SECRET_KEY_HEX=" + sodium.to_hex(pair.privateKey));'
```

Store both values in the backend secret manager. The private key must never be put in frontend environment variables, source control, MongoDB, build artifacts, or client responses. The public key may be served by the backend endpoint.

Both variables are required together. Axum rejects a mismatched keypair at startup. Keep the previous keypair available during a controlled ciphertext migration/rotation; ciphertext encrypted to an old public key cannot be opened with a newly rotated private key.

## Request contract

```json
{
  "broker_id": "oanda",
  "display_name": "Primary OANDA",
  "auth_method": "api_key",
  "sealed_credentials_hex": "<libsodium crypto_box_seal ciphertext encoded as hex>"
}
```

MongoDB stores ciphertext_hex, encryption_algorithm=libsodium_crypto_box_seal, and encryption_version=1. It does not store a nonce field because the sealed-box format embeds the ephemeral public key and derives its nonce.

## Operational notes

- Use a distinct keypair per environment.
- Rotate by versioning the key and re-encrypting existing records before retiring the old private key.
- Never log request bodies, credential JSON, ciphertext payloads, or decrypted values.
- Do not treat a successful decrypt as broker authentication; each adapter must validate credentials with the provider API before the connection can be marked connected.
