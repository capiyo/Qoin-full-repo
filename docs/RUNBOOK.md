# Development runbook

1. Copy `.env.example` to a local environment file; never commit credentials.
2. Start MongoDB with `docker compose up -d mongodb` on a machine with Docker installed.
3. Start Axum from `services/axum` with `cargo run`; its default bind is loopback `127.0.0.1:8081`.
4. Build and start the Drogon gateway with CMake; it listens on loopback `127.0.0.1:8080` by default.
5. Check `GET /health`, then post schema-conforming events to `/api/v1/events` and inspect `/api/v1/state` and `/api/v1/vector`.

## Incident handling
- If event persistence fails, stop the feed and inspect MongoDB before resuming.
- If a state or decision write fails after event persistence, the event is still present; do not assume the response indicates that the entire pipeline committed atomically.
- Live order submission is not exposed by the Axum router. The isolated Binance adapter source is not wired into any route.
- Keep both services bound to loopback until authentication, authorization, rate limits, TLS, and production monitoring exist.
- A production kill switch, position reconciliation, stop management, and automated recovery are not implemented yet.
