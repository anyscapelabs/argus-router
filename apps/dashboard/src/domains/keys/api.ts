/**
 * The api-key endpoints live inside the better-auth tree. Its TS client
 * (1.7.x) ships no `apiKeyClient` plugin while the Rust alpha.5 ApiKeyPlugin
 * serves the canonical wire routes — so the dashboard calls them directly.
 * The Rust handler strips the configured base path (`/api/v1/auth`) from the
 * request URI to resolve the endpoint, so no path header is involved.
 *
 * Wire shapes verified against the running API: create answers the key
 * record flat with the secret `key` inline; list answers
 * `{apiKeys, total}`; delete 404s unknown ids.
 */
import { API_ROUTES } from "@/config/api";
import { ApiError } from "@/api/http-client";

export type ApiKeyRecord = {
  id: string;
  name: string | null;
  start: string | null;
  prefix: string | null;
  enabled: boolean;
  createdAt: string;
  expiresAt: string | null;
  remaining: number | null;
};

export type ListKeysResult = {
  apiKeys: ApiKeyRecord[];
  total: number;
};

/** Create answers the key record flat with the secret `key` inline — shown once. */
export type CreateKeyResult = ApiKeyRecord & {
  key: string;
};

const AUTH = API_ROUTES.auth as unknown as string;

export async function createKey(name: string): Promise<CreateKeyResult> {
  const response = await fetch(`${AUTH}/api-key/create`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  return unwrap(response);
}

export async function listKeys(): Promise<ListKeysResult> {
  const response = await fetch(`${AUTH}/api-key/list`, {
    method: "GET",
    credentials: "include",
  });
  return unwrap(response);
}

/** `POST /api-key/delete` answers 200 with a body, 404 unknown ids. */
export async function revokeKey(keyId: string): Promise<void> {
  const response = await fetch(`${AUTH}/api-key/delete`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ keyId }),
  });
  if (response.ok) return;
  await unwrap(response);
}

async function unwrap<T>(response: Response): Promise<T> {
  const text = await response.text();
  if (!response.ok) {
    let payload: { code?: string; message?: string; error?: { code?: string; message?: string } } | undefined;
    try {
      payload = text ? JSON.parse(text) : undefined;
    } catch {
      // non-JSON error bodies fall back to the status text
    }
    const code = payload?.error?.code ?? payload?.code ?? `HTTP_${response.status}`;
    const message = payload?.error?.message ?? payload?.message ?? response.statusText;
    throw new ApiError(response.status, code, message);
  }
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}
