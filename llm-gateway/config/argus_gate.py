import os
import random

import yaml
from fastapi import HTTPException
from litellm.integrations.custom_logger import CustomLogger

AUTO_ALIAS = "argus/auto"
AUTO_MODELS = frozenset({
    "bedrock/nova-lite",
    "bedrock/glm-4.7-flash",
})
VISION_MODELS = frozenset({
    "bedrock/nova-lite",
    "bedrock/nova-2-lite",
    "bedrock/nova-pro",
    "bedrock/kimi-k2.5",
    "bedrock/kimi-k3",
    "bedrock/kimi-k3.1",
    "bedrock/claude-haiku-4.5",
    "bedrock/claude-opus-4.5",
})
REQUIRED_HEADERS = (
    "x-argus-allowed-models",
    "x-argus-user-id",
    "x-argus-plan",
)


def _deployed_models():
    with open(os.environ.get("LITELLM_CONFIG", "/app/config.yaml")) as config_file:
        config = yaml.safe_load(config_file) or {}
    return {
        entry["model_name"]
        for entry in config.get("model_list", [])
        if entry.get("model_name") != AUTO_ALIAS
    }


DEPLOYED_MODELS = _deployed_models()


def _has_image(data):
    for message in data.get("messages") or []:
        content = message.get("content")
        if isinstance(content, list):
            for part in content:
                if isinstance(part, dict):
                    if part.get("type") in ("image_url", "image", "input_image"):
                        return True
                    if "image_url" in part:
                        return True
    return False


class ArgusGate(CustomLogger):
    async def async_pre_call_hook(self, user_api_key_dict, cache, data, call_type):
        metadata = data.get("metadata") or {}
        headers = metadata.get("headers") or {}

        missing = [name for name in REQUIRED_HEADERS if not headers.get(name)]
        if missing:
            raise HTTPException(
                status_code=403,
                detail="requests must come from the Argus API gateway",
            )

        allowed = {
            model.strip()
            for model in headers["x-argus-allowed-models"].split(",")
            if model.strip()
        }
        if not allowed:
            raise HTTPException(status_code=403, detail="no models are enabled for your plan")

        requested = data.get("model")
        if requested == AUTO_ALIAS:
            candidates = allowed & DEPLOYED_MODELS & AUTO_MODELS
            if not candidates:
                candidates = allowed & DEPLOYED_MODELS
            if _has_image(data):
                candidates &= VISION_MODELS
                if not candidates:
                    raise HTTPException(
                        status_code=403, detail="no vision-capable model is enabled for your plan"
                    )
            candidates = sorted(candidates)
            if not candidates:
                raise HTTPException(
                    status_code=403, detail="no allowed model is available for auto routing"
                )
            data["model"] = random.choice(candidates)
            return data

        if requested not in allowed:
            raise HTTPException(
                status_code=403, detail=f"model `{requested}` is not enabled for your plan"
            )
        if requested not in DEPLOYED_MODELS:
            raise HTTPException(status_code=404, detail=f"model `{requested}` is not available")
        return data


gate = ArgusGate()
