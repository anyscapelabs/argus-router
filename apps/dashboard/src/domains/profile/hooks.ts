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
  providers: string[];
};

export function useProfile({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: ["profile"] as const,
    queryFn: () => apiFetch<Profile>(API_ROUTES.profile),
    enabled,
  });
}
