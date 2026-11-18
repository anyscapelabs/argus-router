"use client";

import { useEffect, useMemo, useState } from "react";
import dynamic from "next/dynamic";
import { RangeControls } from "@/components/usage-range-controls";
import { chartAxisLabels } from "@/components/usage-empty-state";
import { EmptyState } from "@/components/empty-state";
import { SkeletonRows } from "@/components/skeleton-rows";
import { formatTokenCount } from "@/lib/usage-format";
import { useUsage, useUsageSummary } from "@/domains/usage/hooks";
import {
  useAutoRecharge,
  useBilling,
  usePaymentMethodSetup,
  usePaymentMethods,
  useUpdateAutoRecharge,
} from "@/domains/billing/hooks";
import type { UsageRange } from "@/domains/usage/api";

const UsageChart = dynamic(
  () => import("@/components/usage-chart").then((m) => m.UsageChart),
  { ssr: false },
);

export default function UsagePage() {
  const [usageMetric, setUsageMetric] = useState<"cost" | "tokens">("cost");
  const [logRange, setLogRange] = useState<UsageRange>("7d");

  const usage = useUsage({ range: logRange });
  const summary = useUsageSummary({ range: logRange });
  const billing = useBilling();
  const billingConfigured = billing.data?.billingConfigured ?? false;
  const paymentMethods = usePaymentMethods(billingConfigured);
  const autoRecharge = useAutoRecharge(billingConfigured);
  const paymentMethodSetup = usePaymentMethodSetup();
  const updateAutoRecharge = useUpdateAutoRecharge();
  const savedCard = paymentMethods.data?.paymentMethods[0];
  const rechargeSettings = autoRecharge.data;
  const [rechargeOpen, setRechargeOpen] = useState(false);
  const [rechargeAmount, setRechargeAmount] = useState("");
  const [rechargeThreshold, setRechargeThreshold] = useState("");
  const rechargeAmountValue = Number(rechargeAmount);
  const rechargeThresholdValue = Number(rechargeThreshold);
  const rechargeAmountValid =
    Number.isFinite(rechargeAmountValue) &&
    rechargeAmountValue >= 3 &&
    rechargeAmountValue <= 1000;
  const rechargeThresholdValid =
    Number.isFinite(rechargeThresholdValue) &&
    rechargeThresholdValue >= 0.5 &&
    rechargeThresholdValue <= 25;
  const rechargeSubmitDisabled =
    updateAutoRecharge.isPending ||
    !rechargeAmountValid ||
    !rechargeThresholdValid;
  const openRechargeDialog = () => {
    setRechargeAmount(
      rechargeSettings?.amountCents
        ? (rechargeSettings.amountCents / 100).toString()
        : "10",
    );
    setRechargeThreshold(
      rechargeSettings?.thresholdMicros
        ? (rechargeSettings.thresholdMicros / 1_000_000).toString()
        : "5",
    );
    setRechargeOpen(true);
  };
  const submitRecharge = () => {
    updateAutoRecharge.mutate(
      {
        enabled: true,
        amountUsd: Math.round(rechargeAmountValue * 100) / 100,
        thresholdUsd: Math.round(rechargeThresholdValue * 100) / 100,
      },
      { onSuccess: () => setRechargeOpen(false) },
    );
  };
  const turnOffRecharge = () => {
    updateAutoRecharge.mutate({ enabled: false });
  };
  const usageError = summary.isError || usage.isError;
  const usageErrorMessage =
    (summary.isError ? summary.error?.message : undefined) ??
    (usage.isError ? usage.error?.message : undefined) ??
    "Usage could not be loaded";
  const totals = summary.data?.totals;
  const series = summary.data?.series ?? [];
  const modelUsage = useMemo(
    () =>
      (summary.data?.byModel ?? []).map((m) => ({
        model: m.model || "unknown",
        requests: m.requests,
        input: m.inputTokens,
        output: m.outputTokens,
        cacheRead: m.cacheReadTokens,
        cost: m.costUsd,
      })),
    [summary.data],
  );
  const [axisLabels, setAxisLabels] = useState<string[]>([]);
  useEffect(() => {
    const frame = requestAnimationFrame(() => setAxisLabels(chartAxisLabels(logRange)));
    return () => cancelAnimationFrame(frame);
  }, [logRange]);

  const chartGranularity = logRange === "24h" ? "hourly" : "daily";

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h2 className="mt-4 text-sm font-medium text-primary">My usage</h2>
          <p className="mt-1 text-xs text-white/60">
            Token usage and cost across your service accounts.
          </p>
        </div>
        <RangeControls
          range={logRange}
          onRangeChange={setLogRange}
          onRefresh={() => {
            void usage.refetch();
            void summary.refetch();
          }}
          isFetching={usage.isFetching || summary.isFetching}
        />
      </div>

      {usageError && (
        <div
          role="alert"
          className="rounded-md border border-red-400/30 bg-red-400/10 px-3 py-2 text-xs text-red-300"
        >
          Couldn&apos;t load usage: {usageErrorMessage}.{" "}
          <button
            type="button"
            onClick={() => {
              void usage.refetch();
              void summary.refetch();
            }}
            className="cursor-pointer underline underline-offset-2 hover:text-red-200"
          >
            Retry
          </button>
        </div>
      )}

      <div className="flex min-w-0 flex-col overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-3">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-xs font-medium text-white/60">Credit balance</p>
            <p className="mt-1 text-2xl font-medium text-primary">
              {billing.isLoading
                ? "—"
                : `$${((billing.data?.creditBalanceMicros ?? 0) / 1_000_000).toFixed(2)}`}
            </p>
            {savedCard && (
              <p className="mt-1 text-xs text-white/50">
                {savedCard.brand ?? "Card"} ending in {savedCard.lastFour ?? "••••"}
              </p>
            )}
          </div>
          {billingConfigured && (
            <button
              type="button"
              disabled={paymentMethodSetup.isPending}
              onClick={() => paymentMethodSetup.mutate()}
              className="cursor-pointer rounded-sm bg-accent px-3 py-1.5 text-xs font-medium text-background transition-colors hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              Add Payment Method
            </button>
          )}
        </div>
        {billingConfigured && (
          <div className="mt-3 flex items-center justify-between border-t border-border-secondary pt-3">
            <div>
              <p className="text-xs font-medium text-white/60">Auto recharge</p>
              <p className="mt-1 text-xs text-white/50">
                {rechargeSettings?.enabled
                  ? `On — charges $${(rechargeSettings.amountCents / 100).toFixed(2)} when the balance drops below $${(rechargeSettings.thresholdMicros / 1_000_000).toFixed(2)}.`
                  : rechargeSettings?.disabledByFailures
                    ? "Off — disabled after repeated declined payments. Set up again to re-enable."
                    : "Off — automatically charges a saved card when credits run low."}
              </p>
            </div>
            <div className="flex shrink-0 gap-2">
              {rechargeSettings?.enabled && (
                <button
                  type="button"
                  disabled={updateAutoRecharge.isPending}
                  onClick={turnOffRecharge}
                  className="cursor-pointer text-xs text-white/60 transition-colors hover:text-primary disabled:opacity-60"
                >
                  Turn off
                </button>
              )}
              <button
                type="button"
                disabled={paymentMethodSetup.isPending}
                onClick={openRechargeDialog}
                className="cursor-pointer text-xs text-accent transition-colors hover:opacity-80 disabled:opacity-60"
              >
                {rechargeSettings?.enabled ? "Edit" : "Set up"}
              </button>
            </div>
          </div>
        )}
      </div>

      {rechargeOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => {
            if (!updateAutoRecharge.isPending) setRechargeOpen(false);
          }}
          role="presentation"
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Set up auto recharge"
            className="w-full max-w-md rounded-md border border-border-secondary bg-[#262626] p-4 shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-sm font-medium text-primary">Auto recharge</h2>
            <p className="mt-1 text-xs text-white/50">
              We&apos;ll charge a saved card to top up your credits automatically.
            </p>

            <div className="mt-4">
              <label className="text-xs font-medium uppercase tracking-wide text-white/45">
                Recharge amount ($3–$1,000)
              </label>
              <div className="mt-1 flex items-center rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5">
                <span className="text-sm text-white/50">$</span>
                <input
                  type="number"
                  min="3"
                  max="1000"
                  step="1"
                  value={rechargeAmount}
                  onChange={(e) => setRechargeAmount(e.target.value)}
                  className="w-full bg-transparent text-sm text-primary outline-none"
                  aria-label="Recharge amount in dollars"
                />
              </div>
            </div>

            <div className="mt-3">
              <label className="text-xs font-medium uppercase tracking-wide text-white/45">
                Recharge when balance drops below ($0.50–$25)
              </label>
              <div className="mt-1 flex items-center rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5">
                <span className="text-sm text-white/50">$</span>
                <input
                  type="number"
                  min="0.5"
                  max="25"
                  step="0.5"
                  value={rechargeThreshold}
                  onChange={(e) => setRechargeThreshold(e.target.value)}
                  className="w-full bg-transparent text-sm text-primary outline-none"
                  aria-label="Threshold balance in dollars"
                />
              </div>
            </div>

            {!rechargeAmountValid && rechargeAmount !== "" && (
              <p className="mt-2 text-xs text-red-300">
                Amount must be between $3 and $1,000.
              </p>
            )}
            {!rechargeThresholdValid && rechargeThreshold !== "" && (
              <p className="mt-2 text-xs text-red-300">
                Threshold must be between $0.50 and $25.
              </p>
            )}

            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setRechargeOpen(false)}
                className="cursor-pointer rounded-sm border border-border-secondary px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-white/5"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={rechargeSubmitDisabled}
                onClick={submitRecharge}
                className="cursor-pointer rounded-sm bg-accent px-3 py-1.5 text-xs font-medium text-background transition-colors hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {updateAutoRecharge.isPending ? "Saving…" : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total cost</p>
          <p className="mt-1 text-2xl font-medium text-primary">
            {usageError ? "—" : `$${(totals?.costUsd ?? 0).toFixed(4)}`}
          </p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total requests</p>
          <p className="mt-1 text-2xl font-medium text-primary">
            {usageError ? "—" : (totals?.requests ?? 0).toLocaleString()}
          </p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total tokens</p>
          <p className="mt-1 text-2xl font-medium text-primary">
            {usageError
              ? "—"
              : formatTokenCount((totals?.inputTokens ?? 0) + (totals?.outputTokens ?? 0))}
          </p>
        </div>
      </div>

      <div className="mt-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-primary">
            Total {usageMetric === "cost" ? "cost" : "tokens"} ({chartGranularity})
          </h3>
          <p className="mt-1 text-xs text-white/60">
            {usageMetric === "cost"
              ? `Cost per ${chartGranularity === "hourly" ? "hour" : "day"}.`
              : `Tokens per ${chartGranularity === "hourly" ? "hour" : "day"}.`}
          </p>
        </div>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => setUsageMetric("cost")}
            className={`rounded-sm px-2 py-1 text-xs transition-colors ${
              usageMetric === "cost"
                ? "bg-white/10 font-medium text-primary"
                : "text-white/60 hover:bg-white/5 hover:text-primary"
            }`}
          >
            Cost
          </button>
          <button
            type="button"
            onClick={() => setUsageMetric("tokens")}
            className={`rounded-sm px-2 py-1 text-xs transition-colors ${
              usageMetric === "tokens"
                ? "bg-white/10 font-medium text-primary"
                : "text-white/60 hover:bg-white/5 hover:text-primary"
            }`}
          >
            Tokens
          </button>
        </div>
      </div>

      <div className="rounded-md border border-border-secondary bg-[#262626] p-2">
        <UsageChart series={series} metric={usageMetric} range={logRange} />
        {series.length > 0 && (
          <div className="ml-[44px] -mt-2 flex w-[calc(100%-44px)] gap-0 text-[10px] text-white/40">
            {axisLabels.map((label, i) => (
              <span key={i} className="flex-1 text-center">
                {label}
              </span>
            ))}
          </div>
        )}
      </div>

      <div className="mt-4">
        <h3 className="text-sm font-medium text-primary">Most used models</h3>
        <p className="mt-1 text-xs text-white/60">
          Models ranked by request count in this range.
        </p>
      </div>
      <div className="max-w-full overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Rank</th>
              <th className="px-2 py-2 font-medium">Model</th>
              <th className="px-2 py-2 text-right font-medium">Requests</th>
              <th className="px-2 py-2 text-right font-medium">Input tokens</th>
              <th className="px-2 py-2 text-right font-medium">Output tokens</th>
              <th className="px-2 py-2 text-right font-medium rounded-tr-md">Cost</th>
            </tr>
          </thead>
          <tbody>
            {summary.isLoading ? (
              <SkeletonRows columns={6} />
            ) : modelUsage.length === 0 ? (
              <tr>
                <td colSpan={6} className="px-2 py-10">
                  <EmptyState message="No usage in this range" />
                </td>
              </tr>
            ) : (
              modelUsage
                .slice()
                .sort((a, b) => b.requests - a.requests)
                .map((m, i) => (
                  <tr key={m.model} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                    <td className="px-2 py-2 text-white/70">{i + 1}</td>
                    <td className="px-2 py-2 font-medium text-primary">{m.model}</td>
                    <td className="px-2 py-2 text-right text-white/70">{m.requests.toLocaleString()}</td>
                    <td className="px-2 py-2 text-right text-white/70">{m.input.toLocaleString()}</td>
                    <td className="px-2 py-2 text-right text-white/70">{m.output.toLocaleString()}</td>
                    <td className="px-2 py-2 text-right text-white/70">${m.cost.toFixed(4)}</td>
                  </tr>
                ))
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4">
        <h3 className="text-sm font-medium text-primary">Usage by Model</h3>
        <p className="mt-1 text-xs text-white/60">
          Breakdown of usage per model.
        </p>
      </div>
      <div className="max-w-full overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Model</th>
              <th className="px-2 py-2 text-right font-medium">Requests</th>
              <th className="px-2 py-2 text-right font-medium">Input tokens</th>
              <th className="px-2 py-2 text-right font-medium">Output tokens</th>
              <th className="px-2 py-2 text-right font-medium">Cache read</th>
              <th className="px-2 py-2 text-right font-medium">Cost</th>
              <th className="px-2 py-2 text-right font-medium rounded-tr-md">Cache write</th>
            </tr>
          </thead>
          <tbody>
            {summary.isLoading ? (
              <SkeletonRows columns={7} />
            ) : modelUsage.length === 0 ? (
              <tr>
                <td colSpan={7} className="px-2 py-10">
                  <EmptyState message="No usage in this range" />
                </td>
              </tr>
            ) : (
              modelUsage.map((m) => (
                <tr key={m.model} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                  <td className="px-2 py-2 font-medium text-primary">{m.model}</td>
                  <td className="px-2 py-2 text-right text-white/70">{m.requests.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">{m.input.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">{m.output.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">{m.cacheRead.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">${m.cost.toFixed(4)}</td>
                  <td className="px-2 py-2 text-right text-white/40" title="Cache-write tokens are not tracked yet">—</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-6">
        <h3 className="text-sm font-medium text-primary">Usage by Service Account</h3>
        <p className="mt-1 text-xs text-white/60">
          Breakdown of usage per service account key.
        </p>
      </div>
      <div className="max-w-full overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Service account</th>
              <th className="px-2 py-2 text-right font-medium">Requests</th>
              <th className="px-2 py-2 text-right font-medium">Input tokens</th>
              <th className="px-2 py-2 text-right font-medium">Output tokens</th>
              <th className="px-2 py-2 text-right font-medium rounded-tr-md">Cost</th>
            </tr>
          </thead>
          <tbody>
            {(() => {
              if (summary.isLoading) {
                return <SkeletonRows columns={5} />;
              }
              const rows = (summary.data?.byKey ?? []).map((k) => ({
                key: k.keyId ?? "unknown",
                requests: k.requests,
                input: k.inputTokens,
                output: k.outputTokens,
                cost: k.costUsd,
              }));
              if (rows.length === 0) {
                return (
                  <tr>
                    <td colSpan={5} className="px-2 py-10">
                      <EmptyState message="No usage in this range" />
                    </td>
                  </tr>
                );
              }
              return rows.map((k) => (
                <tr key={k.key} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                  <td className="px-2 py-2 font-medium text-primary">{k.key}</td>
                  <td className="px-2 py-2 text-right text-white/70">{k.requests.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">{k.input.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">{k.output.toLocaleString()}</td>
                  <td className="px-2 py-2 text-right text-white/70">${k.cost.toFixed(4)}</td>
                </tr>
              ));
            })()}
          </tbody>
        </table>
      </div>
    </div>
  );
}
