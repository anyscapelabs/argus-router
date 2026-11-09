import { API_ROUTES } from "@/config/api";

export type UsageRow = {
  id: number;
  createdAt: string | null;
  keyId: string | null;
  userId: string | null;
  planId: string | null;
  model: string | null;
  provider: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  costUsd: number | null;
  status: number | null;
  latencyMs: number | null;
  errorCode: string | null;
};

export type UsageResult = {
  rows: UsageRow[];
};

export type UsageRange = "24h" | "7d" | "30d";

export type GetUsageParams = {
  keyId?: string;
  since?: string;
  limit?: number;
};

export type UsageAggregate = {
  requests: number;
  errors: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  costUsd: number;
};

export type UsageByModel = UsageAggregate & { model: string };
export type UsageByKey = UsageAggregate & { keyId: string | null };

/** One chart bucket; `bucket` is a viewer-local `YYYY-MM-DDTHH:MM` string. */
export type UsageSeriesPoint = {
  bucket: string;
  requests: number;
  tokens: number;
  costUsd: number;
};

export type UsageSummary = {
  totals: UsageAggregate;
  byModel: UsageByModel[];
  byKey: UsageByKey[];
  series: UsageSeriesPoint[];
  bucket: "hour" | "day";
  tz: string;
};

/**
 * Start of the window for a range, snapped to the chart's bucket boundary
 * (hour for 24h, local midnight for 7d/30d) so the totals cards equal the
 * sum of the bars instead of including a partial bucket the chart omits.
 */
export function sinceForRange(range: UsageRange, now = new Date()): string {
  const d = new Date(now);
  if (range === "24h") {
    d.setMinutes(0, 0, 0);
    d.setHours(d.getHours() - 23);
  } else {
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - (range === "7d" ? 6 : 29));
  }
  return d.toISOString();
}

export async function getUsage(params: GetUsageParams = {}): Promise<UsageResult> {
  const search = new URLSearchParams();
  if (params.keyId) search.set("key_id", params.keyId);
  if (params.since) search.set("since", params.since);
  if (params.limit != null) search.set("limit", String(params.limit));
  const qs = search.toString();
  const url = qs ? `${API_ROUTES.usage}?${qs}` : API_ROUTES.usage;
  const response = await fetch(url, { method: "GET", credentials: "include" });
  if (!response.ok) throw new Error("Failed to load usage");
  return response.json() as Promise<UsageResult>;
}

export type GetUsageSummaryParams = {
  keyId?: string;
  since?: string;
  bucket?: "hour" | "day";
};

/**
 * Server-side aggregates over every row in range. Totals shown in the UI must
 * come from here: the row list is capped, so summing it undercounts.
 */
export async function getUsageSummary(params: GetUsageSummaryParams = {}): Promise<UsageSummary> {
  const search = new URLSearchParams();
  if (params.keyId) search.set("key_id", params.keyId);
  if (params.since) search.set("since", params.since);
  if (params.bucket) search.set("bucket", params.bucket);
  try {
    search.set("tz", Intl.DateTimeFormat().resolvedOptions().timeZone);
  } catch {
    // no Intl zone available: the server falls back to UTC
  }
  const response = await fetch(`${API_ROUTES.usage}/summary?${search.toString()}`, {
    method: "GET",
    credentials: "include",
  });
  if (!response.ok) throw new Error("Failed to load usage summary");
  return response.json() as Promise<UsageSummary>;
}
