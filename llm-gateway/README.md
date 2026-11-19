# Argus LLM Gateway

The **Argus LLM Gateway** is a [LiteLLM Proxy](https://docs.litellm.ai/docs/proxy/quick_start) service that provides a single OpenAI-compatible endpoint for the Argus desktop application. All LLM traffic is routed through **OpenRouter** using a single API key. The Rust API gateway sits in front of this service and handles per-user authentication, rate-limiting, and request logging — this service only speaks to callers that present the internal service key.

---

## Quick Start

```bash
# 1. Copy the example env file and fill in your secrets
cp .env.example .env
$EDITOR .env   # set OPENROUTER_API_KEY and LITELLM_INTERNAL_KEY

# 2. Start the gateway
bash scripts/start.sh
```

The gateway will be available at **http://localhost:4000** (dev) or on the internal Docker network `argus-internal` (prod).

---

## Platform Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Argus Desktop (Tauri)                                       │
│  model = "argus/auto"  ──────────────────────────────────► │
└──────────────────────────────┬───────────────────────────────┘
                               │ HTTPS
                               ▼
┌──────────────────────────────────────────────────────────────┐
│  Rust API Gateway  (argus-router)                            │
│  • Per-user auth & rate limiting                             │
│  • Injects LITELLM_INTERNAL_KEY before forwarding            │
└──────────────────────────────┬───────────────────────────────┘
                               │ HTTP  (internal network only)
                               ▼
┌──────────────────────────────────────────────────────────────┐
│  LLM Gateway  ◄── THIS SERVICE                               │
│  LiteLLM Proxy  :4000                                        │
│  • Least-busy routing across argus/auto pool                 │
│  • Automatic failover & retries                              │
│  • Single OPENROUTER_API_KEY for all models                  │
└──────────────────────────────┬───────────────────────────────┘
                               │ HTTPS
                               ▼
                    ┌──────────────────┐
                    │   OpenRouter     │
                    │  openrouter.ai   │
                    └──────────────────┘
```

---

## Key Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/chat/completions` | Main chat endpoint (OpenAI-compatible, streaming supported) |
| `GET`  | `/v1/models` | List all configured models |
| `GET`  | `/health` | LiteLLM built-in health check |

> [!IMPORTANT]
> All endpoints require the `Authorization: Bearer <LITELLM_INTERNAL_KEY>` header.
> This key is **not** the OpenRouter key — it is the internal service-to-service secret.

---

## Configuration

Models are defined in [`config/litellm_config.yaml`](config/litellm_config.yaml).

- **`argus/auto`** — The default group. The desktop always sends `model: "argus/auto"`. LiteLLM picks the least-busy backend from the pool and falls back automatically on errors.
- **Named models** (e.g. `openai/gpt-6-luna`) — Available for explicit model selection from the desktop settings panel.

For local development, use the leaner config:

```bash
docker-compose run --rm llm-gateway \
  --config /app/config.dev.yaml --port 4000
```

Or mount the dev config in your compose override.

---

## How to Add a New Model from OpenRouter

1. **Find the model slug** on [openrouter.ai/models](https://openrouter.ai/models).  
   Example: `mistralai/mistral-nemo`

2. **Add an entry** to `config/litellm_config.yaml`:

```yaml
- model_name: mistral/mistral-nemo      # name the desktop sends
  litellm_params:
    model: openrouter/mistralai/mistral-nemo   # prefix with openrouter/
    api_key: os.environ/OPENROUTER_API_KEY
    api_base: https://openrouter.ai/api/v1
```

3. **Restart** the gateway to pick up the change:

```bash
bash scripts/update.sh
```

---

## Logs & Debugging

```bash
# Live logs
docker-compose logs -f llm-gateway

# Health + model status + last 20 log lines
bash scripts/health_check.sh

# One-shot health
curl http://localhost:4000/health
```

JSON-structured logs are written by default in production (`json_logs: true`). In dev, human-readable logs are enabled (`json_logs: false`) — see [`config/litellm_config.dev.yaml`](config/litellm_config.dev.yaml).

> [!TIP]
> Set `set_verbose: true` in the YAML (or use the dev config) to enable full LiteLLM debug output including raw request/response bodies.

---

## Optional: Extended Health Endpoint

[`src/health_extended.py`](src/health_extended.py) is a small FastAPI sidecar that exposes `GET /health/extended` with per-model availability data. It is **not required** — LiteLLM's `/health` is sufficient for most deployments.

```bash
pip install fastapi uvicorn httpx
uvicorn src.health_extended:app --port 4001
```

---

## Further Reading

- Full platform architecture: `docs/` *(see argus-router repo root)*
- LiteLLM Proxy docs: https://docs.litellm.ai/docs/proxy/quick_start
- OpenRouter model list: https://openrouter.ai/models
