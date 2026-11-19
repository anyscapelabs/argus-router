"""
src/health_extended.py — Optional extended health endpoint for Argus LLM Gateway.

NOTE: This module is OPTIONAL.
LiteLLM's built-in GET /health endpoint is sufficient for the vast majority of
production deployments.  Only run this sidecar when you need richer per-model
availability data than /health provides (e.g. for a custom dashboard).

Usage:
    pip install fastapi uvicorn httpx
    GATEWAY_URL=http://localhost:4000 uvicorn src.health_extended:app --port 4001
"""

import os
import httpx
from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse

app = FastAPI(title="Argus LLM Gateway — Extended Health", version="0.1.0")

GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:4000")
INTERNAL_KEY = os.environ.get("LITELLM_INTERNAL_KEY", "")
_HEADERS = {"Authorization": f"Bearer {INTERNAL_KEY}"} if INTERNAL_KEY else {}


@app.get("/health/extended", summary="Extended health with per-model availability")
async def health_extended() -> JSONResponse:
    """Fetches /health and /v1/models from LiteLLM and merges them."""
    async with httpx.AsyncClient(timeout=10) as client:
        try:
            health_r = await client.get(f"{GATEWAY_URL}/health", headers=_HEADERS)
            models_r = await client.get(f"{GATEWAY_URL}/v1/models", headers=_HEADERS)
        except httpx.RequestError as exc:
            raise HTTPException(status_code=503, detail=f"Gateway unreachable: {exc}")

    health = health_r.json() if health_r.status_code == 200 else {"error": health_r.text}
    models = models_r.json() if models_r.status_code == 200 else {"error": models_r.text}

    return JSONResponse({
        "status": "ok" if health_r.status_code == 200 else "degraded",
        "litellm_health": health,
        "models": models.get("data", []),
    })
