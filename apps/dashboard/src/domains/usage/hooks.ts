"use client";

import { useQuery } from "@tanstack/react-query";
import { getUsage, sinceForRange, type UsageRange, type UsageResult } from "./api";

export type UseUsageOptions = {
  keyId?: string;
  range?: UsageRange;
  limit?: number;
};

export function useUsage(options: UseUsageOptions | string = {}) {
  const normalized: UseUsageOptions =
    typeof options === "string" ? { keyId: options } : options;
  const { keyId, range, limit } = normalized;

  return useQuery<UsageResult, Error>({
    queryKey: ["usage", keyId ?? "all", range ?? "all", limit ?? "default"] as const,
    queryFn: () =>
      getUsage({
        keyId,
        since: range ? sinceForRange(range) : undefined,
        limit,
      }),
  });
}
