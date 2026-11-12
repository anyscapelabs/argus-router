"use client";

import { useEffect, useState } from "react";
import { RangeControls } from "@/components/usage-range-controls";
import { Skeleton } from "@/components/skeleton";
import { formatTokenCount } from "@/lib/usage-format";
import { useUsage } from "@/domains/usage/hooks";
import type { UsageRange } from "@/domains/usage/api";

const LOGS_PAGE_SIZE = 50;

export default function LogsPage() {
  const [logRange, setLogRange] = useState<UsageRange>("7d");
  const [logsPage, setLogsPage] = useState(1);

  const usage = useUsage({ range: logRange });
  const usageRows = usage.data?.rows ?? [];

  useEffect(() => {
    setLogsPage(1);
  }, [logRange]);
  useEffect(() => {
    setLogsPage((p) =>
      Math.min(p, Math.max(1, Math.ceil(usageRows.length / LOGS_PAGE_SIZE))),
    );
  }, [usageRows.length]);

  const logsPageCount = Math.max(1, Math.ceil(usageRows.length / LOGS_PAGE_SIZE));
  const logsPageRows = usageRows.slice(
    (logsPage - 1) * LOGS_PAGE_SIZE,
    logsPage * LOGS_PAGE_SIZE,
  );

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h2 className="mt-4 text-sm font-medium text-primary">Logs</h2>
          <p className="mt-1 text-xs text-white/60">
            Recent requests through the gateway.
          </p>
        </div>
        <RangeControls
          range={logRange}
          onRangeChange={setLogRange}
          onRefresh={() => usage.refetch()}
          isFetching={usage.isFetching}
        />
      </div>
      {usage.isError && (
        <div
          role="alert"
          className="rounded-md border border-red-400/30 bg-red-400/10 px-3 py-2 text-xs text-red-300"
        >
          Couldn&apos;t load logs: {usage.error?.message ?? "Unknown error"}.{" "}
          <button
            type="button"
            onClick={() => void usage.refetch()}
            className="cursor-pointer underline underline-offset-2 hover:text-red-200"
          >
            Retry
          </button>
        </div>
      )}
      <div className="max-w-full overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Time</th>
              <th className="px-2 py-2 font-medium">Status</th>
              <th className="px-2 py-2 font-medium">Model</th>
              <th className="px-2 py-2 font-medium">Service account key</th>
              <th className="px-2 py-2 text-right font-medium">Duration</th>
              <th className="px-2 py-2 text-right font-medium">Tokens</th>
              <th className="px-2 py-2 text-right font-medium rounded-tr-md">Cost</th>
            </tr>
          </thead>
          <tbody>
            {usage.isLoading && (
              <>
                {[0, 1, 2, 3, 4].map((row) => (
                  <tr key={row} className="border-b border-border-secondary/50">
                    <td colSpan={7} className="px-2 py-2.5">
                      <Skeleton className={["w-44", "w-36", "w-48", "w-40", "w-32"][row]} />
                    </td>
                  </tr>
                ))}
              </>
            )}
            {usageRows.length === 0 && !usage.isLoading && (
              <tr>
                <td colSpan={7} className="px-2 py-6 text-center text-xs text-white/40">
                  No usage in this range
                </td>
              </tr>
            )}
            {logsPageRows.map((r) => (
              <tr key={r.id} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                <td className="px-2 py-2 whitespace-nowrap tabular-nums text-white/70" suppressHydrationWarning>
                  {r.createdAt
                    ? new Date(r.createdAt).toLocaleString("en-US", {
                        dateStyle: "medium",
                        timeStyle: "short",
                      })
                    : "-"}
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
                <td className="px-2 py-2 font-medium text-primary">{r.model ?? "-"}</td>
                <td className="px-2 py-2 text-white/70">{r.keyId ?? "-"}</td>
                <td className="px-2 py-2 text-right tabular-nums text-white/70">
                  {r.latencyMs != null ? `${(r.latencyMs / 1000).toFixed(1)}s` : "-"}
                </td>
                <td className="px-2 py-2 text-right tabular-nums text-white/70">
                  {formatTokenCount((r.inputTokens ?? 0) + (r.outputTokens ?? 0))}
                </td>
                <td className="px-2 py-2 text-right tabular-nums text-white/70">${(r.costUsd ?? 0).toFixed(4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {usageRows.length > 0 && (
        <div className="flex items-center justify-between text-xs text-white/60">
          <span>
            Showing {(logsPage - 1) * LOGS_PAGE_SIZE + 1}–
            {Math.min(logsPage * LOGS_PAGE_SIZE, usageRows.length)} of {usageRows.length}
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
  );
}
