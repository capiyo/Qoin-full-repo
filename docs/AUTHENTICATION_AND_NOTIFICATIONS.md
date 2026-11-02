# Qoin authentication and notifications

Qoin ports Funspot's phone-first authentication experience and FCM notification lifecycle, with Qoin-owned sessions and account entitlements.

## Authentication flow

1. Web uses Firebase Phone Auth with invisible reCAPTCHA; native uses React Native Firebase phone verification.
2. The client sends the Firebase ID token to `POST /api/auth/firebase-login`.
3. Axum verifies the ID token signature against Google's rotating Firebase Secure Token JWKs, validates audience and issuer against `QOIN_FIREBASE_PROJECT_ID`, and derives the verified phone number from the token. A client-supplied phone number is never accepted as proof of ownership.
4. Existing users receive a Qoin HS256 session (12-hour expiry). New verified phone numbers complete username registration at `POST /api/auth/register`.
5. Users may set a four-digit fallback PIN after sign-in. PINs are bcrypt-hashed. Mobile stores the Qoin session in Expo SecureStore; web persists the session in localStorage.
6. Authenticated requests use the Qoin bearer token. Entitlements are returned by `GET /api/auth/me`; protected data endpoints must enforce them server-side.

## Auth routes

- `POST /api/auth/firebase-login` — `{ "id_token": "..." }`
- `POST /api/auth/register` — `{ "id_token": "...", "username": "..." }`
- `POST /api/auth/pin-login` — `{ "phone": "+...", "pin": "1234" }`
- `POST /api/auth/set-pin` — authenticated `{ "pin": "1234" }`
- `GET /api/auth/me` — authenticated session and profile
- `GET /api/auth/check-user/:phone` — existence/PIN fallback lookup

## Notification routes

- `POST /api/notifications/register-token` — authenticated FCM device registration
- `GET /api/notifications` — authenticated 50-item history
- `GET /api/notifications/unread-summary` — authenticated unread count
- `PUT /api/notifications/mark-read` — mark all or selected IDs read
- `GET /api/notifications/preferences`
- `PUT /api/notifications/preferences`
- `POST /internal/notifications` — internal-only persistence and FCM delivery; requires `x-qoin-internal-key`

## Required runtime configuration

Backend: `QOIN_JWT_SECRET` (32+ random chars), `QOIN_FIREBASE_PROJECT_ID`, `QOIN_INTERNAL_API_KEY` (32+ random chars), `FIREBASE_CLIENT_EMAIL`, and `FIREBASE_PRIVATE_KEY` (service-account PEM).

Web uses the Funspot Firebase web app configuration and VAPID key as defaults (`funzy-d56d7`); set `VITE_FIREBASE_*` and `VITE_FIREBASE_VAPID_KEY` to override them. The web client uses the same-origin gateway; native builds set `EXPO_PUBLIC_QOIN_API_URL` to the reachable Drogon URL.

Native builds require matching Firebase app files at `frontend/apps/mobile/google-services.json` and `frontend/apps/mobile/GoogleService-Info.plist`, provided through the deployment secret/configuration process. Configure phone sign-in, authorized domains, APNs credentials, and FCM in Firebase Console.

## Security and deployment notes

- The internal notification route is on the Axum loopback listener and is not forwarded by the public Drogon gateway.
- Never expose the internal API key, Firebase service-account private key, or JWT secret to either frontend.
- New accounts start with all market entitlements disabled. A trusted server-side entitlement grant is required before access.
- OTP and push require Firebase project setup and server credentials. Native credential files are intentionally not fabricated or committed.
