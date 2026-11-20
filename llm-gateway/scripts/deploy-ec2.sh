#!/usr/bin/env bash
set -euo pipefail

region="${AWS_REGION:-eu-north-1}"
openrouter_parameter="/argus/llm-gateway/openrouter-api-key"
internal_key_parameter="/argus/llm-gateway/internal-key"

cd /opt/llm-gateway
umask 077

openrouter_api_key=$(aws ssm get-parameter \
  --region "$region" \
  --name "$openrouter_parameter" \
  --with-decryption \
  --query Parameter.Value \
  --output text)
internal_key=$(aws ssm get-parameter \
  --region "$region" \
  --name "$internal_key_parameter" \
  --with-decryption \
  --query Parameter.Value \
  --output text)

printf 'OPENROUTER_API_KEY=%s\nLITELLM_INTERNAL_KEY=%s\n' \
  "$openrouter_api_key" \
  "$internal_key" > .env
unset openrouter_api_key

docker compose \
  -f docker-compose.yml \
  -f docker-compose.prod.yml \
  config --quiet
docker compose \
  -f docker-compose.yml \
  -f docker-compose.prod.yml \
  pull --quiet
docker compose \
  -f docker-compose.yml \
  -f docker-compose.prod.yml \
  up -d --force-recreate --remove-orphans

for _ in $(seq 1 60); do
  if [ -n "$(docker compose \
    -f docker-compose.yml \
    -f docker-compose.prod.yml \
    ps -q --filter status=running llm-gateway 2>/dev/null)" ]; then
    echo "LLM gateway container is running"
    exit 0
  fi
  sleep 3
done

docker compose \
  -f docker-compose.yml \
  -f docker-compose.prod.yml \
  ps
docker compose \
  -f docker-compose.yml \
  -f docker-compose.prod.yml \
  logs --no-color --tail 100 llm-gateway
echo "LLM gateway health check timed out" >&2
exit 1
