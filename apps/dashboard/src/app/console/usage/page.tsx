"use client";

import { useUsage } from "@/domains/usage/hooks";

export default function UsagePage() {
  const usage = useUsage();
  const rows = usage.data?.rows ?? [];
  const totalCost = rows.reduce((s, r) => s + (r.costUsd ?? 0), 0);
  const totalRequests = rows.length;
  const totalTokens = rows.reduce((s, r) => s + (r.inputTokens ?? 0) + (r.outputTokens ?? 0), 0);

  return (
    <div className="flex min-h-full flex-1 flex-col px-2">
      <h2 className="text-sm font-medium text-primary">Usage</h2>
      <p className="mt-1 text-xs text-white/60">
        Token usage and cost across your service accounts.
      </p>
      <div className="mt-4 grid min-w-0 grid-cols-3 gap-4">
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
      <div className="mt-6 min-w-0">
        <div className="max-w-full overflow-x-auto rounded-md border border-border-secondary">
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
              {rows.map((r) => (
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
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}