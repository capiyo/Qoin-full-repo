# Qoin notifications

## Identity and storage

- All user-facing notification routes verify the Firebase ID token and derive the owner UID from that token. A request cannot choose another user's UID.
- Notification history is stored in MongoDB and scoped by `user_id`.
- Client-generated live stream events are restricted to the current user's own history, bounded in size, and deduplicated by a unique per-user event ID.
- Mark-read operations always include the authenticated UID in the database filter.
- Notification preferences are stored on the Qoin user record.
- FCM device tokens are unique and cannot be reassigned to another UID through registration.

## Public gateway routes

- `GET /api/v1/notifications`
- `GET /api/v1/notifications/unread-summary`
- `POST /api/v1/notifications/record`
- `POST /api/v1/notifications/mark-read`
- `GET /api/v1/notifications/preferences`
- `POST /api/v1/notifications/preferences`
- `POST /api/v1/notifications/register-token`

The private server-to-server `POST /internal/notifications/send` route is not exposed through Drogon. It requires `QOIN_INTERNAL_API_KEY`, validates the target Firebase UID, stores the notification, respects user preferences, and then attempts FCM delivery.

## FCM configuration

The backend uses the Firebase Cloud Messaging HTTP v1 API. Configure these backend-only deployment secrets:

- `QOIN_FIREBASE_PROJECT_ID`
- `FIREBASE_CLIENT_EMAIL`
- `FIREBASE_PRIVATE_KEY`
- `QOIN_INTERNAL_API_KEY`

Never expose service-account values through `VITE_*`, `EXPO_PUBLIC_*`, browser bundles, mobile builds, or API responses.

## Implementation boundary

- Persistent in-app history and Firebase-scoped record/read/preferences routes are implemented.
- The web and mobile Archive/Explore workspaces persist their live signal/liquidation notifications and hydrate history after Firebase sign-in.
- FCM delivery code and token registration endpoint are implemented, but device-token acquisition/registration is not yet wired into the shipped web/mobile startup flows. Push delivery is therefore not claimed as active until a real device token is registered and the Firebase Admin service-account environment is configured.
- Notification creation from the live client stream is an activity record for that signed-in user; it is not a trusted cross-user market alert. Trusted server-originated pushes must use the private internal route.
