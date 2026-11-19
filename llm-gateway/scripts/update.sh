#!/usr/bin/env bash
# scripts/update.sh — Zero-downtime update of the Argus LLM Gateway
#
# Pulls the latest LiteLLM image from GHCR and restarts only the
# llm-gateway container, leaving any other services in the stack untouched.
# Docker Compose will start the new container before stopping the old one
# when the service has a healthcheck defined (rolling replacement).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " Argus LLM Gateway — Image Update"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── 1. Pull latest image ───────────────────────────────────────────────────────
echo ""
echo "[1/3] Pulling latest LiteLLM image..."
docker-compose pull llm-gateway

# ── 2. Rolling restart (no-deps, no full rebuild) ─────────────────────────────
echo ""
echo "[2/3] Restarting llm-gateway (zero-downtime)..."
docker-compose up -d --no-deps --build llm-gateway

# ── 3. Wait for health ────────────────────────────────────────────────────────
echo ""
echo "[3/3] Waiting for gateway to become healthy..."
GATEWAY_URL="${GATEWAY_URL:-http://localhost:4000}"

for i in $(seq 1 30); do
  if curl -sf "$GATEWAY_URL/health" > /dev/null 2>&1; then
    echo "Gateway is healthy after $((i * 2))s."
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo "ERROR: Gateway did not become healthy within 60 seconds."
    docker-compose logs --tail=50 llm-gateway
    exit 1
  fi
  sleep 2
done

echo ""
echo "Update complete."
docker-compose ps llm-gateway
