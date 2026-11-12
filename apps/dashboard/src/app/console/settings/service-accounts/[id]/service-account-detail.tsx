"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { GoTrash, GoKebabHorizontal, GoCircleSlash } from "react-icons/go";
import { IoChevronForward } from "react-icons/io5";
import { FiKey } from "react-icons/fi";
import { FaCircle } from "react-icons/fa";
import { MenuDropdown } from "@/components/menu-dropdown";
import { Skeleton } from "@/components/skeleton";
import { useApiKeys, useRevokeApiKey } from "@/domains/keys/hooks";
import { useModels } from "@/domains/models/hooks";
import { useUsage, useUsageSummary } from "@/domains/usage/hooks";
import { formatTokenCount, modelDisplayName } from "@/lib/usage-format";
import { ConfirmDialog } from "@/components/confirm-dialog";

const LOGS_PAGE_SIZE = 50;

const MODEL_COLORS = [
  "#60a5fa",
  "#34d399",
  "#fbbf24",
  "#f87171",
  "#a78bfa",
  "#38bdf8",
  "#fb923c",
  "#f472b6",
];

export default function ServiceAccountDetail({ id }: { id: string }) {
  const router = useRouter();
  const keys = useApiKeys();
  const revoke = useRevokeApiKey();
  const usage = useUsage(id);
  const models = useModels();
  const account = (keys.data?.apiKeys ?? []).find((k) => k.id === id);
  const displayName = account?.name
    ? account.name.charAt(0).toUpperCase() + account.name.slice(1)
    : id;
  const availableModels = (models.data?.models ?? []).filter((m) => m.enabled).length;

  const rows = usage.data?.rows ?? [];
  const [logsPage, setLogsPage] = useState(1);
  const [confirmAction, setConfirmAction] = useState<"revoke" | "delete" | null>(null);

  useEffect(() => {
    setLogsPage((p) =>
      Math.min(p, Math.max(1, Math.ceil(rows.length / LOGS_PAGE_SIZE))),
    );
  }, [rows.length]);

  const logsPageCount = Math.max(1, Math.ceil(rows.length / LOGS_PAGE_SIZE));
  const logsPageRows = rows.slice(
    (logsPage - 1) * LOGS_PAGE_SIZE,
    logsPage * LOGS_PAGE_SIZE,
  );

  const summary = useUsageSummary({ keyId: id });
  const chartSummary = useUsageSummary({ keyId: id, range: "30d" });
  const totalCost = summary.data?.totals.costUsd ?? 0;
  const totalRequests = summary.data?.totals.requests ?? 0;
  const totalTokens = (summary.data?.totals.inputTokens ?? 0) + (summary.data?.totals.outputTokens ?? 0);

  const [thirtyDays, setThirtyDays] = useState<{ key: string; label: string }[]>([]);
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const now = new Date();
      const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
      setThirtyDays(
        Array.from({ length: 30 }, (_, i) => {
          const d = new Date(today);
          d.setDate(d.getDate() - (29 - i));
          const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
          return {
            key,
            label: i % 2 === 0 ? `${d.getMonth() + 1}/${d.getDate()}` : "",
          };
        }),
      );
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  const byModelSeries = chartSummary.data?.byModelSeries;
  const byBucketModel = new Map<string, Map<string, number>>();
  if (Array.isArray(byModelSeries)) {
    for (const p of byModelSeries) {
      const k = p.bucket.slice(0, 10);
      let bm = byBucketModel.get(k);
      if (!bm) {
        bm = new Map();
        byBucketModel.set(k, bm);
      }
      bm.set(p.model, (bm.get(p.model) ?? 0) + p.requests);
    }
  } else {
    const cutoff = new Date();
    cutoff.setHours(0, 0, 0, 0);
    cutoff.setDate(cutoff.getDate() - 29);
    for (const r of rows) {
      if (!r.createdAt) continue;
      const d = new Date(r.createdAt);
      if (d < cutoff) continue;
      const k = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
      const model = r.model || "unknown";
      let bm = byBucketModel.get(k);
      if (!bm) {
        bm = new Map();
        byBucketModel.set(k, bm);
      }
      bm.set(model, (bm.get(model) ?? 0) + 1);
    }
  }

  let chartModels = (chartSummary.data?.byModel ?? [])
    .filter((m) => m.requests > 0)
    .map((m, i) => ({
      model: m.model,
      color: MODEL_COLORS[i % MODEL_COLORS.length],
    }));
  if (chartModels.length === 0) {
    const totals = new Map<string, number>();
    for (const bm of byBucketModel.values()) {
      for (const [model, count] of bm.entries()) {
        totals.set(model, (totals.get(model) ?? 0) + count);
      }
    }
    chartModels = [...totals.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([model], i) => ({ model, color: MODEL_COLORS[i % MODEL_COLORS.length] }));
  }

  let maxDailyRequests = 1;
  for (const d of thirtyDays) {
    const bm = byBucketModel.get(d.key);
    if (!bm) continue;
    let total = 0;
    for (const m of chartModels) total += bm.get(m.model) ?? 0;
    if (total > maxDailyRequests) maxDailyRequests = total;
  }

  return (
    <div className="px-2 py-1">
      <div className="mb-4 flex items-center gap-2 text-sm text-white/60">
        <Link href="/console/settings/service-accounts" className="hover:text-primary">
          Service Accounts
        </Link>
        <IoChevronForward className="size-3.5 text-white/40" />
        <span className="text-primary">{displayName}</span>
      </div>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="inline-flex size-11 shrink-0 items-center justify-center rounded-md border border-border-secondary bg-[#262626]">
            <FiKey className="size-5 text-white/70" />
          </div>
          <div>
            <span className="block text-base font-medium text-primary">{displayName}</span>
            <div className="mt-0.5 flex items-center gap-1.5 text-xs text-white/60">
              <span>Service account</span>
              {account && (
                <>
                  <FaCircle className="size-1.5" />
                  <span>{account.enabled ? "Active" : "Disabled"}</span>
                  <FaCircle className="size-1.5" />
                  <span>
                    Created on{" "}
                    {new Date(account.createdAt).toLocaleDateString("en-US", {
                      month: "short",
                      day: "numeric",
                    })}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>
        <MenuDropdown
          label="Account actions"
          trigger={<GoKebabHorizontal className="size-3.5" />}
          items={[
            { label: "Revoke secret", icon: GoCircleSlash, tone: "warning", onSelect: () => setConfirmAction("revoke") },
            {
              label: "Delete service account",
              icon: GoTrash,
              tone: "danger",
              onSelect: () => setConfirmAction("delete"),
            },
          ]}
        />
      </div>
      <div className="mt-6">
        <h2 className="text-sm font-medium text-primary">Models</h2>
        <p className="mt-1 text-sm text-white/60">
          {models.isLoading ? <Skeleton className="inline-block h-3.5 w-16 align-middle" /> : `${availableModels} Available`}
        </p>
      </div>
      <div className="mt-4">
        <h2 className="text-sm font-medium text-primary">Spend</h2>
        <p className="mt-1 text-sm text-white/60">${totalCost.toFixed(4)}</p>
      </div>
      {(keys.isError || revoke.isError || usage.isError) && (
        <p className="mt-2 text-xs text-red-400">
          {keys.isError ? keys.error.message : revoke.error?.message ?? usage.error?.message}
        </p>
      )}

      <div className="mt-6 grid min-w-0 grid-cols-3 gap-4">
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total cost</p>
          <p className="mt-1 text-2xl font-medium text-primary">${totalCost.toFixed(4)}</p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total requests</p>
          <p className="mt-1 text-2xl font-medium text-primary">{formatTokenCount(totalRequests)}</p>
        </div>
        <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
          <p className="text-xs text-white/60">Total tokens</p>
          <p className="mt-1 text-2xl font-medium text-primary">{formatTokenCount(totalTokens)}</p>
        </div>
      </div>

      <div className="mt-6 rounded-md border border-border-secondary bg-[#262626] p-2">
        <p className="text-sm font-medium text-primary">Request per Model</p>
        {(chartSummary.isLoading && usage.isLoading) || thirtyDays.length === 0 ? (
          <div className="mt-4 flex h-40 items-end gap-2">
            {["h-1/4", "h-3/5", "h-2/5", "h-4/5", "h-1/3", "h-3/4", "h-1/2", "h-2/3"].map((height) => (
              <Skeleton key={height} className={`w-8 ${height}`} />
            ))}
            <Skeleton className="h-1/2 flex-1" />
          </div>
        ) : chartModels.length === 0 ? (
          <p className="mt-4 text-xs text-white/60">No requests yet.</p>
        ) : (
          <>
            <div className="relative mt-4 h-40">
              <div className="pointer-events-none absolute inset-0">
                {[0, 25, 50, 75, 100].map((p) => (
                  <div
                    key={p}
                    className="absolute inset-x-0 border-t border-dotted border-white/10"
                    style={p === 100 ? { bottom: 0 } : { top: `${p}%` }}
                  />
                ))}
              </div>
              <div className="relative flex h-full items-end">
                {thirtyDays.map((d, i) => {
                  const bm = byBucketModel.get(d.key);
                  const dayTotal = chartModels.reduce((s, m) => s + (bm?.get(m.model) ?? 0), 0);
                  const dayModels = chartModels
                    .map((m) => ({ ...m, requests: bm?.get(m.model) ?? 0 }))
                    .filter((m) => m.requests > 0);
                  return (
                    <div
                      key={d.key}
                      className={`group relative flex h-full flex-1 flex-col-reverse items-center border-l border-dotted border-white/10 transition-colors hover:bg-white/5 ${i === thirtyDays.length - 1 ? "border-r border-dotted border-white/10" : ""}`}
                    >
                      {dayTotal > 0 && (
                        <>
                          <div
                            className="flex w-4 flex-col-reverse overflow-hidden rounded-t-sm"
                            style={{ height: `${(dayTotal / maxDailyRequests) * 100}%` }}
                          >
                            {chartModels.map((m) => {
                              const v = bm?.get(m.model) ?? 0;
                              if (!v) return null;
                              return (
                                <div
                                  key={m.model}
                                  className="w-full"
                                  style={{
                                    height: `${(v / dayTotal) * 100}%`,
                                    backgroundColor: m.color,
                                  }}
                                />
                              );
                            })}
                          </div>
                          <div
                            className={`pointer-events-none absolute bottom-full z-20 mb-2 hidden group-hover:block ${i < 4 ? "left-0" : i > 25 ? "right-0" : "left-1/2 -translate-x-1/2"}`}
                          >
                            <div className="min-w-[11rem] whitespace-nowrap rounded-md border border-border-secondary bg-[#1f1f1f] p-2 text-xs shadow-lg">
                              <p className="mb-1.5 text-white/40">{d.key}</p>
                              <div className="flex flex-col gap-1.5">
                                {dayModels.map((m) => (
                                  <div key={m.model} className="flex items-center gap-2">
                                    <span
                                      className="inline-block size-2.5 shrink-0 rounded-[2px]"
                                      style={{ backgroundColor: m.color }}
                                    />
                                    <span className="text-white/80">{modelDisplayName(m.model)}</span>
                                    <span className="ml-auto pl-2 tabular-nums text-primary">
                                      {formatTokenCount(m.requests)}
                                    </span>
                                  </div>
                                ))}
                              </div>
                            </div>
                          </div>
                        </>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
            <div className="mt-1 flex text-[10px] text-white/40">
              {thirtyDays.map((d) => (
                <div key={d.key} className="flex flex-1 justify-center whitespace-nowrap">
                  {d.label}
                </div>
              ))}
            </div>
            <div className="mt-4 flex flex-wrap items-center gap-x-6 gap-y-3 text-xs text-white/60">
              {chartModels.map((m) => (
                <div key={m.model} className="flex items-center gap-2.5">
                  <span
                    className="inline-block size-2.5 shrink-0 rounded-[2px]"
                    style={{ backgroundColor: m.color }}
                  />
                  <span className="truncate leading-none">{modelDisplayName(m.model)}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </div>

      <div className="mt-6">
        <h2 className="text-sm font-medium text-primary">Logs</h2>
        <div className="mt-3 rounded-md border border-border-secondary">
          <table className="w-full min-w-[48rem] text-left text-xs">
            <thead>
              <tr className="border-b border-border-secondary bg-tertiary text-white/40">
                <th className="px-2 py-2 font-medium rounded-tl-md">Time</th>
                <th className="px-2 py-2 font-medium">Model</th>
                <th className="px-2 py-2 font-medium">Status</th>
                <th className="px-2 py-2 text-right font-medium">In</th>
                <th className="px-2 py-2 text-right font-medium">Out</th>
                <th className="px-2 py-2 text-right font-medium">Latency</th>
                <th className="px-2 py-2 text-right font-medium rounded-tr-md">Cost</th>
              </tr>
            </thead>
            <tbody>
              {usage.isLoading && (
                <>
                  {[0, 1, 2, 3, 4].map((row) => (
                    <tr key={row} className="border-b border-border-secondary/50">
                      <td colSpan={7} className="px-2 py-2.5">
                        <Skeleton className={["w-44", "w-36", "w-40", "w-32", "w-40"][row]} />
                      </td>
                    </tr>
                  ))}
                </>
              )}
              {logsPageRows.map((r) => {
                return (
                  <tr key={r.id} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                    <td className="px-2 py-2 whitespace-nowrap tabular-nums text-white/70" suppressHydrationWarning>
                      {r.createdAt
                        ? new Date(r.createdAt).toLocaleString("en-US", {
                            dateStyle: "medium",
                            timeStyle: "short",
                          })
                        : "-"}
                    </td>
                    <td className="px-2 py-2 font-medium text-primary">
                      {r.model ? modelDisplayName(r.model) : "-"}
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
                    <td className="px-2 py-2 text-right tabular-nums text-white/70">{formatTokenCount(r.inputTokens ?? 0)}</td>
                    <td className="px-2 py-2 text-right tabular-nums text-white/70">{formatTokenCount(r.outputTokens ?? 0)}</td>
                    <td className="px-2 py-2 text-right tabular-nums text-white/70">
                      {r.latencyMs != null ? `${(r.latencyMs / 1000).toFixed(1)}s` : "-"}
                    </td>
                    <td className="px-2 py-2 text-right tabular-nums text-white/70">${(r.costUsd ?? 0).toFixed(4)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {rows.length > 0 && (
          <div className="mt-2 flex items-center justify-between text-xs text-white/60">
            <span>
              Showing {(logsPage - 1) * LOGS_PAGE_SIZE + 1}–
              {Math.min(logsPage * LOGS_PAGE_SIZE, rows.length)} of {rows.length}
            </span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setLogsPage((p) => Math.max(1, p - 1))}
                disabled={logsPage <= 1}
                className="cursor-pointer rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs text-primary transition-colors hover:bg-white/10 disabled:cursor-default disabled:opacity-40 disabled:hover:bg-tertiary"
              >
                Previous
              </button>
              <span className="tabular-nums">
                Page {logsPage} of {logsPageCount}
              </span>
              <button
                type="button"
                onClick={() => setLogsPage((p) => Math.min(logsPageCount, p + 1))}
                disabled={logsPage >= logsPageCount}
                className="cursor-pointer rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs text-primary transition-colors hover:bg-white/10 disabled:cursor-default disabled:opacity-40 disabled:hover:bg-tertiary"
              >
                Next
              </button>
            </div>
          </div>
        )}
      </div>

      <ConfirmDialog
        open={confirmAction !== null}
        title={confirmAction === "delete" ? "Delete service account?" : "Revoke secret token?"}
        description={
          confirmAction === "delete" ? (
            <>
              This will permanently delete{" "}
              <span className="text-primary">{displayName}</span> and revoke its secret token.
              The key will stop working immediately. This action cannot be undone.
            </>
          ) : (
            <>
              This will revoke the secret token for{" "}
              <span className="text-primary">{displayName}</span>. The key will stop working
              immediately. This action cannot be undone.
            </>
          )
        }
        confirmLabel={
          revoke.isPending
            ? confirmAction === "delete"
              ? "Deleting…"
              : "Revoking…"
            : confirmAction === "delete"
              ? "Delete service account"
              : "Revoke secret"
        }
        tone={confirmAction === "delete" ? "danger" : "warning"}
        isPending={revoke.isPending}
        error={revoke.isError ? revoke.error.message : null}
        onCancel={() => {
          if (!revoke.isPending) setConfirmAction(null);
        }}
        onConfirm={async () => {
          if (!confirmAction) return;
          try {
            await revoke.mutateAsync(id);
            if (confirmAction === "delete") {
              router.push("/console/settings/service-accounts");
            } else {
              setConfirmAction(null);
            }
          } catch {
          }
        }}
      />
    </div>
  );
}
