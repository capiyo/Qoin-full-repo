# Qoin Firebase identity setup

Qoin uses Firebase Authentication as the client identity provider, Cloud Firestore for the user's private profile document, and Firebase Storage for profile pictures. Firebase client configuration values are public identifiers, not server secrets. Access control is enforced by the committed Firestore and Storage rules.

## Firebase Console

1. Create or select the Firebase project for Qoin.
2. Add a Web app and copy its Firebase client configuration.
3. In Authentication → Sign-in method, enable **Email/Password**.
4. Create the Cloud Firestore database.
5. Enable Firebase Storage and choose its bucket.
6. In Project settings → General, copy the web app's API key, auth domain, project ID, storage bucket, messaging sender ID, and app ID.
7. Add those values to the app-specific environment file:
   - Web: `frontend/apps/web/.env`, using the `VITE_FIREBASE_*` names in `frontend/apps/web/.env.example`.
   - Expo: `frontend/apps/mobile/.env`, using the `EXPO_PUBLIC_FIREBASE_*` names in `frontend/apps/mobile/.env.example`.
8. Deploy the rules from the repository root after selecting the intended Firebase project:
   ```sh
   firebase login
   firebase use --add
   firebase deploy --only firestore:rules,storage
   ```

The mobile client uses Firebase Auth persistence backed by AsyncStorage. The web client uses Firebase's browser persistence. Both clients share the same email/password identity, private user profile document, and avatar object path.

## Data and security contract

- Auth identity is the Firebase UID. No client-supplied user ID is trusted for profile writes.
- Profile metadata and the user's `archivedSymbols` list are stored at `users/{uid}`; users can read and update only their own document. Firestore rules whitelist the supported fields, validate profile string lengths and cap the Archive list at 500 entries. Archive changes are shared between web and mobile.
- Avatar objects are stored at `users/{uid}/avatar`.
- Avatar uploads are limited to JPEG, PNG, or WebP and 8 MiB.
- Storage reads require an authenticated user. Writes and deletes require the matching UID.
- Do not put Firebase service-account credentials in either frontend environment file or in Git.
- Deploy the Firestore and Storage rules before enabling avatar uploads.

The current account menu exposes profile details, notifications, avatar selection, and sign-out. Archive changes and historical replay are gated by an authenticated Firebase user. Market data and public research views remain browseable without signing in.

## Current scope

Firebase Authentication supplies the identity for authenticated backend routes. Configure `QOIN_FIREBASE_PROJECT_ID` in Axum (the project ID is public configuration). The Rust verifier validates Firebase Secure Token signatures, issuer, audience, expiry, issue time, authentication time and UID. Authenticated broker-account routes derive `owner_uid` exclusively from the verified bearer token; never trust a UID supplied in JSON or a query parameter. The Drogon gateway forwards the Authorization bearer token unchanged.

## Broker connection and credential storage

Axum exposes authenticated `/internal/users/me`, `/internal/broker-connections`, and `/internal/broker-accounts` routes. Drogon exposes these through `/api/v1/me`, `/api/v1/broker-connections`, and `/api/v1/broker-accounts`. A connection request is stored as `pending_validation` and `executable=false`; this is deliberate. It is not evidence that a provider accepted the credentials or that a trading account was discovered. Actual account discovery and provider authorization require a real registered broker adapter and are not fabricated by this registry.

Broker credentials are encrypted with AES-256-GCM before they are written to the separate `broker_credentials` collection. The service requires `QOIN_CREDENTIAL_ENCRYPTION_KEY_HEX`, a 32-byte (64 hexadecimal character) key injected at runtime by the deployment secret manager. Never commit this value, put it in frontend environment files, or log it. If the key is absent, broker connection creation fails closed while the rest of the research service can start. If the configured key is malformed, startup fails. Production deployment must use a managed secret/key service, restrict secret access to the Axum workload, and establish a tested key rotation/re-encryption procedure before accepting customer credentials.

Connection and account queries are scoped by the verified Firebase UID. Disconnecting a connection first marks it non-executable and revoked, removes its encrypted credential record, and disables any associated account records. Credential payloads are never returned by the API. A failed credential cleanup is surfaced as an operator-action error rather than reported as a fully completed disconnect.

The current adapter interface does not yet implement provider account discovery or credential validation. Therefore these endpoints establish secure ownership and storage, not a claim of live broker connectivity. Do not enable real-money execution based on a connection's existence.
