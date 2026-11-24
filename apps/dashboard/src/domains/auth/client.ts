import { createAuthClient } from "better-auth/client";
import { magicLinkClient } from "better-auth/client/plugins";

import { API_ROUTES } from "@/config/api";

export const authClient = createAuthClient({
  baseURL: API_ROUTES.auth,
  plugins: [magicLinkClient()],
  fetchOptions: {
    credentials: "include",
  },
});

export function dashboardPath(path: string) {
  return `${window.location.origin}${path}`;
}

export function sanitizeNextPath(value: string | null | undefined, fallback = "/console/overview") {
  if (!value) return fallback;
  const trimmed = value.trim();
  if (!trimmed.startsWith("/") || trimmed.startsWith("//")) return fallback;
  if (trimmed.includes("\\")) return fallback;
  try {
    const url = new URL(trimmed, "https://dashboard.local");
    if (url.origin !== "https://dashboard.local") return fallback;
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return fallback;
  }
}

export function withNext(path: string, next: string | null | undefined) {
  const safeNext = sanitizeNextPath(next, "");
  if (!safeNext) return path;
  const separator = path.includes("?") ? "&" : "?";
  return `${path}${separator}next=${encodeURIComponent(safeNext)}`;
}
