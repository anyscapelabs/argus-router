import json
import os

import yaml
from fastapi import HTTPException
from litellm.integrations.custom_logger import CustomLogger

AUTO_ALIAS = "argus/auto"
CATALOG_PATH = os.environ.get("ARGUS_CATALOG_PATH", "/app/catalog/models.json")
GROUP_ORDER = {"lite": 0, "standard": 1, "premium": 2, "ultra": 3}
REQUIRED_HEADERS = (
    "x-argus-allowed-models",
    "x-argus-user-id",
    "x-argus-plan",
)
EFFORT_TIERS = {
    "lite": "lite",
    "low": "lite",
    "standard": "standard",
    "medium": "standard",
    "high": "premium",
    "premium": "premium",
    "ultra": "ultra",
    "max": "ultra",
}

with open(CATALOG_PATH) as catalog_file:
    CATALOG = json.load(catalog_file)

BY_ID = {entry["id"]: entry for entry in CATALOG}
BLOCKED_MODELS = {
    entry["id"] for entry in CATALOG if entry.get("status") == "blocked"
}
ACTIVE_ENTRIES = [
    entry for entry in CATALOG
    if entry["id"] != AUTO_ALIAS and entry["id"] not in BLOCKED_MODELS
]
AUTO_ENTRIES = sorted(
    ACTIVE_ENTRIES,
    key=lambda entry: (GROUP_ORDER.get(entry.get("group"), 99), entry["id"]),
)
AUTO_PROVIDERS = sorted(
    {entry["provider"] for entry in AUTO_ENTRIES},
    key=lambda provider: min(
        GROUP_ORDER.get(entry.get("group"), 99)
        for entry in AUTO_ENTRIES
        if entry["provider"] == provider
    ),
)
AUTO_MODELS = {
    (tier, provider): {
        entry["id"] for entry in AUTO_ENTRIES
        if entry.get("group") == tier and entry["provider"] == provider
    }
    for tier in GROUP_ORDER
    for provider in AUTO_PROVIDERS
}
AUTO_GROUPS = {
    (tier, provider): f"{AUTO_ALIAS}-{tier}-{provider}"
    for tier in GROUP_ORDER
    for provider in AUTO_PROVIDERS
}
VISION_MODELS = {
    entry["id"] for entry in ACTIVE_ENTRIES
    if entry.get("capabilities", {}).get("vision")
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


def _prompt_size(data):
    total = 0
    for message in data.get("messages") or []:
        content = message.get("content")
        if isinstance(content, str):
            total += len(content)
        elif isinstance(content, list):
            total += len(json.dumps(content))
    return total


def _requested_tier(data, headers):
    effort = (headers.get("x-argus-effort") or "").strip().lower()
    if effort in EFFORT_TIERS:
        return EFFORT_TIERS[effort]
    size = _prompt_size(data)
    if size >= 30_000:
        return "ultra"
    if size >= 12_000:
        return "premium"
    if size >= 4_000:
        return "standard"
    return "lite"


def _select_auto_group(data, headers, allowed):
    has_image = _has_image(data)
    requested = _requested_tier(data, headers)
    start = GROUP_ORDER.get(requested, 0)
    for rank in range(start, -1, -1):
        tier = next(name for name, value in GROUP_ORDER.items() if value == rank)
        for provider in AUTO_PROVIDERS:
            candidates = allowed & AUTO_MODELS.get((tier, provider), set())
            if not candidates:
                continue
            group = AUTO_GROUPS[(tier, provider)]
            if group not in DEPLOYED_MODELS:
                continue
            if not has_image or candidates & VISION_MODELS:
                return group
    return None


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
            group = _select_auto_group(data, headers, allowed)
            if group is None:
                raise HTTPException(
                    status_code=403, detail="no allowed model is available for auto routing"
                )
            data["model"] = group
            return data

        if requested in BLOCKED_MODELS:
            raise HTTPException(status_code=403, detail=f"model `{requested}` is blocked")
        if requested not in allowed:
            raise HTTPException(
                status_code=403, detail=f"model `{requested}` is not enabled for your plan"
            )
        if requested not in DEPLOYED_MODELS:
            raise HTTPException(status_code=404, detail=f"model `{requested}` is not available")
        return data


gate = ArgusGate()
