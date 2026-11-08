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
- Profile metadata and the user's `archivedSymbols` list are stored at `users/{uid}`; users can read and update only their own document. Archive changes are shared between web and mobile.
- Avatar objects are stored at `users/{uid}/avatar`.
- Avatar uploads are limited to JPEG, PNG, or WebP and 8 MiB.
- Storage reads require an authenticated user. Writes and deletes require the matching UID.
- Do not put Firebase service-account credentials in either frontend environment file or in Git.
- Deploy the Firestore and Storage rules before enabling avatar uploads.

The current account menu exposes profile details, notifications, avatar selection, and sign-out. Archive changes and historical replay are gated by an authenticated Firebase user. Market data and public research views remain browseable without signing in.

## Current scope

This is Firebase client authentication and user-profile persistence. It does not yet create a separate Qoin backend JWT or authorize arbitrary future server-side write endpoints. Any future personalized or mutating Axum/Drogon route must verify Firebase ID tokens server-side and enforce UID ownership independently; never trust a UID sent in JSON or a query parameter.
