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
  status: number | null;
  latencyMs: number | null;
  errorCode: string | null;
  logType?: "chat" | "voice" | "web_search";
  details?: {
    turns?: number;
    query?: string;
    resultCount?: number;
  } | null;
};

export type UsageResult = {
  rows: UsageRow[];
};

export type UsageRange = "24h" | "7d" | "30d" | "month";

export type GetUsageParams = {
  keyId?: string;
  logType?: "chat" | "voice" | "web_search";
  since?: string;
  until?: string;
  limit?: number;
};

export type UsageAggregate = {
  requests: number;
  errors: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
};

export type UsageByModel = UsageAggregate & { model: string };
export type UsageByKey = UsageAggregate & { keyId: string | null };

export type UsageSeriesPoint = {
  bucket: string;
  requests: number;
  tokens: number;
};

export type UsageByModelSeriesPoint = {
  bucket: string;
  model: string;
  requests: number;
  tokens: number;
};

export type UsagePeriodSummary = {
  startAt: string;
  resetAt: string;
  tokens: number;
};

export type UsageSummary = {
  weeklyResetAt?: string;
  monthlyResetAt?: string;
  periods?: {
    weekly: UsagePeriodSummary;
    monthly: UsagePeriodSummary;
  };
  totals: UsageAggregate;
  byModel: UsageByModel[];
  byKey: UsageByKey[];
  series: UsageSeriesPoint[];
  byModelSeries: UsageByModelSeriesPoint[];
  bucket: "hour" | "day";
  tz: string;
  voiceMinutesUsed?: number;
  voiceMinutesQuota?: number;
  webSearchUsed?: number;
  webSearchQuota?: number;
};

export function sinceForRange(range: UsageRange, now = new Date()): string {
  const d = new Date(now);
  if (range === "24h") {
    d.setMinutes(0, 0, 0);
    d.setHours(d.getHours() - 23);
  } else if (range === "month") {
    d.setDate(1);
    d.setHours(0, 0, 0, 0);
  } else {
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - (range === "7d" ? 6 : 29));
  }
  return d.toISOString();
}

export async function getUsage(params: GetUsageParams = {}): Promise<UsageResult> {
  const search = new URLSearchParams();
  if (params.keyId) search.set("key_id", params.keyId);
  if (params.logType) search.set("type", params.logType);
  if (params.since) search.set("since", params.since);
  if (params.until) search.set("until", params.until);
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

export async function getUsageSummary(params: GetUsageSummaryParams = {}): Promise<UsageSummary> {
  const search = new URLSearchParams();
  if (params.keyId) search.set("key_id", params.keyId);
  if (params.since) search.set("since", params.since);
  if (params.bucket) search.set("bucket", params.bucket);
  try {
    search.set("tz", Intl.DateTimeFormat().resolvedOptions().timeZone);
  } catch {
  }
  const response = await fetch(`${API_ROUTES.usage}/summary?${search.toString()}`, {
    method: "GET",
    credentials: "include",
  });
  if (!response.ok) throw new Error("Failed to load usage summary");
  return response.json() as Promise<UsageSummary>;
}
