# Qoin development and operations runbook

Audit date: 2026-11-28  
Authoritative branch: `master`

## Operating boundary

- `master` is the only development, integration, verification, and release branch. Do not create feature or integration branches.
- Rust/Axum is the authoritative vector and research engine. C++/Drogon is the HTTP/WebSocket gateway only. MongoDB is the persistence layer.
- V1 vector decisions are research signals. V2 set/relationship ledgers are evidence and attribution tools; they are not order authority.
- The broker registry starts empty. Provider catalogue entries and environment credentials do not constitute a connected or executable adapter.
- The private Axum order routes are fail-closed. The persisted operator kill switch always initializes disabled. No real-money order dispatch is authorized.
- Paper-session and operating-cycle Rust modules exist, but the authenticated APIs, per-user durable lifecycle, replay-safe recovery, and client controls are not integrated on `master` yet.

## Local configuration

1. Work from a clean clone or checkout of `master`.
2. Copy `.env.example` to a private local environment file. Never commit the local file or paste real secrets into issues, logs, frontend variables, or source code.
3. Start a MongoDB server accessible through `QOIN_MONGODB_URI`. Docker is optional; a separately installed local MongoDB service is also supported.
4. Set `QOIN_MONGODB_DATABASE` and `QOIN_MONGODB_URI`. Use a dedicated database user with only the permissions required by Qoin.
5. Configure `QOIN_FIREBASE_PROJECT_ID` for server-side Firebase ID-token verification. Private user-scoped routes must fail closed when identity verification is unavailable.
6. Configure `QOIN_EXECUTION_TOKEN` and `QOIN_LEDGER_MAINTENANCE_TOKEN` as strong backend-only secrets for the restricted operator/maintenance routes. Never put them in Vite or Expo public variables.
7. Add provider credentials only as backend `QOIN_BROKER_*` environment values. These values do not register an adapter or enable dispatch.
8. Configure optional read-only market feeds with the provider enable flags and symbols. `FINNHUB_API_KEY` is optional and required only for the configured Finnhub market/news services.

## Build and verification

Run the checks from the repository root:

```sh
cargo test --workspace
pnpm install
pnpm --filter @qoin/react-web build
pnpm --filter @qoin/mobile exec tsc --noEmit
```

Run the Rust engine, web, mobile, desktop, Drogon, and integration workflows against the exact `master` commit being considered. A queued, skipped, stale, or failed workflow is not a pass. Do not claim local verification unless the commands were actually run in the target environment.

## Service startup

- Start MongoDB and verify that the configured database user can connect.
- Start Axum on its private loopback address using the workspace manifest for `backend/rust-axum`.
- Build and start Drogon on its configured gateway address. Keep the Axum listener and MongoDB private; expose only the gateway through a TLS-terminating reverse proxy.
- Check `GET /health`. Confirm MongoDB connectivity, market-event recovery status, and the reported execution-adapter registry state.
- Verify that market events are accepted only with canonical venue, symbol, event ID, event type, and positive timestamp values. Confirm duplicate delivery does not create duplicate source events or ledger records.
- Verify the WebSocket gateway forwards the event, state, and research decision for the subscribed symbol.

## Security and execution controls

- Firebase ID tokens are verified server-side. Derive the owner UID from the verified token; never trust a user ID, role, or account owner supplied in a request body.
- User-owned MongoDB reads and writes must include the verified owner UID in their filters.
- Keep broker credentials on the server. Never send them to the web or mobile clients, Firestore profiles, logs, or API responses.
- Keep internal Axum routes private. Do not expose the internal order, trading-control, broker provisioning, or ledger maintenance routes through the public Drogon gateway.
- The operator kill switch is initialized to `false` by the backend on first database initialization. There is no environment variable that may arm it.
- Even if the operator control is manually changed, the empty broker registry and missing provider adapters must continue to prevent order dispatch.
- A provider appearing in the catalogue or having credentials configured is not proof of adapter readiness, account ownership, buying power, or reconciliation support.
- Never interpret a research signal, backtest, allocation plan, or paper-session library result as a real order or a promise of profitability.

## Data integrity and recovery

- Preserve the immutable market-event log. Do not edit or delete source events to repair downstream state.
- If a state, decision, vector, or ledger write fails after event persistence, inspect the durable event and derived collections before retrying. The event pipeline is not a single cross-collection transaction.
- Monitor recovery replayed/skipped event counts, provider freshness, WebSocket disconnects, event gaps, database availability, and disk capacity.
- Back up MongoDB using an authenticated, encrypted process. Test restores periodically; an untested backup is not a verified backup.
- On an incident, stop new ingestion if event ordering or persistence integrity is uncertain, preserve logs and source events, restore only after identifying the failure window, and document the recovery.

## Production release gate

A successful build is not production approval. Before any public production release, independently verify authentication and authorization, network isolation, secret rotation, provider data quality, chronological out-of-sample research, portfolio-level risk, backups and restore drills, monitoring, incident response, and accessibility/security review.

Before paper execution is described as integrated, complete the authenticated per-user operating-cycle and paper-session APIs, durable idempotent session event processing, crash recovery/replay, history endpoints, and client diagnostics.

Real-money execution remains unauthorized. Do not enable it based on a green CI run, a broker catalogue entry, or environment credentials.
