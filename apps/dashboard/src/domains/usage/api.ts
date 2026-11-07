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

export async function getUsage(keyId?: string): Promise<UsageResult> {
  const url = keyId ? `${API_ROUTES.usage}?keyId=${encodeURIComponent(keyId)}` : API_ROUTES.usage;
  const response = await fetch(url, { method: "GET", credentials: "include" });
  if (!response.ok) throw new Error("Failed to load usage");
  return response.json() as Promise<UsageResult>;
}