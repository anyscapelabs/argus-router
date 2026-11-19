# Argus LLM Gateway — EC2 Deployment Guide

> [!IMPORTANT]
> The LiteLLM proxy is an **internal-only** service. It must never be directly reachable from the public internet. All public traffic goes through the Rust gateway.

---

## 1. Prerequisites

### 1.1 EC2 Instance

| Requirement | Minimum | Recommended |
|---|---|---|
| Instance type | `t3.small` (2 vCPU / 2 GB RAM) | `t3.medium` (2 vCPU / 4 GB RAM) |
| OS | Ubuntu 22.04 LTS | Ubuntu 22.04 LTS |
| VPC | Same VPC as the Rust gateway | Same VPC + same subnet |
| Security Group (inbound) | SSH (22) from your IP only | SSH (22) from your IP only |
| Security Group (internal) | Port 4000 from Rust gateway SG | Port 4000 from Rust gateway SG |

> [!CAUTION]
> Do **not** add a public inbound rule for port 4000. LiteLLM should only be reachable from within the VPC (or localhost if co-located with Rust).

### 1.2 Software Requirements

- Docker Engine ≥ 24.0
- Docker Compose plugin ≥ 2.20
- Git (to clone/pull the repo)

---

## 2. Install Docker and Docker Compose

```bash
# Update apt and install dependencies
sudo apt-get update
sudo apt-get install -y ca-certificates curl gnupg

# Add Docker's official GPG key
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | \
  sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

# Add Docker repository
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
  https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

# Install Docker Engine and Compose plugin
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io \
  docker-buildx-plugin docker-compose-plugin

# Allow current user to run docker without sudo
sudo usermod -aG docker $USER
newgrp docker

# Verify
docker --version
docker compose version
```

---

## 3. Environment Variables

Create a `.env` file in the project root (next to `docker-compose.yml`). **Never commit this file.**

```bash
# /home/brnx/Desktop/argus-router/llm-gateway/.env

# --- Required ---

# Your OpenRouter API key — all LLM traffic flows through this
OPENROUTER_API_KEY=sk-or-v1-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

# Service-to-service key — Rust gateway uses this to authenticate with LiteLLM.
# Generate a strong random value: openssl rand -hex 32
# This same value must be set as LITELLM_INTERNAL_KEY in the Rust gateway's env.
LITELLM_INTERNAL_KEY=sk-internal-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

# --- Optional ---

# LiteLLM internal master key (can be same as LITELLM_INTERNAL_KEY or separate)
LITELLM_MASTER_KEY=${LITELLM_INTERNAL_KEY}
```

> [!WARNING]
> `LITELLM_INTERNAL_KEY` must match exactly what is configured in the Rust gateway's `LLM_GATEWAY_INTERNAL_KEY` env var. A mismatch means all forwarded chat completions will return 401.

Add `.env` to `.gitignore`:

```bash
echo ".env" >> .gitignore
```

---

## 4. Running with Docker Compose

### 4.1 Start the Service

```bash
cd /home/brnx/Desktop/argus-router/llm-gateway

# Start in detached mode
docker compose up -d

# Verify it's running
docker compose ps
```

Expected output:
```
NAME                    IMAGE              COMMAND     SERVICE       STATUS     PORTS
argus-llm-gateway       ghcr.io/berriai/litellm:...   ...   llm-gateway   running    4000/tcp
```

### 4.2 Verify Health

```bash
curl -f http://localhost:4000/health
# Expected: {"status": "healthy"}
```

---

## 5. Update the Rust Gateway's Forwarding URL

The Rust gateway needs to know where to send chat completions. Set these environment variables in the Rust gateway's deployment (systemd unit, docker-compose, or EC2 user data):

```bash
# If LiteLLM is on the same EC2 instance as Rust:
LLM_GATEWAY_URL=http://localhost:4000

# If LiteLLM is in Docker Compose on the same host (using Docker bridge network):
LLM_GATEWAY_URL=http://llm-gateway:4000

# If LiteLLM is on a separate EC2 instance in the same VPC:
LLM_GATEWAY_URL=http://10.0.1.42:4000   # replace with actual private IP

# Service-to-service auth key (must match LITELLM_INTERNAL_KEY above)
LITELLM_INTERNAL_KEY=sk-internal-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

After setting these, restart the Rust gateway process or container.

---

## 6. Health Check

```bash
# Basic health
curl http://localhost:4000/health

# With auth (if master key is set)
curl -H "Authorization: Bearer $LITELLM_MASTER_KEY" http://localhost:4000/health

# Check available models
curl -H "Authorization: Bearer $LITELLM_INTERNAL_KEY" http://localhost:4000/v1/models
```

---

## 7. Logs

```bash
# Tail live logs
docker logs -f argus-llm-gateway

# Last 100 lines
docker logs --tail 100 argus-llm-gateway

# Since a specific time
docker logs --since "2024-01-15T10:00:00" argus-llm-gateway
```

---

## 8. Updating Models

Models are configured in `config/litellm_config.yaml`. To add or remove models:

```bash
# 1. Edit the config
nano config/litellm_config.yaml

# 2. Apply changes (restarts LiteLLM container)
docker compose restart llm-gateway

# 3. Confirm the new models are live
curl -H "Authorization: Bearer $LITELLM_INTERNAL_KEY" \
  http://localhost:4000/v1/models | jq '.data[].id'
```

> [!NOTE]
> LiteLLM reloads config on startup. A restart is instant (< 5 seconds) with Docker Compose. For zero-downtime, see Section 9.

---

## 9. Rolling Update (Zero Downtime)

For production deployments where you cannot tolerate a gap in service:

```bash
# Option A: Blue-Green with two Compose services (recommended)
# Run a second instance on port 4001, test it, then update the Rust gateway URL,
# then stop the old instance.

# Option B: In-place with minimal downtime (acceptable for internal service)
# Rust gateway will get 502s for ~3-5 seconds during restart.

# Pull the latest image
docker compose pull

# Restart with new image (brief downtime)
docker compose up -d --force-recreate

# Verify
curl -f http://localhost:4000/health
```

> [!TIP]
> Because the Rust gateway is the public-facing layer, brief LiteLLM restarts only affect in-flight streaming requests. New connections queue in Rust until LiteLLM is back up (if Rust implements a small retry on 502). The desktop will show a brief error for active streams.

### 9.1 Deploying a Config Change Without Downtime

If only changing `litellm_config.yaml` (not the Docker image):

```bash
# Copy updated config into running container and send SIGHUP (if LiteLLM supports it)
docker cp config/litellm_config.yaml argus-llm-gateway:/app/config/litellm_config.yaml
docker compose restart llm-gateway
```

---

## 10. Monitoring

### 10.1 Prometheus Metrics

LiteLLM exposes a Prometheus-compatible `/metrics` endpoint:

```bash
curl http://localhost:4000/metrics
```

Key metrics to watch:

| Metric | Description |
|---|---|
| `litellm_requests_total` | Total requests by model and status |
| `litellm_request_duration_seconds` | Latency histogram |
| `litellm_llm_api_failed_requests_total` | Failed upstream calls |
| `litellm_remaining_tokens` | Token budget remaining (if configured) |

### 10.2 Scrape Config (Prometheus)

```yaml
# prometheus.yml snippet
scrape_configs:
  - job_name: 'argus-llm-gateway'
    static_configs:
      - targets: ['localhost:4000']  # or private IP if Prometheus is external
    metrics_path: /metrics
    scrape_interval: 15s
```

### 10.3 Simple Uptime Check

Add to your monitoring system (e.g., UptimeRobot, Grafana OnCall):

```
URL: http://<private-ip>:4000/health
Expected response: 200 OK
Check interval: 30s
Alert after: 2 consecutive failures
```

---

## 11. Troubleshooting

| Symptom | Likely Cause | Fix |
|---|---|---|
| `curl /health` returns connection refused | Container not running | `docker compose up -d` |
| Rust gateway gets 401 from LiteLLM | `LITELLM_INTERNAL_KEY` mismatch | Verify both sides have the same value |
| `argus/auto` returns "model not found" | Model not in `litellm_config.yaml` | Add to config and restart |
| High latency on first request | Cold start of OpenRouter connection | Expected; subsequent requests are fast |
| Container OOM killed | Too many concurrent streams | Upgrade to `t3.medium` or add `mem_limit` in Compose |
