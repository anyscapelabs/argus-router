import json
import os

import yaml
from fastapi import HTTPException
from litellm.integrations.custom_logger import CustomLogger

AUTO_ALIAS = "argus/auto"
CATALOG_PATH = os.environ.get("ARGUS_CATALOG_PATH", "/app/catalog/models.json")
REQUIRED_HEADERS = (
    "x-argus-allowed-models",
    "x-argus-user-id",
    "x-argus-plan",
)

with open(CATALOG_PATH) as catalog_file:
    CATALOG = json.load(catalog_file)

BY_ID = {entry["id"]: entry for entry in CATALOG}
AUTO_ENTRIES = sorted(
    (
        entry
        for entry in CATALOG
        if entry.get("auto") and entry["id"] != AUTO_ALIAS
    ),
    key=lambda entry: (entry.get("auto_priority", 999), entry["id"]),
)
AUTO_PROVIDERS = sorted(
    {entry["provider"] for entry in AUTO_ENTRIES},
    key=lambda provider: min(
        entry.get("auto_priority", 999)
        for entry in AUTO_ENTRIES
        if entry["provider"] == provider
    ),
)
AUTO_GROUPS = {provider: f"{AUTO_ALIAS}-{provider}" for provider in AUTO_PROVIDERS}
AUTO_MODELS = {
    provider: {entry["id"] for entry in AUTO_ENTRIES if entry["provider"] == provider}
    for provider in AUTO_PROVIDERS
}
VISION_MODELS = {
    entry["id"] for entry in CATALOG if entry.get("capabilities", {}).get("vision")
}


def _deployed_models():
    with open(os.environ.get("LITELLM_CONFIG", "/app/config.yaml")) as config_file:
        config = yaml.safe_load(config_file) or {}
    return {
        entry["model_name"]
        for entry in config.get("model_list", [])
        if not entry.get("model_name", "").startswith(AUTO_ALIAS)
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
        if any(not headers.get(name) for name in REQUIRED_HEADERS):
            raise HTTPException(
                status_code=403, detail="requests must come from the Argus API gateway"
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
            has_image = _has_image(data)
            provider = next(
                (
                    candidate
                    for candidate in AUTO_PROVIDERS
                    if allowed & AUTO_MODELS[candidate]
                    and (not has_image or allowed & AUTO_MODELS[candidate] & VISION_MODELS)
                ),
                None,
            )
            if provider is None:
                raise HTTPException(
                    status_code=403, detail="no allowed model is available for auto routing"
                )
            data["model"] = AUTO_GROUPS[provider]
            return data

        if requested not in allowed:
            raise HTTPException(
                status_code=403, detail=f"model `{requested}` is not enabled for your plan"
            )
        if requested not in DEPLOYED_MODELS:
            raise HTTPException(status_code=404, detail=f"model `{requested}` is not available")
        return data


gate = ArgusGate()
