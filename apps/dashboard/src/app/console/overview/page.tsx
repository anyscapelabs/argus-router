"use client";

import { useEffect, useMemo, useState } from "react";
import { GoChevronDown, GoDownload, GoChevronRight } from "react-icons/go";
import { useUsage } from "@/domains/usage/hooks";

const platforms = [
  { label: "macOS", detail: "Apple Silicon" },
  { label: "macOS", detail: "Intel" },
  { label: "Windows", detail: ".exe" },
  { label: "Windows", detail: ".msi" },
  { label: "Linux", detail: ".deb" },
  { label: "Linux", detail: ".rpm" },
  { label: "Linux", detail: ".AppImage" },
];

const subTabs = ["Activity", "My usage", "Leaderboard", "Logs", "Audit"];

type RequestLog = {
  id: string;
  date: Date;
  model: string;
  status: number;
  tokens: number;
  duration: string;
  firstToken: string;
  inputTokens: number;
  outputTokens: number;
  cost: string;
};

function generateLogs(): RequestLog[] {
  const models = ["GLM-5.3-Flash", "GPT-5", "Claude 4", "Llama 3"];
  const logs: RequestLog[] = [];
  const today = new Date();
  for (let i = 39; i >= 0; i--) {
    const d = new Date(today);
    d.setMinutes(d.getMinutes() - i * 47);
    const seed = Math.abs(Math.sin(i * 12.9898) * 10000) % 1;
    const status = seed > 0.85 ? 429 : seed > 0.95 ? 500 : 200;
    const model = models[Math.floor(seed * models.length) % models.length];
    const input = 500 + Math.floor(seed * 300);
    const output = 200 + Math.floor(seed * 700);
    logs.push({
      id: `req_${i}`,
      date: d,
      model,
      status,
      tokens: input + output,
      duration: `${(seed * 2 + 0.2).toFixed(1)}s`,
      firstToken: `${Math.floor(seed * 800 + 100)} ms`,
      inputTokens: input,
      outputTokens: output,
      cost: `$${(seed * 0.04).toFixed(2)}`,
    });
  }
  return logs.reverse();
}

function getStatusColor(_status: number): string {
  return "bg-white/25";
}

const levelColors = [
  "bg-[#1a1a1a]",
];

export default function OverviewPage() {
  const [os, setOs] = useState("Unknown");
  const [open, setOpen] = useState(false);
  const [activeSubTab, setActiveSubTab] = useState("Activity");
  const [hovered, setHovered] = useState<RequestLog | null>(null);

  const logs = useMemo(() => generateLogs(), []);
  const usage = useUsage();
  const usageRows = usage.data?.rows ?? [];
  useEffect(() => {
    const platform = navigator.platform.toLowerCase();
    if (platform.includes("mac")) setOs("macOS");
    else if (platform.includes("win")) setOs("Windows");
    else if (platform.includes("linux")) setOs("Linux");
  }, []);

  const recentLogs = logs.slice(-5);

  return (
    <div className="flex min-h-full flex-1 flex-col px-2">
      <div className="mb-4 flex shrink-0 flex-row gap-1">
        {subTabs.map((tab) => {
          const active = activeSubTab === tab;
          return (
            <button
              key={tab}
              type="button"
              onClick={() => setActiveSubTab(tab)}
              className={`cursor-pointer rounded-sm px-2 py-1 text-xs transition-colors ${
                active
                  ? "bg-[#202020] font-medium text-primary"
                  : "text-white/60 hover:bg-[#202020] hover:text-primary"
              }`}
            >
              {tab}
            </button>
          );
        })}
      </div>

      {activeSubTab === "Activity" && (
        <div className="flex gap-6">
          <div className="w-2/3 min-w-0 max-w-full rounded-md border border-border-secondary bg-[#262626] p-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <h2 className="text-sm font-medium text-primary">Connect Argus Agent</h2>
                <p className="mt-1 text-xs text-white/60">
                  Once connected you can access everything available in Console. Not installed yet?{" "}
                  <span className="text-white/80">Download Argus Agent</span>
                </p>
              </div>
              <button type="button" className="shrink-0 text-xs text-white/40 hover:text-primary">
                —
              </button>
            </div>

            <div className="mt-4 flex gap-4 border-b border-border-secondary text-xs">
              <span className="border-b-2 border-primary pb-2 font-medium text-primary">Desktop</span>
            </div>

            <ol className="mt-6">
              <li className="flex items-start gap-3 pb-8">
                <div className="flex shrink-0 flex-col items-center self-stretch">
                  <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-border-secondary bg-transparent text-[11px] text-white/60">
                    1
                  </span>
                  <span className="mt-1 w-px flex-1 bg-border-secondary" />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium text-primary">Install Argus Agent Desktop</p>
                  <p className="mt-0.5 text-xs text-white/40">Skip this if you already have it.</p>
                  <div className="relative mt-3 flex items-center">
                    <button
                      type="button"
                      className="inline-flex cursor-pointer items-center gap-1.5 rounded-l-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                    >
                      <GoDownload className="size-3" />
                      Download for {os}
                    </button>
                    <div className="h-5 w-px bg-black/20" />
                    <button
                      type="button"
                      onClick={() => setOpen((v) => !v)}
                      className="inline-flex cursor-pointer items-center gap-1 rounded-r-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                    >
                      Other platforms
                      <GoChevronDown className={`size-3 transition-transform ${open ? "rotate-180" : ""}`} />
                    </button>
                    {open && (
                      <div className="absolute top-full right-0 z-10 mt-1 w-40 overflow-hidden rounded-md border border-border-secondary bg-[#262626] py-1 shadow-lg">
                        {platforms.map((p, i) => (
                          <button
                            key={`${p.label}-${p.detail}-${i}`}
                            type="button"
                            onClick={() => setOpen(false)}
                            className="mx-1 flex w-[calc(100%-0.5rem)] cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1 text-left text-xs text-white/80 transition-colors hover:bg-[#202020] hover:text-primary"
                          >
                            <span>{p.label}</span>
                            <span className="text-white/40">{p.detail}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              </li>

              <li className="flex items-start gap-3 pb-8">
                <div className="flex shrink-0 flex-col items-center self-stretch">
                  <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-border-secondary bg-transparent text-[11px] text-white/60">
                    2
                  </span>
                  <span className="mt-1 w-px flex-1 bg-border-secondary" />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium text-primary">Connect this workspace</p>
                  <p className="mt-0.5 text-xs text-white/40">
                    In the app, choose <span className="text-white/70">Connect provider → Argus Agent Console</span>.
                    Your browser opens; approve the sign-in and pick <span className="text-white/70">Default</span>.
                  </p>
                </div>
              </li>

              <li className="flex items-start gap-3">
                <div className="flex shrink-0 flex-col items-center">
                  <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-border-secondary bg-transparent text-[11px] text-white/60">
                    3
                  </span>
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium text-primary">Choose a model</p>
                  <p className="mt-0.5 text-xs text-white/40">
                    Pick a model from the model selector dropdown in the app.
                  </p>
                </div>
              </li>
            </ol>

            <div className="mt-6 flex items-center gap-4 border-t border-border-secondary pt-3">
              <button
                type="button"
                className="cursor-pointer rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
              >
                Dismiss guide
              </button>
              <span className="text-xs text-white/40">Will be accessible again from the user menu</span>
            </div>
          </div>

          <div className="min-w-0 flex-1">
            <div className="p-4">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-medium text-primary">Logs</h3>
                <div className="flex items-center gap-3">
                  <div className="flex items-center gap-1 text-xs text-white/40">
                    Less
                    {levelColors.map((c) => (
                      <span key={c} className={`size-2.5 rounded-[2px] ${c}`} />
                    ))}
                    More
                  </div>
                  <button
                    type="button"
                    className="flex size-6 items-center justify-center rounded-sm border border-border-secondary bg-transparent text-white/60 transition-colors hover:bg-[#202020] hover:text-primary"
                    aria-label="View logs"
                  >
                    <GoChevronRight className="size-3.5" />
                  </button>
                </div>
              </div>
              <p className="mt-0.5 text-xs text-white/40">
                {logs.length} requests · Last 40
              </p>

              <div className="relative mt-4">
                <div className="flex h-12 w-full items-end gap-[2px]">
                  {logs.map((req, i) => (
                    <div
                      key={req.id}
                      onMouseEnter={() => setHovered(req)}
                      onMouseLeave={() => setHovered(null)}
                      className={`flex-1 cursor-pointer rounded-t-[2px] transition-all hover:ring-1 hover:ring-white/40 ${getStatusColor(req.status)}`}
                      style={{ height: `${30 + ((i * 17) % 70)}%` }}
                    />
                  ))}
                </div>

                {hovered && (
                  <div className="pointer-events-none absolute -top-4 right-0 z-20 w-56 translate-y-[-100%] rounded-md border border-border-secondary bg-[#262626] p-3 shadow-2xl after:absolute after:top-full after:right-8 after:-translate-y-px after:border-4 after:border-transparent after:border-t-border-secondary">
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] text-white/50" suppressHydrationWarning>{hovered.date.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit" })} · 9h ago</span>
                    </div>
                    <div className="mt-2 flex items-center gap-2">
                      <span
                        className={`rounded-sm px-1.5 py-0.5 text-[11px] font-medium ${
                          hovered.status === 200 ? "bg-white/10 text-primary" : "bg-white/10 text-yellow-400"
                        }`}
                      >
                        {hovered.status}
                      </span>
                      <span className="text-xs font-medium text-primary">{hovered.model}</span>
                    </div>
                    <div className="mt-3 space-y-1.5 text-[11px]">
                      <div className="flex justify-between">
                        <span className="text-white/50">Duration</span>
                        <span className="text-white/80">{hovered.duration}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-white/50">First token</span>
                        <span className="text-white/80">{hovered.firstToken}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-white/50">Tokens</span>
                        <span className="text-white/80">
                          {hovered.inputTokens} in · {hovered.outputTokens} out
                        </span>
                      </div>
                      <div className="flex justify-between border-t border-border-secondary pt-1.5">
                        <span className="text-white/50">Cost</span>
                        <span className="text-white/80">{hovered.cost}</span>
                      </div>
                    </div>
                  </div>
                )}
              </div>

              <div className="mt-6 border-t border-border-secondary pt-4">
                <div className="flex items-center justify-between">
                  <p className="text-xs font-medium text-primary">Recent requests</p>
                  <span className="text-xs text-white/40">Last 5</span>
                </div>
                <div className="mt-3 space-y-0.5">
                  {recentLogs.map((log) => (
                    <div
                      key={log.id}
                      className="group flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 text-xs transition-colors hover:bg-[#202020]"
                    >
                      <span
                        className={`rounded-sm px-1.5 py-0.5 font-medium ${
                          log.status === 200 ? "bg-white/10 text-primary" : "bg-white/10 text-yellow-400"
                        }`}
                      >
                        {log.status}
                      </span>
                      <span className="text-white/80">{log.model}</span>
                      <span className="hidden text-white/40 lg:inline">{log.tokens.toLocaleString()} tokens</span>
                      <span className="hidden text-white/40 md:inline">{log.duration}</span>
                      <span className="ml-auto text-white/40" suppressHydrationWarning>
                        {log.date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
      {activeSubTab === "My usage" && (
        <div className="flex flex-col gap-4">
          <div className="grid min-w-0 grid-cols-3 gap-4">
            <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
              <p className="text-xs text-white/60">Total cost</p>
              <p className="mt-1 text-2xl font-medium text-primary">
                ${usageRows.reduce((s, r) => s + (r.costUsd ?? 0), 0).toFixed(4)}
              </p>
            </div>
            <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
              <p className="text-xs text-white/60">Total requests</p>
              <p className="mt-1 text-2xl font-medium text-primary">{usageRows.length.toLocaleString()}</p>
            </div>
            <div className="min-w-0 overflow-hidden rounded-md border border-border-secondary bg-[#262626] p-2">
              <p className="text-xs text-white/60">Total tokens</p>
              <p className="mt-1 text-2xl font-medium text-primary">
                {usageRows.reduce((s, r) => s + (r.inputTokens ?? 0) + (r.outputTokens ?? 0), 0).toLocaleString()}
              </p>
            </div>
          </div>
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
                {usageRows.map((r) => (
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
      )}
    </div>
  );
}
