"use client";

import { useEffect, useMemo, useRef, useState } from "react";
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

const HEIGHT = 240;

function formatAxisValue(value: number): string {
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  if (abs >= 1_000_000_000) return `${sign}${trimCompact(abs / 1_000_000_000)}B`;
  if (abs >= 1_000_000) return `${sign}${trimCompact(abs / 1_000_000)}M`;
  if (abs >= 1_000) return `${sign}${trimCompact(abs / 1_000)}K`;
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

function bucketSeries(series: UsageSeriesPoint[], range: UsageRange): Bucket[] {
  const now = new Date();
  const byKey = new Map(series.map((point) => [point.bucket, point]));
  const unit = range === "24h" ? "hour" : "day";
  const count = range === "24h" ? 24 : range === "7d" ? 7 : range === "month" ? now.getDate() : 30;

  const first = unit === "hour" ? startOfHour(now) : startOfDay(now);
  return Array.from({ length: count }, (_, index) => {
    const date = new Date(first);
    if (unit === "hour") date.setHours(date.getHours() - (count - 1 - index));
    else date.setDate(date.getDate() - (count - 1 - index));
    const point = byKey.get(localKey(date, unit));
    return {
      label: unit === "hour" ? `${date.getHours()}` : `${date.getMonth() + 1}/${date.getDate()}`,
      value: point?.tokens ?? 0,
      requests: point?.requests ?? 0,
    };
  });
}

export function UsageChart({
  series,
  range = "24h",
  isLoading = false,
}: {
  series: UsageSeriesPoint[];
  range?: UsageRange;
  isLoading?: boolean;
}) {
  const container = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(750);
  const buckets = useMemo(() => bucketSeries(series, range), [series, range]);
  const hasData = series.length > 0;

  useEffect(() => {
    const node = container.current;
    if (!node) return;
    const observer = new ResizeObserver((entries) => {
      const next = Math.floor(entries[0]?.contentRect.width ?? 0);
      if (next > 0) setWidth(next);
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const definition = useMemo(() => {
    if (!hasData) return null;
    const chartRows = buckets.map((bucket) => ({
      hour: bucket.label,
      value: bucket.value,
      requests: bucket.requests,
    }));
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (defineChart as any)(
      {
        marks: [
          barY(chartRows, {
            id: "usage-bars",
            x: "hour",
            y: "value",
            fill: "var(--accent)",
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
                format: (value: number) => formatAxisValue(value),
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
            const bucket = chartRows.find((row) => row.hour === label);
            return {
              title: range === "24h" ? `${label}:00` : label,
              rows: [
                {
                  label: "Tokens",
                  value: Number(points[0]?.yValue ?? 0).toLocaleString(),
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
  }, [buckets, hasData, range]);

  const renderer = motion({ initial: "always" });

  if (isLoading) {
    return <div ref={container} style={{ height: HEIGHT }} className="w-full" />;
  }

  if (!hasData || !definition) {
    return (
      <div ref={container} style={{ height: HEIGHT }} className="flex w-full items-center justify-center">
        <UsageEmptyState />
      </div>
    );
  }

  return (
    <div ref={container} style={{ height: HEIGHT }} className="w-full">
      <RendererChart
        definition={definition}
        renderer={renderer}
        width={width}
        height={HEIGHT}
        ariaLabel="Usage chart"
      />
    </div>
  );
}
