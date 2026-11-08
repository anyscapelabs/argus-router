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

export function sinceForRange(range: UsageRange, now = new Date()): string {
  const ms =
    range === "24h" ? 24 * 60 * 60 * 1000 : range === "7d" ? 7 * 24 * 60 * 60 * 1000 : 30 * 24 * 60 * 60 * 1000;
  return new Date(now.getTime() - ms).toISOString();
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
