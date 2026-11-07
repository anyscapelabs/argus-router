"use client";

import { useQuery } from "@tanstack/react-query";
import { getUsage, type UsageResult } from "./api";

export function useUsage(keyId?: string) {
  return useQuery<UsageResult, Error>({
    queryKey: ["usage", keyId ?? "all"] as const,
    queryFn: () => getUsage(keyId),
  });
}