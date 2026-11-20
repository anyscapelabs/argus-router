#!/usr/bin/env bash
# scripts/health_check.sh — Argus LLM Gateway health inspector
# Hits /health/liveliness and /v1/models, prints model list, then tails recent logs.
set -euo pipefail

GATEWAY_URL="${GATEWAY_URL:-http://localhost:4000}"
LOG_LINES="${LOG_LINES:-20}"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " Argus LLM Gateway — Health Check"
echo " Target: $GATEWAY_URL"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── 1. Basic health ────────────────────────────────────────────────────────────
echo ""
echo "[ /health/liveliness ]"
HEALTH_RESPONSE=$(curl -sf "$GATEWAY_URL/health/liveliness" 2>&1) || {
  echo "FAIL — could not reach $GATEWAY_URL/health/liveliness"
  echo "Is the container running?  docker-compose ps"
  exit 1
}
echo "$HEALTH_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$HEALTH_RESPONSE"

# ── 2. Model list ─────────────────────────────────────────────────────────────
echo ""
echo "[ /v1/models ]"
# LITELLM_INTERNAL_KEY may not be set in the shell; fall back to a placeholder
# so the curl still runs (unauthenticated calls return 401, which is informative).
AUTH_HEADER=""
if [ -n "${LITELLM_INTERNAL_KEY:-}" ]; then
  AUTH_HEADER="Authorization: Bearer $LITELLM_INTERNAL_KEY"
fi

MODELS_RESPONSE=$(curl -sf \
  ${AUTH_HEADER:+-H "$AUTH_HEADER"} \
  "$GATEWAY_URL/v1/models" 2>&1) || {
  echo "WARN — /v1/models returned an error (may need LITELLM_INTERNAL_KEY in env)"
  echo "$MODELS_RESPONSE"
}

if [ -n "$MODELS_RESPONSE" ]; then
  echo "Available models:"
  echo "$MODELS_RESPONSE" \
    | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for m in data.get('data', []):
        print(f\"  • {m.get('id','?')}\")
except Exception as e:
    print(sys.stdin.read())
" 2>/dev/null || echo "$MODELS_RESPONSE"
fi

# ── 3. Recent logs ────────────────────────────────────────────────────────────
echo ""
echo "[ Last $LOG_LINES log lines ]"
docker-compose logs --tail="$LOG_LINES" llm-gateway 2>/dev/null \
  || echo "(docker-compose not available or container not running)"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " Done."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
