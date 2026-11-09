"use client";

import { useEffect, useRef, useMemo } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import { GoTrash, GoKebabHorizontal } from "react-icons/go";
import { MenuDropdown } from "@/components/menu-dropdown";
import { useApiKeys, useRevokeApiKey } from "@/domains/keys/hooks";
import { useUsage, useUsageSummary } from "@/domains/usage/hooks";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

/** Serves the accent from the stylesheet so the bars follow the theme tokens
 *  instead of a hard-coded blue. Canvas paint can't consume `var()`, so the
 *  computed value is read out once per chart build. */
function themeColor(fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue("--accent")
    .trim();
  return value || fallback;
}

function withAlpha(hex: string, alpha: number): string {
  const compact = hex.replace("#", "");
  if (compact.length === 3) {
    return `rgba(${parseInt(compact[0], 16) * 17}, ${parseInt(compact[1], 16) * 17}, ${parseInt(compact[2], 16) * 17}, ${alpha})`;
  }
  if (compact.length === 6) {
    return `rgba(${parseInt(compact.slice(0, 2), 16)}, ${parseInt(compact.slice(2, 4), 16)}, ${parseInt(compact.slice(4, 6), 16)}, ${alpha})`;
  }
  return hex;
}

export default function ServiceAccountDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const id = params?.id ?? "";
  const keys = useApiKeys();
  const revoke = useRevokeApiKey();
  const usage = useUsage(id);
  const account = (keys.data?.apiKeys ?? []).find((k) => k.id === id);

  const rows = usage.data?.rows ?? [];
  // Totals are aggregated server-side over every row; `rows` is capped and only feeds the log table/chart.
  const summary = useUsageSummary({ keyId: id });
  const totalCost = summary.data?.totals.costUsd ?? 0;
  const totalRequests = summary.data?.totals.requests ?? 0;
  const totalTokens = (summary.data?.totals.inputTokens ?? 0) + (summary.data?.totals.outputTokens ?? 0);

  const modelCounts = useMemo(
    () => (summary.data?.byModel ?? []).map((m) => ({ model: m.model || "unknown", count: m.requests })),
    [summary.data],
  );

  const chartRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);

  useEffect(() => {
    const el = chartRef.current;
    if (!el || modelCounts.length === 0) return;
    const data = [modelCounts.map((_, i) => i), modelCounts.map((m) => m.count)] as [number[], number[]];

    const options = (width: number): uPlot.Options => {
      const accent = themeColor("#e5e5e5");
      return {
        width,
        height: 192,
        scales: {
          y: { range: [0, Math.max(...modelCounts.map((m) => m.count))] },
        },
        series: [
          {},
          {
            label: "Requests",
            stroke: accent,
            fill: withAlpha(accent, 1),
            paths: uPlot.paths.bars!({ size: [0.12], gap: 2 }),
          },
        ],
        axes: [
          {
            stroke: "rgba(255,255,255,0.55)",
            grid: { show: false },
            ticks: { show: false },
            values: (_, vals) => vals.map((v) => modelCounts[v]?.model ?? ""),
          },
          {
            stroke: "rgba(255,255,255,0.55)",
            grid: { stroke: "rgba(255,255,255,0.07)" },
            ticks: { show: false },
          },
        ],
      };
    };

    const settle = () => {
      const width = Math.max(120, Math.floor(el.clientWidth));
      if (plotRef.current) {
        plotRef.current.setSize({ width, height: 192 });
      } else {
        plotRef.current = new uPlot(options(width), data, el);
      }
    };

    settle();
    const observer = new ResizeObserver(settle);
    observer.observe(el);
    return () => {
      observer.disconnect();
      plotRef.current?.destroy();
      plotRef.current = null;
    };
  }, [modelCounts]);

  return (
    <div className="px-2 py-1">
      <div className="mb-4 flex items-center gap-2 text-xs text-white/60">
        <Link href="/console/settings/service-accounts" className="hover:text-primary">
          Service Accounts
        </Link>
        <span>/</span>
        <span className="text-primary">{account?.name ?? id}</span>
      </div>

      <div className="flex items-start justify-between gap-4">
        <h1 className="text-base font-medium text-primary">
          {account?.name ?? "Service Account"}
        </h1>
        <MenuDropdown
          label="Account actions"
          trigger={<GoKebabHorizontal className="size-3.5" />}
          items={[
            { label: "Revoke secret", tone: "warning", onSelect: () => revoke.mutateAsync(id) },
            {
              label: "Delete service account",
              icon: GoTrash,
              tone: "danger",
              onSelect: async () => {
                await revoke.mutateAsync(id);
                router.push("/console/settings/service-accounts");
              },
            },
          ]}
        />
      </div>
      <p className="mt-1 text-sm text-white/60">
        Details for this service account.
      </p>
      {(keys.isError || revoke.isError || usage.isError) && (
        <p className="mt-2 text-xs text-red-400">
          {keys.isError ? keys.error.message : revoke.error?.message ?? usage.error?.message}
        </p>
      )}
      {account && keys.isSuccess && (
        <div className="mt-3 grid grid-cols-2 gap-4 text-xs sm:grid-cols-3">
          <div>
            <p className="text-white/40">Key</p>
            <p className="mt-0.5 font-mono text-white/80">
              {account.prefix ?? ""}
              {account.start ?? "…"}…
            </p>
          </div>
          <div>
            <p className="text-white/40">Created</p>
            <p className="mt-0.5 text-white/80">{new Date(account.createdAt).toLocaleDateString()}</p>
          </div>
          <div>
            <p className="text-white/40">Status</p>
            <p className={`mt-0.5 ${account.enabled ? "text-green-400" : "text-red-400"}`}>
              {account.enabled ? "Active" : "Revoked"}
            </p>
          </div>
        </div>
      )}

      <div className="mt-6 grid min-w-0 grid-cols-3 gap-4">
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total cost</p>
          <p className="mt-1 text-2xl font-medium text-primary">${totalCost.toFixed(4)}</p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total requests</p>
          <p className="mt-1 text-2xl font-medium text-primary">{totalRequests.toLocaleString()}</p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total tokens</p>
          <p className="mt-1 text-2xl font-medium text-primary">{totalTokens.toLocaleString()}</p>
        </div>
      </div>

      <div className="mt-6 rounded-md border border-border-secondary bg-[#262626] p-4">
        <h2 className="text-sm font-medium text-primary">Models per request</h2>
        <div ref={chartRef} className="mt-4 h-56 w-full" />
      </div>

      <div className="mt-6 min-w-0">
        <h2 className="text-sm font-medium text-primary">Logs</h2>
        <div className="mt-3 max-w-full overflow-x-auto rounded-md border border-border-secondary">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b border-border-secondary bg-tertiary text-white/40">
                <th className="px-2 py-2 font-medium rounded-tl-md">Time</th>
                <th className="px-2 py-2 font-medium">Model</th>
                <th className="px-2 py-2 font-medium">Status</th>
                <th className="px-2 py-2 text-right font-medium">In</th>
                <th className="px-2 py-2 text-right font-medium">Out</th>
                <th className="px-2 py-2 text-right font-medium">Latency</th>
                <th className="px-2 py-2 text-right font-medium">Cost</th>
                <th className="px-2 py-2 font-medium rounded-tr-md">Date</th>
              </tr>
            </thead>
            <tbody>
              {usage.isLoading && (
                <tr>
                  <td colSpan={8} className="px-2 py-3 text-white/60">
                    Loading usage…
                  </td>
                </tr>
              )}
              {rows.map((r) => {
                return (
                  <tr key={r.id} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                    <td className="px-2 py-2 whitespace-nowrap text-white/70">
                      {r.createdAt ? new Date(r.createdAt).toLocaleTimeString() : "-"}
                    </td>
                    <td className="px-2 py-2">
                      <div className="min-w-0">
                        <div className="font-medium text-primary">{r.model ?? "-"}</div>
                        <div className="text-[10px] text-white/40">{r.provider ?? ""}</div>
                      </div>
                    </td>
                    <td className="px-2 py-2">
                      <span
                        className={`rounded-sm px-1.5 py-0.5 font-medium ${
                          r.status === 200 ? "bg-white/10 text-primary" : "bg-white/10 text-yellow-400"
                        }`}
                      >
                        {r.status ?? "-"}
                      </span>
                    </td>
                    <td className="px-2 py-2 text-right text-white/70">{(r.inputTokens ?? 0).toLocaleString()}</td>
                    <td className="px-2 py-2 text-right text-white/70">{(r.outputTokens ?? 0).toLocaleString()}</td>
                    <td className="px-2 py-2 text-right text-white/70">
                      {r.latencyMs != null ? `${(r.latencyMs / 1000).toFixed(1)}s` : "-"}
                    </td>
                    <td className="px-2 py-2 text-right text-white/70">${(r.costUsd ?? 0).toFixed(4)}</td>
                    <td className="px-2 py-2 whitespace-nowrap text-white/70">
                      {r.createdAt ? new Date(r.createdAt).toLocaleDateString() : "-"}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}