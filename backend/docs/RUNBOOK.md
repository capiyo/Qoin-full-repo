# Development runbook

1. Copy `.env.example` to a local environment file; never commit credentials.
2. Start MongoDB with `docker compose up -d mongodb` on a machine with Docker installed.
3. Start Axum from `services/axum` with `cargo run`; its default bind is loopback `127.0.0.1:8081`.
4. Build and start the Drogon gateway with CMake; it listens on loopback `127.0.0.1:8080` by default.
5. Check `GET /health`, post schema-conforming events to `POST /api/v1/events`, and inspect `GET /api/v1/state?symbol=BTCUSDT` and `GET /api/v1/vector?symbol=BTCUSDT`.
6. Inspect the current strategy map with `GET /api/v1/strategy`. The first Axum startup seeds a validated baseline into MongoDB collection `edge_strategy` if no active map exists.

## Updating the research strategy map

Send a complete JSON document matching the returned map to `PUT /api/v1/strategy`. Keep the key `active`; increment `version` for each meaningful parameter change. Axum validates every required feature weight, feature scale, and threshold, persists the map to MongoDB, then activates it in memory. Do not treat a successful update as evidence of profitability. Keep all changes in research/paper mode and compare versions through chronological replay before promoting any map.

## Incident handling
- If event persistence fails, stop the feed and inspect MongoDB before resuming.
- If a state or decision write fails after event persistence, the event is still present; do not assume the response means the whole pipeline committed atomically.
- If the stored edge map is malformed, Axum fails startup rather than silently using different parameters.
- Live order submission is not exposed by the Axum router. The isolated Binance adapter source is not wired into any route.
- Keep both services bound to loopback until authentication, authorization, rate limits, TLS, and production monitoring exist.
- A production kill switch, position reconciliation, stop management, and full crash recovery are not implemented yet.
