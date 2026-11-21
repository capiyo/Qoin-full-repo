#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${QOIN_GATEWAY_URL:-http://127.0.0.1:8080}"
CURL=(curl --fail --silent --show-error)

echo "[1/3] Gateway health"
"${CURL[@]}" "$BASE_URL/health" | tee /tmp/qoin-smoke-health.json
jq -e '.status == "ok" and .mongodb == true' /tmp/qoin-smoke-health.json >/dev/null

echo "[2/3] Instrument registry"
"${CURL[@]}" "$BASE_URL/api/v1/instruments" | tee /tmp/qoin-smoke-instruments.json
jq -e '.items | type == "array"' /tmp/qoin-smoke-instruments.json >/dev/null

echo "[3/3] Public write boundary"
status=$(curl --silent --show-error -o /tmp/qoin-smoke-public-write.json -w '%{http_code}' \
  -X POST "$BASE_URL/api/v1/events" \
  -H 'content-type: application/json' \
  --data '{"event_id":"smoke-forbidden","venue":"smoke","symbol":"BTCUSDT","event_type":"trade","ts_ms":1792860000000,"price":65000,"size":0.001,"side":"buy"}')
if [[ "$status" != "405" ]]; then
  echo "Expected public event writes to be rejected with HTTP 405; got $status" >&2
  cat /tmp/qoin-smoke-public-write.json >&2
  exit 1
fi

echo "Qoin gateway smoke checks passed."
