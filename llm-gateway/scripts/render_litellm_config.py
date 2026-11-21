#!/usr/bin/env python3
"""Render the LiteLLM proxy config from the Argus model catalog."""

import json
import os
import sys

AUTO_ALIAS = "argus/auto"
GROUP_ORDER = {"lite": 0, "standard": 1, "premium": 2, "ultra": 3}


def group_rank(entry):
    return GROUP_ORDER.get(entry.get("group"), 99)


def is_deployable(entry):
    return entry.get("status") != "blocked"


def provider_params(entry):
    provider = entry["provider"]
    model = entry["upstream_model"]
    if provider == "openrouter":
        return {
            "model": f"openrouter/{model}",
            "api_key": "os.environ/OPENROUTER_API_KEY",
            "api_base": "https://openrouter.ai/api/v1",
            "extra_headers": {
                "HTTP-Referer": "https://anyscapelabs.com",
                "X-Title": "Argus Agent",
            },
        }
    if provider == "bedrock":
        return {
            "model": f"bedrock/{model}" if not model.startswith("bedrock/") else model,
            "aws_region_name": os.environ.get("AWS_REGION_NAME", "eu-north-1"),
        }
    raise ValueError(f"unsupported provider `{provider}` for {entry['id']}")


def yaml_value(value):
    if isinstance(value, dict):
        items = ", ".join(f"{key}: {json.dumps(item)}" for key, item in value.items())
        return "{" + items + "}"
    return value


def render_entry(entry, group=None, suffix=""):
    lines = [f"  - model_name: {group or entry['id']}", "    litellm_params:"]
    for key, value in provider_params(entry).items():
        lines.append(f"      {key}: {yaml_value(value)}")
    lines.append("    model_info:")
    lines.append("      disable_background_health_check: true")
    if group:
        lines.append(f"      id: {entry['id'].replace('/', '-')}-auto{suffix}")
    lines.append(f"      max_tokens: {entry['max_output']}")
    return lines


def main():
    catalog_path = sys.argv[1] if len(sys.argv) > 1 else "../apps/api/catalog/models.json"
    output_path = sys.argv[2] if len(sys.argv) > 2 else "config/litellm_config.yaml"

    with open(catalog_path) as catalog_file:
        catalog = json.load(catalog_file)

    active = [entry for entry in catalog if is_deployable(entry) and entry["id"] != AUTO_ALIAS]
    auto_entries = sorted(active, key=lambda entry: (group_rank(entry), entry["id"]))
    providers = sorted(
        {entry["provider"] for entry in auto_entries},
        key=lambda provider: min(
            group_rank(entry) for entry in auto_entries if entry["provider"] == provider
        ),
    )
    tiers = [tier for tier in GROUP_ORDER if any(entry.get("group") == tier for entry in auto_entries)]

    lines = [
        "# GENERATED from apps/api/catalog/models.json — do not edit.",
        "# Run scripts/render_litellm_config.py after changing the catalog.",
        "model_list:",
    ]
    for entry in active:
        lines.extend(render_entry(entry))

    for tier in tiers:
        for provider in providers:
            group = f"{AUTO_ALIAS}-{tier}-{provider}"
            tier_provider_entries = [
                entry
                for entry in auto_entries
                if entry.get("group") == tier and entry["provider"] == provider
            ]
            if not tier_provider_entries:
                continue
            for entry in tier_provider_entries:
                lines.extend(render_entry(entry, group, suffix=f"-{tier}"))

    lines.extend(
        [
            "",
            "router_settings:",
            "  routing_strategy: simple-shuffle",
            "  num_retries: 3",
            "  retry_after: 2",
            "  allowed_fails: 2",
            "  cooldown_time: 60",
            "  # Fall back across providers, then down a tier.",
            "  fallbacks:",
        ]
    )
    for tier_index, tier in enumerate(tiers):
        tier_providers = [
            provider
            for provider in providers
            if any(entry.get("group") == tier and entry["provider"] == provider for entry in auto_entries)
        ]
        for index, provider in enumerate(tier_providers):
            lines.append(f"    - {AUTO_ALIAS}-{tier}-{provider}:")
            if len(tier_providers) > 1:
                next_provider = tier_providers[(index + 1) % len(tier_providers)]
                lines.append(f"        - {AUTO_ALIAS}-{tier}-{next_provider}")
            if tier_index > 0:
                lower_tier = tiers[tier_index - 1]
                lines.append(f"        - {AUTO_ALIAS}-{lower_tier}-{provider}")
    lines.append("")

    lines.extend(
        [
            "litellm_settings:",
            "  drop_params: true",
            "  request_timeout: 120",
            "  streaming_timeout: 120",
            "  set_verbose: false",
            "  json_logs: true",
            "  include_cost_in_streaming_usage: false",
            "  callbacks: argus_gate.gate",
            "",
            "general_settings:",
            "  master_key: os.environ/LITELLM_INTERNAL_KEY",
            "  background_health_checks: false",
            "  always_include_stream_usage: true",
            "  disable_spend_logs: true",
            "  disable_error_logs: false",
            "  store_model_in_db: false",
            "  allow_requests_on_db_fail: true",
            "",
        ]
    )

    with open(output_path, "w") as output_file:
        output_file.write("\n".join(lines))


if __name__ == "__main__":
    main()
