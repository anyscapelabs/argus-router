"use client";

import { useBilling } from "@/domains/billing/hooks";
import { usePlans } from "@/domains/plans/hooks";
import { useUsageSummary } from "@/domains/usage/hooks";

function formatResetDate(value?: string) {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "--";
  return new Intl.DateTimeFormat("en-GB", {
    day: "2-digit",
    month: "short",
    timeZone: "UTC",
  }).format(date);
}

function weeklyResetFromMonthly(value?: string) {
  if (!value) return undefined;
  const reset = new Date(value);
  if (Number.isNaN(reset.getTime())) return undefined;
  const periodStart = new Date(reset);
  periodStart.setMonth(periodStart.getMonth() - 1);
  const elapsed = Date.now() - periodStart.getTime();
  if (elapsed <= 0) return periodStart.toISOString();
  const weeks = Math.floor(elapsed / (7 * 24 * 60 * 60 * 1000)) + 1;
  return new Date(periodStart.getTime() + weeks * 7 * 24 * 60 * 60 * 1000).toISOString();
}

function usagePercent(used: number, limit: number | string | undefined) {
  const amount = Number(limit ?? 0);
  return amount > 0 ? Math.min(Math.round((used / amount) * 100), 100) : 0;
}

function UsageProgressCard({
  title,
  percent,
  resetAt,
  footer,
  ariaLabel,
}: {
  title: string;
  percent: number;
  resetAt?: string;
  footer?: string;
  ariaLabel: string;
}) {
  return (
    <div className="flex min-h-10 flex-col rounded-xl bg-secondary p-4">
      <p className="text-sm font-medium text-primary/80">{title}</p>
      <div className="mt-2 flex items-baseline justify-between gap-2">
        <div className="flex items-baseline gap-2">
          <p className="text-2xl font-medium text-primary">{percent}%</p>
          <span className="text-sm text-primary/60">Used</span>
        </div>
        <span className="text-sm text-primary/60">Resets {formatResetDate(resetAt)}</span>
      </div>
      <div
        role="progressbar"
        aria-label={ariaLabel}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        className="mt-3 h-1.5 w-full overflow-hidden rounded-full bg-primary/10"
      >
        <div
          className="h-full rounded-full bg-accent transition-[width] duration-500"
          style={{ width: `${percent}%` }}
        />
      </div>
      {footer && <p className="mt-auto pt-4 text-xs text-primary/60">{footer}</p>}
    </div>
  );
}

export function UsageCards() {
  const billing = useBilling();
  const plans = usePlans();
  const usage = useUsageSummary();
  const currentPlan = plans.data?.plans.find((plan) => plan.id === billing.data?.planId);

  const subscriptionResetAt = billing.data?.subscription?.nextBillingDate ?? undefined;
  const weeklyUsed = usage.data?.periods?.weekly.tokens ?? 0;
  const monthlyUsed = usage.data?.periods?.monthly.tokens ?? 0;
  const weeklyResetAt =
    usage.data?.periods?.weekly.resetAt ??
    usage.data?.weeklyResetAt ??
    weeklyResetFromMonthly(subscriptionResetAt);
  const monthlyResetAt =
    usage.data?.periods?.monthly.resetAt ??
    usage.data?.monthlyResetAt ??
    subscriptionResetAt;

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
      <UsageProgressCard
        title="Weekly Included Usage"
        percent={usagePercent(weeklyUsed, currentPlan?.weeklyTokens)}
        resetAt={weeklyResetAt}
        ariaLabel="Weekly usage"
      />
      <UsageProgressCard
        title="Monthly Usage"
        percent={usagePercent(monthlyUsed, currentPlan?.monthlyTokens)}
        resetAt={monthlyResetAt}
        footer="Monthly Included Usage"
        ariaLabel="Monthly usage"
      />
    </div>
  );
}

export function UsageExtraCards() {
  const usage = useUsageSummary();
  const usageBilling = useBilling();
  const monthlyResetAt =
    usage.data?.periods?.monthly.resetAt ??
    usage.data?.monthlyResetAt ??
    usageBilling.data?.subscription?.nextBillingDate;

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
      <UsageProgressCard
        title="Voice Usage"
        percent={usagePercent(
          usage.data?.voiceMinutesUsed ?? 0,
          usage.data?.voiceMinutesQuota,
        )}
        resetAt={monthlyResetAt}
        footer="Included Voice Minutes"
        ariaLabel="Voice usage"
      />
      <UsageProgressCard
        title="Web Search Usage"
        percent={usagePercent(usage.data?.webSearchUsed ?? 0, usage.data?.webSearchQuota)}
        resetAt={monthlyResetAt}
        footer="Included Web Searches"
        ariaLabel="Web search usage"
      />
    </div>
  );
}
