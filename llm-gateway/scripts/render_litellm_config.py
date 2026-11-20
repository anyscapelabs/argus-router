#!/usr/bin/env python3
"""Render the LiteLLM proxy config from the Argus model catalog."""

import json
import os
import sys

AUTO_ALIAS = "argus/auto"


def provider_params(entry):
    provider = entry["provider"]
    model = entry["upstream_model"]
    if provider == "openrouter":
        return {
            "model": f"openrouter/{model}",
            "api_key": "os.environ/OPENROUTER_API_KEY",
            "api_base": "https://openrouter.ai/api/v1",
        }
    if provider == "bedrock":
        return {
            "model": f"bedrock/{model}" if not model.startswith("bedrock/") else model,
            "aws_region_name": os.environ.get("AWS_REGION_NAME", "eu-north-1"),
        }
    raise ValueError(f"unsupported provider `{provider}` for {entry['id']}")


def render_entry(entry, group=None):
    lines = [f"  - model_name: {group or entry['id']}", "    litellm_params:"]
    for key, value in provider_params(entry).items():
        lines.append(f"      {key}: {value}")
    lines.append("    model_info:")
    lines.append("      disable_background_health_check: true")
    if group:
        lines.append(f"      id: {entry['id'].replace('/', '-')}-auto")
    lines.append(f"      max_tokens: {entry['max_output']}")
    return lines


def main():
    catalog_path = sys.argv[1] if len(sys.argv) > 1 else "../apps/api/catalog/models.json"
    output_path = sys.argv[2] if len(sys.argv) > 2 else "config/litellm_config.yaml"

    with open(catalog_path) as catalog_file:
        catalog = json.load(catalog_file)

    auto_entries = sorted(
        (entry for entry in catalog if entry.get("auto") and entry["id"] != AUTO_ALIAS),
        key=lambda entry: (entry.get("auto_priority", 999), entry["id"]),
    )
    providers = sorted(
        {entry["provider"] for entry in auto_entries},
        key=lambda provider: min(
            entry.get("auto_priority", 999)
            for entry in auto_entries
            if entry["provider"] == provider
        ),
    )

    lines = [
        "# GENERATED from apps/api/catalog/models.json — do not edit.",
        "# Run scripts/render_litellm_config.py after changing the catalog.",
        "model_list:",
    ]
    for entry in catalog:
        if entry["id"] != AUTO_ALIAS:
            lines.extend(render_entry(entry))

    for provider in providers:
        group = f"{AUTO_ALIAS}-{provider}"
        for entry in auto_entries:
            if entry["provider"] == provider:
                lines.extend(render_entry(entry, group))

    lines.extend(
        [
            "",
            "router_settings:",
            "  routing_strategy: simple-shuffle",
            "  num_retries: 3",
            "  retry_after: 2",
            "  allowed_fails: 2",
            "  cooldown_time: 60",
            "  # Provider-level fallback: if one provider fails, use the next one.",
            "  fallbacks:",
        ]
    )
    for index, provider in enumerate(providers):
        next_provider = providers[(index + 1) % len(providers)]
        lines.append(f"    - {AUTO_ALIAS}-{provider}:")
        lines.append(f"        - {AUTO_ALIAS}-{next_provider}")

    lines.extend(
        [
            "",
            "litellm_settings:",
            "  drop_params: true",
            "  request_timeout: 120",
            "  streaming_timeout: 120",
            "  set_verbose: false",
            "  json_logs: true",
            "  include_cost_in_streaming_usage: true",
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
