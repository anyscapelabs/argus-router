"use client";

import { Suspense, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { GoChevronDown, GoDownload, GoChevronRight } from "react-icons/go";
import { useUsage } from "@/domains/usage/hooks";
import type { UsageRow } from "@/domains/usage/api";
import { UsageEmptyState } from "@/components/usage-empty-state";
import { Skeleton } from "@/components/skeleton";
import { formatTokenCount, modelDisplayName } from "@/lib/usage-format";

const platforms = [
  { label: "macOS", detail: "Apple Silicon" },
  { label: "macOS", detail: "Intel" },
  { label: "Windows", detail: ".exe" },
  { label: "Windows", detail: ".msi" },
  { label: "Linux", detail: ".deb" },
  { label: "Linux", detail: ".rpm" },
  { label: "Linux", detail: ".AppImage" },
];

type RequestLog = {
  id: string;
  date: Date;
  model: string;
  status: number;
  tokens: number;
  duration: string;
  inputTokens: number;
  outputTokens: number;
  cost: string;
};

function toActivityLog(row: UsageRow): RequestLog {
  const input = row.inputTokens ?? 0;
  const output = row.outputTokens ?? 0;
  return {
    id: String(row.id),
    date: row.createdAt ? new Date(row.createdAt) : new Date(0),
    model: row.model ?? "unknown",
    status: row.status ?? 0,
    tokens: input + output,
    duration: row.latencyMs != null ? `${(row.latencyMs / 1000).toFixed(1)}s` : "-",
    inputTokens: input,
    outputTokens: output,
    cost: `$${(row.costUsd ?? 0).toFixed(4)}`,
  };
}

function getStatusColor(status: number): string {
  return status >= 400 || status === 0 ? "bg-red-500/60" : "bg-white/25";
}

const levelColors = ["bg-[#1a1a1a]"];

const downloadUrl = process.env.NEXT_PUBLIC_ARGUS_DOWNLOAD_URL;
const GUIDE_DISMISSED_KEY = "argus.guide.dismissed";

export default function OverviewPage() {
  const [os, setOs] = useState("Unknown");
  const [open, setOpen] = useState(false);
  const [guideDismissed, setGuideDismissed] = useState(false);
  const [hovered, setHovered] = useState<{ log: RequestLog; index: number } | null>(null);

  const activityUsage = useUsage({ limit: 40 });
  const activityRows = activityUsage.data?.rows ?? [];
  const activityLogs = useMemo(() => {
    return activityRows.slice().reverse().map(toActivityLog);
  }, [activityRows]);

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      try {
        setGuideDismissed(window.localStorage.getItem(GUIDE_DISMISSED_KEY) === "1");
      } catch {
      }
      const platform = window.navigator.platform.toLowerCase();
      if (platform.includes("mac")) setOs("macOS");
      else if (platform.includes("win")) setOs("Windows");
      else if (platform.includes("linux")) setOs("Linux");
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  const dismissGuide = (dismissed: boolean) => {
    setGuideDismissed(dismissed);
    try {
      if (dismissed) window.localStorage.setItem(GUIDE_DISMISSED_KEY, "1");
      else window.localStorage.removeItem(GUIDE_DISMISSED_KEY);
    } catch {
    }
  };

  const sparkLogs = activityLogs.slice(-40);
  const recentLogs = activityLogs.slice(-5);

  return (
    <Suspense
      fallback={
        <div className="flex flex-1 flex-col gap-2 p-10">
          <Skeleton className="h-3 w-40" />
          <Skeleton className="h-3 w-64" />
          <Skeleton className="h-3 w-52" />
        </div>
      }
    >
      <div className="flex min-h-full flex-1 flex-col px-2">
        <div className="flex flex-col gap-6 xl:flex-row">
          <div className="w-full min-w-0 max-w-4xl rounded-md border border-border-secondary bg-[#262626] p-4">
            {guideDismissed ? (
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-xs text-white/60">
                  Setup guide hidden — it walks you through pairing Argus Agent with this console.
                </p>
                <button
                  type="button"
                  onClick={() => dismissGuide(false)}
                  className="cursor-pointer rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs text-primary transition-colors hover:bg-white/10"
                >
                  Show guide
                </button>
              </div>
            ) : (
              <>
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h2 className="text-sm font-medium text-primary">Connect Argus Agent</h2>
                    <p className="mt-1 text-xs text-white/60">
                      Once connected you can access everything available in Console. Not installed yet?{" "}
                      {downloadUrl ? (
                        <a
                          href={downloadUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-white/80 underline underline-offset-2 hover:text-primary"
                        >
                          Download Argus Agent
                        </a>
                      ) : (
                        <span className="text-white/80">Download Argus Agent</span>
                      )}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => dismissGuide(true)}
                    aria-label="Hide setup guide"
                    className="shrink-0 cursor-pointer text-xs text-white/40 hover:text-primary"
                  >
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
                        {downloadUrl ? (
                          <>
                            <a
                              href={downloadUrl}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="inline-flex items-center gap-1.5 rounded-l-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                            >
                              <GoDownload className="size-3" />
                              Download for {os}
                            </a>
                            <div className="h-5 w-px bg-black/20" />
                            <button
                              type="button"
                              onClick={() => setOpen((v) => !v)}
                              aria-expanded={open}
                              aria-haspopup="menu"
                              className="inline-flex cursor-pointer items-center gap-1 rounded-r-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                            >
                              Other platforms
                              <GoChevronDown className={`size-3 transition-transform ${open ? "rotate-180" : ""}`} />
                            </button>
                            {open && (
                              <div className="absolute top-full right-0 z-10 mt-1 w-40 overflow-hidden rounded-md border border-border-secondary bg-[#262626] py-1 shadow-lg">
                                {platforms.map((p, i) => (
                                  <a
                                    key={`${p.label}-${p.detail}-${i}`}
                                    href={downloadUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    onClick={() => setOpen(false)}
                                    className="mx-1 flex w-[calc(100%-0.5rem)] items-center justify-between gap-2 rounded-md px-2 py-1 text-left text-xs text-white/80 transition-colors hover:bg-[#202020] hover:text-primary"
                                  >
                                    <span>{p.label}</span>
                                    <span className="text-white/40">{p.detail}</span>
                                  </a>
                                ))}
                              </div>
                            )}
                          </>
                        ) : (
                          <Link
                            href="/console/settings/service-accounts"
                            className="inline-flex items-center gap-1.5 rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                          >
                            Create your pairing key
                          </Link>
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
                        In the app, choose <span className="text-white/70">Connect</span> and paste a service-account key.
                        Create one from{" "}
                        <Link
                          href="/console/settings/service-accounts"
                          className="text-white/70 underline underline-offset-2 hover:text-primary"
                        >
                          Settings → Service Accounts
                        </Link>
                        .
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
                    onClick={() => dismissGuide(true)}
                    className="cursor-pointer rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
                  >
                    Dismiss guide
                  </button>
                  <span className="text-xs text-white/40">You can show it again from this page at any time.</span>
                </div>
              </>
            )}
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
                  <Link
                    href="/console/logs"
                    className="flex size-6 items-center justify-center rounded-sm border border-border-secondary bg-transparent text-white/60 transition-colors hover:bg-[#202020] hover:text-primary"
                    aria-label="View logs"
                  >
                    <GoChevronRight className="size-3.5" />
                  </Link>
                </div>
              </div>
              <p className="mt-0.5 text-xs text-white/40">
                {sparkLogs.length === 0
                  ? "No requests yet"
                  : `${sparkLogs.length} requests · Last ${sparkLogs.length}`}
              </p>

              <div className="relative mt-4">
                {sparkLogs.length === 0 ? (
                  <UsageEmptyState className="h-24" />
                ) : (
                  <div className="flex h-12 w-full items-end gap-[2px]">
                    {(() => {
                      const maxTokens = Math.max(1, ...sparkLogs.map((l) => l.tokens));
                      return sparkLogs.map((req, index) => {
                        const heightPct = 18 + Math.round((req.tokens / maxTokens) * 82);
                        return (
                          <button
                            key={req.id}
                            type="button"
                            onMouseEnter={() => setHovered({ log: req, index })}
                            onMouseLeave={() => setHovered(null)}
                            onFocus={() => setHovered({ log: req, index })}
                            onBlur={() => setHovered(null)}
                            aria-label={`${modelDisplayName(req.model)} — ${req.tokens} tokens, status ${req.status}`}
                            className={`w-2 shrink-0 cursor-pointer rounded-t-[2px] transition-all hover:ring-1 hover:ring-white/40 focus:outline-none focus:ring-1 focus:ring-white/60 ${getStatusColor(req.status)}`}
                            style={{ height: `${heightPct}%` }}
                          />
                        );
                      });
                    })()}
                  </div>
                )}

                {hovered && (
                  <div
                    className="pointer-events-none absolute top-[52px] z-50 w-56 -translate-x-1/2 rounded-md border border-border-secondary bg-[#262626] p-3 shadow-2xl after:absolute after:bottom-full after:left-1/2 after:-translate-x-1/2 after:translate-y-px after:border-4 after:border-transparent after:border-b-border-secondary"
                    style={{ left: `clamp(7rem, ${hovered.index * 10 + 4}px, calc(100% - 7rem))` }}
                  >
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] text-white/50" suppressHydrationWarning>
                        {hovered.log.date.toLocaleTimeString("en-US", {
                          hour: "2-digit",
                          minute: "2-digit",
                          second: "2-digit",
                        })}
                      </span>
                    </div>
                    <div className="mt-2 flex items-center gap-2">
                      <span
                        className={`rounded-sm px-1.5 py-0.5 text-[11px] font-medium ${
                          hovered.log.status === 200 ? "bg-white/10 text-primary" : "bg-white/10 text-yellow-400"
                        }`}
                      >
                        {hovered.log.status}
                      </span>
                      <span className="text-xs font-medium text-primary">{modelDisplayName(hovered.log.model)}</span>
                    </div>
                    <div className="mt-3 space-y-1.5 text-[11px]">
                      <div className="flex justify-between">
                        <span className="text-white/50">Duration</span>
                        <span className="text-white/80">{hovered.log.duration}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-white/50">Tokens</span>
                        <span className="text-white/80">
                          {formatTokenCount(hovered.log.inputTokens)} in · {formatTokenCount(hovered.log.outputTokens)} out
                        </span>
                      </div>
                      <div className="flex justify-between border-t border-border-secondary pt-1.5">
                        <span className="text-white/50">Cost</span>
                        <span className="text-white/80">{hovered.log.cost}</span>
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
                <div className="mt-3 min-h-[140px]">
                  {recentLogs.length === 0 ? (
                    <UsageEmptyState className="h-[140px]" />
                  ) : (
                    <div className="space-y-0.5">
                      {recentLogs.map((log) => (
                        <div
                          key={log.id}
                          className="group grid cursor-pointer grid-cols-[2.75rem_minmax(0,1fr)_4.5rem_3.5rem_4.5rem] items-center gap-x-3 rounded-md px-2 py-1.5 text-xs transition-colors hover:bg-[#202020]"
                        >
                          <span
                            className={`inline-flex w-fit rounded-sm px-1.5 py-0.5 font-medium tabular-nums ${
                              log.status === 200 ? "bg-white/10 text-primary" : "bg-white/10 text-yellow-400"
                            }`}
                          >
                            {log.status}
                          </span>
                          <span className="truncate text-white/80" title={modelDisplayName(log.model)}>
                            {modelDisplayName(log.model)}
                          </span>
                          <span className="text-right tabular-nums text-white/40">
                            {formatTokenCount(log.tokens)}
                          </span>
                          <span className="text-right tabular-nums text-white/40">{log.duration}</span>
                          <span className="text-right tabular-nums text-white/40" suppressHydrationWarning>
                            {log.date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Suspense>
  );
}
