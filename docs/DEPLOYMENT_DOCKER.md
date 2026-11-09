# Qoin container deployment baseline

This repository includes a Docker Compose deployment topology for the **research and paper-trading** service boundary:

- Drogon is the only published application service.
- Axum is reachable only on the private Compose network.
- MongoDB is reachable only on the private Compose network and requires authentication.
- Axum and Drogon run as non-root users in their images.
- Provider credentials are injected through the untracked root `.env` file or the deployment platform's secret manager.

## Required configuration

Create a root `.env` file from `.env.example`. Set unique, strong values for `MONGO_INITDB_ROOT_USERNAME` and `MONGO_INITDB_ROOT_PASSWORD`. Keep this file out of version control. Set `FINNHUB_API_KEY` only when the corresponding data entitlement is available. Binance public market data can be disabled independently.

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
8. Keep order execution disabled. This deployment has no production-ready portfolio risk controls, authenticated order authorization, exchange reconciliation, protective-order lifecycle or independently validated live execution.

The Compose deployment uses Docker build-time dependency resolution. For a reproducible release, generate and commit the Rust `Cargo.lock`, pin base images by digest, generate an SBOM, scan images and deploy immutable image tags.
