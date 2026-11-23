"use client";

import { useUsageSummary } from "@/domains/usage/hooks";

const COMPACT = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 1,
});

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-2 rounded-xl bg-secondary p-4">
      <p className="text-xs text-primary/60">{label}</p>
      <p className="text-2xl font-medium text-primary">{value}</p>
    </div>
  );
}

export function UsageMetricCards() {
  const usage = useUsageSummary();
  const totals = usage.data?.totals;
  const allTimeTokens =
    (totals?.inputTokens ?? 0) + (totals?.outputTokens ?? 0) + (totals?.cacheReadTokens ?? 0);

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
      <MetricCard label="All Time Tokens" value={COMPACT.format(allTimeTokens)} />
      <MetricCard
        label="Monthly Tokens"
        value={COMPACT.format(usage.data?.periods?.monthly.tokens ?? 0)}
      />
      <MetricCard label="Total Requests" value={COMPACT.format(totals?.requests ?? 0)} />
    </div>
  );
}
