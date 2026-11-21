# Qoin container deployment baseline

The Compose topology exposes Drogon as the public gateway, keeps Axum private, and keeps MongoDB on a private network. Research and market-data endpoints remain separate from the authenticated execution boundary.

## Required configuration

Create a root `.env` file from `.env.example`. Generate distinct MongoDB passwords and a private `QOIN_EXECUTION_TOKEN` using a cryptographically secure random generator. Set `QOIN_FIREBASE_PROJECT_ID` to the Firebase project ID used by both frontend apps. Keep all values in the deployment secret manager in production; never put provider credentials in source control or frontend environment variables.

The Axum container receives `QOIN_FIREBASE_PROJECT_ID` and the private execution token. The Drogon gateway receives the same private execution token and injects it only on internal requests; clients must never receive it. Firebase ID tokens are forwarded from the client to Axum and verified against Google's Secure Token signing keys, issuer and audience. Axum derives the order owner UID from the verified token, never from the request body.

## Broker-neutral execution routes

Drogon proxies the following routes to the private Axum service:

- `POST /api/v1/orders` — submit one normalized order to the selected broker adapter.
- `GET /api/v1/orders/{client_order_id}/reconcile` — reconcile the order through its originally selected adapter and only for the authenticated owner.
- `GET /api/v1/trading/readiness` — inspect selected provider readiness; this route never submits orders.
- `GET /api/v1/brokers` — read the provider discovery catalogue.

The order request carries a stable `broker_id`, provider account reference, Qoin canonical instrument ID, side, order type, native quantity, optional limit/trigger prices and a protective stop. The adapter supplies the trusted account snapshot, instrument metadata, fresh bid/ask quote, native margin and provider-specific capabilities. The server clock is authoritative. The protective stop is passed to the provider adapter as an attached native bracket instruction; opening orders are rejected if the provider cannot guarantee that protection.

The backend persists the unique client order ID before submission. Duplicate requests are rejected, ambiguous timeouts require adapter-based reconciliation, and no retry is automatically issued. The execution kill switch is stored in MongoDB and defaults to disabled.

## Current capability boundary

The generic contract, adapter registry, account-currency risk gate, quote validation, Firebase owner authentication, gateway token injection, order ledger and reconciliation route are implemented on the feature branch. The registry is intentionally empty: the catalogue's Forex and crypto entries are discovery options, not implemented execution connectors. No broker account is connected and no provider is execution-ready. The readiness endpoint therefore reports no dispatch eligibility, and live dispatch must remain disabled.

A provider must not be registered until its adapter implements and passes provider-specific tests for account ownership, credential isolation/rotation, market data, instrument and contract sizing, native attached protection, margin, order submit/cancel/status, complete client-order-ID history lookup, partial fills, fee/funding reporting and reconciliation. Test/sandbox verification and operator approval are release gates. Do not paste API keys into chat or expose them to web/mobile clients.

## Start

```sh
docker compose build
docker compose up -d
docker compose ps
docker compose logs -f axum gateway
```

Only the gateway is published. Axum and MongoDB remain private. Before production exposure, add TLS ingress, rate and request-size limits, origin allowlists, managed secrets, managed MongoDB backups/PITR, monitoring and tested restore procedures.
