"use client";

import { useMemo } from "react";
import { barY, defineChart } from "@tanstack/charts";
import { Chart as RendererChart } from "@tanstack/charts/react/core";
import { motion } from "@tanstack/charts/motion";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleBand, scaleLinear } from "d3-scale";
import type { UsageRange, UsageSeriesPoint } from "@/domains/usage/api";
import { UsageEmptyState } from "./usage-empty-state";

type Bucket = {
  label: string;
  value: number;
  requests: number;
};

function formatAxisValue(value: number, metric: "cost" | "tokens"): string {
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  if (abs >= 1_000_000_000) return `${sign}${trimCompact(abs / 1_000_000_000)}B`;
  if (abs >= 1_000_000) return `${sign}${trimCompact(abs / 1_000_000)}M`;
  if (abs >= 1_000) return `${sign}${trimCompact(abs / 1_000)}K`;
  if (metric === "cost") {
    if (abs === 0) return "0";
    if (abs < 0.01) return value.toFixed(4).replace(/\.?0+$/, "");
    return value.toFixed(2).replace(/\.?0+$/, "");
  }
  if (Number.isInteger(value)) return String(value);
  return trimCompact(abs);
}

function trimCompact(n: number): string {
  return n.toFixed(1).replace(/\.0$/, "");
}

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

function localKey(d: Date, unit: "hour" | "day"): string {
  const date = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return unit === "hour" ? `${date}T${pad(d.getHours())}:00` : `${date}T00:00`;
}

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
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
          y: {
            scale: scaleLinear,
            grid: true,
            axis: {
              ticks: {
                format: (value: number) => formatAxisValue(value, metric),
              },
            },
          },
        },
        margin: { top: 5, right: 5, bottom: 20, left: 44 },
      },
      {
        focus: "group-x",
        tooltip: {
          use: tooltip,
          className: "sc-chart-tooltip",
          anchor: "group-center",
          placement: "auto",
          content: (points: ReadonlyArray<{ xValue?: unknown; yValue?: unknown; color?: string }>) => {
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
