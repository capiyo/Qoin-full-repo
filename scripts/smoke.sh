#!/usr/bin/env bash
set -euo pipefail
BASE_URL="${QOIN_GATEWAY_URL:-http://127.0.0.1:8080}"
curl --fail --silent --show-error "$BASE_URL/health"
printf '\n'
EVENT="{\"event_id\":\"smoke-$(date +%s)\",\"venue\":\"smoke\",\"symbol\":\"BTCUSDT\",\"event_type\":\"trade\",\"ts_ms\":$(date +%s%3N),\"price\":65000,\"size\":0.001,\"side\":\"buy\"}"
curl --fail --silent --show-error -H 'content-type: application/json' -d "$EVENT" "$BASE_URL/api/v1/events"
printf '\n'
curl --fail --silent --show-error "$BASE_URL/api/v1/state?symbol=BTCUSDT"
printf '\n'
curl --fail --silent --show-error "$BASE_URL/api/v1/vector?symbol=BTCUSDT"
printf '\n'
SIGNAL='{"observation":{"ofi":0.2,"liquidity":0.4,"volatility":0.1,"funding":0.0,"open_interest_change":0.1,"liquidation_pressure":0.0,"basis":0.0},"has_open_position":false}'
curl --fail --silent --show-error -H 'content-type: application/json' -d "$SIGNAL" "$BASE_URL/v1/signal"
printf '\n'
# The route must exist but must never return a user's plan without identity.
# In an unconfigured local environment the API returns 503; with Firebase
# configured and no bearer token it returns 401.
PLAN_STATUS="$(curl --silent --show-error -o /dev/null -w '%{http_code}' "$BASE_URL/v1/investment-plan")"
if [[ "$PLAN_STATUS" != "401" && "$PLAN_STATUS" != "503" ]]; then
  echo "investment-plan route did not fail closed (HTTP $PLAN_STATUS)" >&2
  exit 1
fi
printf 'investment-plan route is gated (HTTP %s)\n' "$PLAN_STATUS"
