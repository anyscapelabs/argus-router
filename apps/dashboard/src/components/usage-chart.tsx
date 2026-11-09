"use client";

import { useMemo } from "react";
import { GoInfo } from "react-icons/go";
import { barY, defineChart } from "@tanstack/charts";
import { Chart as RendererChart } from "@tanstack/charts/react/core";
import { motion } from "@tanstack/charts/motion";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleBand, scaleLinear } from "d3-scale";
import type { UsageRange, UsageSeriesPoint } from "@/domains/usage/api";

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

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** Viewer-local `YYYY-MM-DDTHH:00` / `YYYY-MM-DDT00:00` key, matching the server's bucket strings. */
function localKey(d: Date, unit: "hour" | "day"): string {
  const date = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return unit === "hour" ? `${date}T${pad(d.getHours())}:00` : `${date}T00:00`;
}

/**
 * Lays the server-computed series (aggregated in SQL over every row in range,
 * in the viewer's timezone) onto a fixed local-time axis, zero-filling gaps.
 */
function bucketSeries(series: UsageSeriesPoint[], metric: "cost" | "tokens", range: UsageRange): Bucket[] {
  const now = new Date();
  const byKey = new Map(series.map((p) => [p.bucket, p]));
  const unit = range === "24h" ? "hour" : "day";
  const count = range === "24h" ? 24 : range === "7d" ? 7 : 30;

  const first = unit === "hour" ? startOfHour(now) : startOfDay(now);
  return Array.from({ length: count }, (_, i) => {
    const d = new Date(first);
    if (unit === "hour") d.setHours(d.getHours() - (count - 1 - i));
    else d.setDate(d.getDate() - (count - 1 - i));
    const point = byKey.get(localKey(d, unit));
    return {
      label: unit === "hour" ? `${d.getHours()}` : `${d.getMonth() + 1}/${d.getDate()}`,
      value: point ? (metric === "cost" ? point.costUsd : point.tokens) : 0,
      requests: point?.requests ?? 0,
    };
  });
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
  series,
  metric = "cost",
  range = "24h",
}: {
  series: UsageSeriesPoint[];
  metric?: "cost" | "tokens";
  range?: UsageRange;
}) {
  const buckets = useMemo(() => bucketSeries(series, metric, range), [series, metric, range]);
  const hasData = series.length > 0;

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
