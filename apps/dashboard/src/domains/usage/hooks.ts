"use client";

import { useQuery } from "@tanstack/react-query";
import {
  getUsage,
  getUsageSummary,
  sinceForRange,
  type UsageRange,
  type UsageResult,
  type UsageSummary,
} from "./api";

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

export function useUsageSummary(options: UseUsageOptions = {}) {
  const { keyId, range } = options;
  return useQuery<UsageSummary, Error>({
    queryKey: ["usage", "summary", keyId ?? "all", range ?? "all"] as const,
    queryFn: () =>
      getUsageSummary({
        keyId,
        since: range ? sinceForRange(range) : undefined,
        bucket: range === "24h" ? "hour" : "day",
      }),
  });
}
