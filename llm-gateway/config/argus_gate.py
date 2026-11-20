import os
import random

import yaml
from fastapi import HTTPException
from litellm.integrations.custom_logger import CustomLogger

AUTO_ALIAS = "argus/auto"
REQUIRED_HEADERS = (
    "x-argus-allowed-models",
    "x-argus-user-id",
    "x-argus-plan",
)


def _deployed_models():
    with open(os.environ.get("LITELLM_CONFIG", "/app/config.yaml")) as config_file:
        config = yaml.safe_load(config_file) or {}
    return {
        entry["litellm_params"]["model"].removeprefix("openrouter/")
        for entry in config.get("model_list", [])
        if entry.get("model_name") != AUTO_ALIAS
    }


DEPLOYED_MODELS = _deployed_models()


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
            candidates = sorted(allowed & DEPLOYED_MODELS)
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
