import { API_ROUTES, type ApiRoute } from "@/config/api";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

const JSON_HEADERS = { "Content-Type": "application/json" } as const;

/**
 * Thin wrapper around fetch for the versioned API (`/api/v1`). Auth-domain
 * traffic goes through the better-auth client instead; this wrapper is for
 * plain domain endpoints (agents, profile, ...).
 */
export async function apiFetch<T>(
  route: ApiRoute,
  init: Omit<RequestInit, "body"> & { body?: unknown } = {},
): Promise<T> {
  const { body, headers, ...rest } = init;
  const response = await fetch(route, {
    credentials: "include",
    headers: { ...JSON_HEADERS, ...headers },
    body: body === undefined ? undefined : JSON.stringify(body),
    ...rest,
  });

  if (!response.ok) {
    let payload: {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string };
    } = {};
    try {
      payload = await response.json();
    } catch {
      // non-JSON error bodies are folded into the generic message below
    }
    const code = payload.error?.code ?? payload.code ?? `HTTP_${response.status}`;
    const message = payload.error?.message ?? payload.message ?? response.statusText;
    throw new ApiError(response.status, code, message);
  }

  return (await response.json()) as T;
}
