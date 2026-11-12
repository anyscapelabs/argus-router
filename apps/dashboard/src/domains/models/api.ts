import { API_ROUTES } from "@/config/api";
import { ApiError } from "@/api/http-client";

export type ModelCapabilities = {
  tools: boolean;
  vision: boolean;
  reasoning: boolean;
};

export type ModelPricing = {
  input: string;
  output: string;
  cacheRead: string | null;
  cacheWrite: string | null;
};

export type ModelView = {
  id: string;
  name: string;
  provider: string;
  compat: string;
  context: number;
  maxOutput: number;
  capabilities: ModelCapabilities;
  pricing: ModelPricing;
  free: boolean;
  enabled: boolean;
};

export type ModelsResult = {
  plan: string;
  models: ModelView[];
};

export type ToggleResult = {
  modelId: string;
  enabled: boolean;
  allowlist: string[];
};

export type RefreshResult = {
  refreshed: boolean;
  count: number;
};

const MODELS = API_ROUTES.models as unknown as string;

export async function getModels(): Promise<ModelsResult> {
  const response = await fetch(MODELS, {
    method: "GET",
    credentials: "include",
  });
  return unwrap(response);
}

export async function toggleModelAllowlist(
  modelId: string,
  enabled: boolean,
): Promise<ToggleResult> {
  const response = await fetch(`${MODELS}/allowlist`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ modelId, enabled }),
  });
  return unwrap(response);
}

export async function refreshModels(): Promise<RefreshResult> {
  const response = await fetch(`${MODELS}/refresh`, {
    method: "POST",
    credentials: "include",
  });
  return unwrap(response);
}

async function unwrap<T>(response: Response): Promise<T> {
  const text = await response.text();
  if (!response.ok) {
    let payload:
      | {
          code?: string;
          message?: string;
          error?: { code?: string; message?: string };
        }
      | undefined;
    try {
      payload = text ? JSON.parse(text) : undefined;
    } catch {
    }
    const code = payload?.error?.code ?? payload?.code ?? `HTTP_${response.status}`;
    const message = payload?.error?.message ?? payload?.message ?? response.statusText;
    throw new ApiError(response.status, code, message);
  }
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}
