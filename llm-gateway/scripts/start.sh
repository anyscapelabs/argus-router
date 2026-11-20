#!/usr/bin/env bash
# scripts/start.sh — Start the Argus LLM Gateway
# Checks for a .env file, starts the Docker Compose stack, and polls /health
# until the gateway is ready or times out after 60 seconds.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

if [ ! -f ".env" ]; then
  echo "ERROR: .env file not found. Copy .env.example to .env and fill in values."
  exit 1
fi

echo "Starting Argus LLM Gateway..."
docker-compose up -d

echo "Waiting for health check..."
for i in $(seq 1 30); do
  if curl -sf http://localhost:4000/health/liveliness > /dev/null 2>&1; then
    echo "LLM Gateway is healthy!"
    exit 0
  fi
  sleep 2
done

echo "ERROR: Gateway did not become healthy in 60 seconds"
docker-compose logs --tail=50
exit 1
