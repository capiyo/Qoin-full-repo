# Qoin container deployment baseline

This repository includes a Docker Compose deployment topology with a public research gateway and a private Axum execution boundary. Live execution is disabled by default.

- Drogon is the only published application service.
- Axum is reachable only on the private Compose network.
- MongoDB is reachable only on the private Compose network and requires authentication.
- Axum and Drogon run as non-root users in their images.
- Provider credentials are injected through the untracked root `.env` file or the deployment platform's secret manager.

## Required configuration

Create a root `.env` file from `.env.example`. Generate both MongoDB passwords as 64-character hexadecimal strings (for example, `openssl rand -hex 32`) so they are safe to embed in the MongoDB URI. Use different values for the root and application passwords. If enabling the private execution route, generate `QOIN_EXECUTION_TOKEN` independently (for example, `openssl rand -hex 32`) and inject exchange credentials through the secret manager. Leave `QOIN_TRADING_MODE=disabled` unless intentionally enabling the operator-controlled route. Keep `.env` out of version control; the repository `.gitignore` excludes local environment files. Set `FINNHUB_API_KEY` only when the corresponding data entitlement is available. Binance public market data can be disabled independently.

## Private execution route

The Axum service exposes these operator-only routes on the private Compose network; Drogon does not proxy them:

- `POST /internal/orders` — submit one Binance USD-M market order with a mandatory side-correct protective stop for opening orders.
- `GET /internal/orders/{client_order_id}/reconcile` — query the entry order and its protective stop at Binance and persist the observed status.
- `POST /internal/trading/kill-switch` with `{"enabled":false}` — block new submissions immediately; `{"enabled":true}` reopens the entry gate. This does not cancel open orders or close positions.

All three require the `x-qoin-execution-token` header. New orders are blocked by default; explicitly enable the entry gate only after configuring and verifying the account. Every submission must use a unique 8–36 character `client_order_id`. The MongoDB ledger reserves that ID before any exchange request, so an uncertain result cannot be blindly retried. Use the reconciliation route to inspect it.

Example request body:

```json
{
  "symbol": "BTCUSDT",
  "side": "BUY",
  "quantity": 0.001,
  "stop_price": 60000,
  "client_order_id": "qoin-manual-000001"
}
```

For testnet, set `QOIN_TRADING_MODE=testnet` and the testnet confirmation string; the adapter selects and enforces the Binance Futures testnet host. For real-money operation, use `QOIN_TRADING_MODE=live` and the live confirmation string; the adapter enforces the production host. `BINANCE_FUTURES_BASE_URL` may be left blank so the selected mode supplies the correct host. Do not expose Axum's port publicly.

## Start

```sh
docker compose build
docker compose up -d
docker compose ps
docker compose logs -f axum gateway
```

The gateway health endpoint proxies the Axum health check. The Axum service is not published to the host. MongoDB data is held in the named `qoin-mongodb-data` volume.

## Production deployment requirements

This Compose file is a deployable baseline, not proof of production readiness. Before exposing the gateway publicly:

1. Put it behind a TLS-terminating reverse proxy or managed ingress.
2. Configure ingress-level rate limits, request-size limits, WebSocket connection limits, origin allowlists and DDoS controls.
3. Use managed secrets rather than a checked-in or image-baked environment file.
4. Use a managed MongoDB deployment with TLS, least-privilege database users, automated backups, point-in-time recovery and a tested restore procedure.
5. Configure provider-specific credentials and verify contractual data entitlements, symbol mapping, timestamp semantics and historical coverage.
6. Add external monitoring and alerting for gateway health, provider freshness, ingestion lag, MongoDB capacity and failed background tasks.
7. Keep Axum private. Do not publish its port or permit direct internet ingress.
8. Keep Axum private. The optional `POST /internal/orders` route is not proxied by Drogon and requires `QOIN_EXECUTION_TOKEN`, explicit live-mode confirmation, and Binance credentials. Configure the token through the deployment secret manager, not source control. The current route has durable client-order-ID deduplication and a protective-stop/emergency-close attempt, but it is not a complete portfolio risk engine or unattended production trading system; account reconciliation and an operational kill switch remain outstanding.

The Compose deployment uses Docker build-time dependency resolution. For a reproducible release, generate and commit the Rust `Cargo.lock`, pin base images by digest, generate an SBOM, scan images and deploy immutable image tags.
