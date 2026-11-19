# Qoin production deployment and operations

This runbook is the release gate for the Qoin application. A successful container build is not a production approval. Keep the system in research/paper mode until the explicit gates below have been met.

## Current operating boundary

- Vector V1 is the only decision-authority track. V2 Sets & Ledger is research, relationship discovery and provenance only; it must not route orders.
- The equal-share broker allocation endpoint produces a plan only. It does not connect accounts, check buying power, or submit orders.
- The live order adapter is currently Binance USD-M only. Other broker identifiers must remain unavailable until a real adapter, account lifecycle and reconciliation implementation is complete.
- Keep `QOIN_TRADING_MODE=disabled` and `QOIN_TRADING_ENABLED=false` for the initial production deployment. Do not put exchange credentials in a frontend build or mobile app.
- Do not describe a strategy as profitable or validated without preregistered out-of-sample results, realistic costs and independent review.

## Required deployment configuration

Create a private deployment `.env` from the examples. Never commit it.

Required values:
- `MONGO_INITDB_ROOT_USERNAME`, `MONGO_INITDB_ROOT_PASSWORD`
- `QOIN_MONGODB_APP_USERNAME`, `QOIN_MONGODB_APP_PASSWORD`
- `FIREBASE_WEB_API_KEY` (server-side Firebase Identity Toolkit verification)
- Firebase client configuration for each deployed client: `VITE_FIREBASE_*` for web and `EXPO_PUBLIC_FIREBASE_*` for mobile.
- Public client API and WebSocket origins: `VITE_QOIN_API_URL`, `VITE_QOIN_WS_URL`, `EXPO_PUBLIC_QOIN_API_URL`, `EXPO_PUBLIC_QOIN_WS_URL`.

Optional:
- `FIREBASE_IDENTITY_TOOLKIT_URL` only for a controlled emulator/test environment; production should use Google's HTTPS endpoint.
- `FINNHUB_API_KEY` for the optional equities/macro provider.
- Provider symbols and enable flags as required by the actual subscriptions.

Use unique, strong database credentials. If credentials contain URI-reserved characters, percent-encode them in the MongoDB connection URI or use URI-safe generated credentials. Keep MongoDB and Axum on the private Compose network. Expose only the gateway behind a TLS-terminating reverse proxy/WAF. Restrict administrative and internal endpoints to private network/operator access.

## First deployment

1. Provision a supported host, persistent encrypted volume for MongoDB, TLS certificate, DNS and a firewall. Permit public ingress only on HTTPS/WSS (normally 443); do not expose MongoDB or Axum's internal port.
2. Configure Firebase Authentication providers and authorized domains. Deploy and test `firebase/firestore.rules` and `firebase/storage.rules` before accepting users.
3. Populate secrets in the deployment secret manager or protected environment file. Verify file permissions and ensure secrets are excluded from logs, backups accessible to ordinary users, and source control.
4. Run `docker compose config --quiet` to validate interpolation, then `docker compose pull` / `docker compose build --pull` and `docker compose up -d`.
5. Verify `/health` returns HTTP 200 with MongoDB, Firebase server authentication and market recovery healthy. Verify the gateway health endpoint independently.
6. From a real web and mobile build, sign in with Firebase and verify authenticated reads, backtests and broker allocation planning. Confirm missing, malformed and expired tokens are rejected.
7. Verify provider timestamps, symbol/venue identity, stale-feed handling, reconnect behavior and event recovery. Confirm no duplicate events after a provider restart.
8. Verify that order entry remains disabled. Do not add exchange API credentials to production until paper-trading, risk and reconciliation gates have been signed off.

## Security and access controls

- Every user-specific API operation must authenticate the Firebase ID token server-side. Never trust a UID, email or role supplied in a request body.
- When adding user-owned MongoDB records, persist the verified principal UID as the owner and include it in every read/update/delete filter. Do not rely on frontend filtering for isolation.
- Keep provider ingestion, strategy mutation, trading controls and order reconciliation on private/internal interfaces. The public gateway must not proxy internal write routes.
- Use separate operator credentials and permissions from user authentication. Protect the execution token as a secret and rotate it; never put it in client code.
- Add rate limits and request/body limits at the public gateway/reverse proxy. Apply stricter limits to sign-in-adjacent and expensive historical/backtest routes.
- Use TLS for all public API and WebSocket traffic. Configure secure headers, CORS allowlists, origin checks and structured redacted logs at the edge.
- Do not log Firebase ID tokens, exchange keys, passwords, full authorization headers or sensitive account data.

## Data integrity and recovery

- MongoDB must use authentication, private networking, persistent storage, encrypted backups and tested point-in-time recovery.
- Take automated backups and run a scheduled restore drill. A backup that has not been restored successfully is not a verified backup.
- Monitor pending event-pipeline markers, replayed/skipped events, provider heartbeat/freshness, event gaps, WebSocket disconnects and database storage/replication health.
- Alert on nonzero skipped recovery events, growing pending queues, stale market feeds, repeated provider reconnects, duplicate/idempotency conflicts and database errors.
- Keep market events immutable. Derived state, vectors and decisions must be reproducible from the event log and versioned strategy configuration.

## Trading release gates

Trading remains disabled until all of these are independently demonstrated:
1. Firebase user identity and operator authorization are enforced server-side on every relevant endpoint.
2. Broker account connection uses least-privilege keys, encrypted secret storage and verified account ownership.
3. Instrument metadata, tick/lot sizes, minimum notionals, leverage, margin mode, trading status and quote currency are fetched and validated per broker.
4. Risk controls enforce per-order, per-symbol, per-account and portfolio exposure limits; max drawdown, daily loss, stale-data, spread, volatility and kill-switch controls are tested.
5. Order submission is idempotent and durable; timeouts and ambiguous responses are reconciled against broker state before retrying.
6. Partial fills, cancel/replace, rejects, fees, funding, liquidation, position drift and reconnect recovery are reconciled continuously.
7. Paper trading and walk-forward/out-of-sample tests cover normal, stressed, disconnected and degraded market conditions with realistic fees, slippage and latency.
8. A human operator has tested the kill switch and documented rollback and incident procedures.

Never turn on live execution by changing an environment variable alone. Require a reviewed release, operator approval and a verified broker-specific runbook.

## Release evidence to attach

For each release, retain the commit SHA, CI run links and results for Rust tests, C++ gateway build, web/mobile/desktop builds, integration tests, Firebase rules tests, container scan, dependency/security scan, backup restore drill and paper-trading report. A queued or skipped workflow is not a pass.

## Incident response

1. Disable new order entry using the operator kill switch and verify the disabled state at the backend.
2. Preserve logs, market events, decisions, order requests and broker responses; do not delete or rewrite audit records.
3. Reconcile every open order and position directly with the broker before restarting execution.
4. Restore from backup only after identifying the failure window and preserving the affected data for investigation.
5. Record root cause, customer impact, recovery steps and follow-up controls before re-enabling any trading capability.
