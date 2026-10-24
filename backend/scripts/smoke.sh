#!/usr/bin/env bash
set -euo pipefail
BASE_URL="${QOIN_GATEWAY_URL:-http://127.0.0.1:8080}"
: "${QOIN_INGEST_TOKEN:?Set QOIN_ADMIN_TOKEN to the service token before running this authenticated smoke test}"
curl --fail --silent --show-error "$BASE_URL/health"
printf '\n'
EVENT="{\"event_id\":\"smoke-$(date +%s)\",\"venue\":\"smoke\",\"symbol\":\"BTCUSDT\",\"event_type\":\"trade\",\"ts_ms\":$(date +%s%3N),\"price\":65000,\"size\":0.001,\"side\":\"buy\"}"
curl --fail --silent --show-error -H "authorization: Bearer ${QOIN_ADMIN_TOKEN}" -H 'content-type: application/json' -d "$EVENT" "$BASE_URL/api/v1/events"
printf '\n'
curl --fail --silent --show-error "$BASE_URL/api/v1/state?symbol=BTCUSDT"
printf '\n'
curl --fail --silent --show-error "$BASE_URL/api/v1/vector?symbol=BTCUSDT"
printf '\n'
