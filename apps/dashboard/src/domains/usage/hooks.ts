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
  logType?: "chat" | "voice" | "web_search";
  range?: UsageRange;
  since?: string;
  until?: string;
  limit?: number;
  enabled?: boolean;
};

export function useUsage(options: UseUsageOptions | string = {}) {
  const normalized: UseUsageOptions =
    typeof options === "string" ? { keyId: options } : options;
  const { keyId, logType, range, since, until, limit, enabled = true } = normalized;

  return useQuery<UsageResult, Error>({
    queryKey: [
      "usage",
      keyId ?? "all",
      logType ?? "all",
      range ?? "all",
      since ?? "all",
      until ?? "all",
      limit ?? "default",
    ] as const,
    queryFn: () =>
      getUsage({
        keyId,
        logType,
        since: range ? sinceForRange(range) : since,
        until,
        limit,
      }),
    enabled,
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
