"use client";

import { useMemo } from "react";
import { GoInfo } from "react-icons/go";
import { barY, defineChart } from "@tanstack/charts";
import { Chart as RendererChart } from "@tanstack/charts/react/core";
import { motion } from "@tanstack/charts/motion";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleBand, scaleLinear } from "d3-scale";
import type { UsageRange, UsageRow } from "@/domains/usage/api";

type Bucket = {
  label: string;
  value: number;
  requests: number;
};

function startOfHour(d: Date): Date {
  const out = new Date(d);
  out.setMinutes(0, 0, 0);
  return out;
}

function startOfDay(d: Date): Date {
  const out = new Date(d);
  out.setHours(0, 0, 0, 0);
  return out;
}

function bucketRows(rows: UsageRow[], metric: "cost" | "tokens", range: UsageRange): Bucket[] {
  const now = new Date();

  if (range === "24h") {
    const buckets: Bucket[] = Array.from({ length: 24 }, (_, i) => {
      const hour = (now.getHours() - 23 + i + 48) % 24;
      return { label: `${hour}`, value: 0, requests: 0 };
    });
    const windowStart = new Date(now.getTime() - 23 * 60 * 60 * 1000);
    windowStart.setMinutes(0, 0, 0);

    for (const row of rows) {
      if (!row.createdAt) continue;
      const created = new Date(row.createdAt);
      if (created < windowStart) continue;
      const hourStart = startOfHour(created);
      const idx = Math.floor((hourStart.getTime() - windowStart.getTime()) / (60 * 60 * 1000));
      if (idx < 0 || idx >= 24) continue;
      const bucket = buckets[idx];
      if (!bucket) continue;
      bucket.requests += 1;
      bucket.value +=
        metric === "cost"
          ? (row.costUsd ?? 0)
          : (row.inputTokens ?? 0) + (row.outputTokens ?? 0);
    }
    return buckets;
  }

  const days = range === "7d" ? 7 : 30;
  const dayStart = startOfDay(now);
  const windowStart = new Date(dayStart.getTime() - (days - 1) * 24 * 60 * 60 * 1000);
  const buckets: Bucket[] = Array.from({ length: days }, (_, i) => {
    const d = new Date(windowStart.getTime() + i * 24 * 60 * 60 * 1000);
    const label = `${d.getMonth() + 1}/${d.getDate()}`;
    return { label, value: 0, requests: 0 };
  });

  for (const row of rows) {
    if (!row.createdAt) continue;
    const created = new Date(row.createdAt);
    if (created < windowStart) continue;
    const idx = Math.floor((startOfDay(created).getTime() - windowStart.getTime()) / (24 * 60 * 60 * 1000));
    if (idx < 0 || idx >= days) continue;
    const bucket = buckets[idx];
    if (!bucket) continue;
    bucket.requests += 1;
    bucket.value +=
      metric === "cost"
        ? (row.costUsd ?? 0)
        : (row.inputTokens ?? 0) + (row.outputTokens ?? 0);
  }
  return buckets;
}

export function UsageEmptyState({ className = "" }: { className?: string }) {
  return (
    <div className={`flex flex-col items-center justify-center gap-2 ${className}`}>
      <GoInfo className="size-5 text-white/35" aria-hidden />
      <p className="text-xs text-white/40">No usage available in this range</p>
    </div>
  );
}

export function UsageChart({
  rows,
  metric = "cost",
  range = "24h",
}: {
  rows: UsageRow[];
  metric?: "cost" | "tokens";
  range?: UsageRange;
}) {
  const buckets = useMemo(() => bucketRows(rows, metric, range), [rows, metric, range]);
  const hasData = rows.length > 0;

  const definition = useMemo(() => {
    if (!hasData) return null;
    const chartRows = buckets.map((b) => ({
      hour: b.label,
      value: b.value,
      requests: b.requests,
    }));
    // Dynamic row arrays lose the const-mark tuple that defineChart's overloads expect.
    return (defineChart as any)(
      {
        marks: [
          barY(chartRows, {
            id: "usage-bars",
            x: "hour",
            y: "value",
            fill: "#e5e5e5",
            radius: 2,
          }),
        ],
        scales: {
          x: {
            scale: () => scaleBand<string>().paddingInner(0.4).paddingOuter(0.1),
            axis: false,
          },
          y: { scale: scaleLinear, grid: true, axis: true },
        },
        margin: { top: 5, right: 5, bottom: 20, left: 30 },
      },
      {
        focus: "group-x",
        tooltip: {
          use: tooltip,
          className: "sc-chart-tooltip",
          anchor: "group-center",
          placement: "auto",
          content: (points: any) => {
            const label = String(points[0]?.xValue ?? "");
            const bucket = chartRows.find((b) => b.hour === label);
            return {
              title: range === "24h" ? `${label}:00` : label,
              rows: [
                {
                  label: metric === "cost" ? "Cost" : "Tokens",
                  value:
                    metric === "cost"
                      ? Number(points[0]?.yValue ?? 0).toFixed(4)
                      : Number(points[0]?.yValue ?? 0).toLocaleString(),
                  color: points[0]?.color,
                },
                {
                  label: "Requests",
                  value: String(bucket?.requests ?? 0),
                  color: points[0]?.color,
                },
              ],
            };
          },
        },
      },
    );
  }, [buckets, hasData, metric, range]);

  const renderer = motion({ initial: "always" });
  const width = 750;
  const height = 240;

  if (!hasData || !definition) {
    return (
      <div style={{ width, height }} className="flex items-center justify-center">
        <UsageEmptyState />
      </div>
    );
  }

  return (
    <div style={{ width, height }}>
      <RendererChart
        definition={definition}
        renderer={renderer}
        initialWidth={width}
        height={height}
        ariaLabel="Usage chart"
      />
    </div>
  );
}

export function chartAxisLabels(range: UsageRange): string[] {
  const now = new Date();
  if (range === "24h") {
    return Array.from({ length: 24 }, (_, i) => {
      const hour = (now.getHours() - 23 + i + 48) % 24;
      return i % 5 === 0 ? `${hour}:00` : "";
    });
  }
  const days = range === "7d" ? 7 : 30;
  const dayStart = startOfDay(now);
  const windowStart = new Date(dayStart.getTime() - (days - 1) * 24 * 60 * 60 * 1000);
  const step = range === "7d" ? 1 : 5;
  return Array.from({ length: days }, (_, i) => {
    const d = new Date(windowStart.getTime() + i * 24 * 60 * 60 * 1000);
    return i % step === 0 ? `${d.getMonth() + 1}/${d.getDate()}` : "";
  });
}
