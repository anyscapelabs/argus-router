"use client";

import { useQuery } from "@tanstack/react-query";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

export type Profile = {
  id: string;
  email: string | null;
  name: string | null;
  image: string | null;
  emailVerified: boolean;
};

/**
 * `GET /api/v1/profile` — session-protected; a 401 rejects as `ApiError`
 * status 401 (no retry), which callers can watch for redirect decisions.
 * Pass `{ enabled: false }` to hold the request until a session is known.
 */
export function useProfile({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: ["profile"] as const,
    queryFn: () => apiFetch<Profile>(API_ROUTES.profile),
    enabled,
  });
}
