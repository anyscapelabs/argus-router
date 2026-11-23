"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { chartAxisLabels } from "@/components/usage-empty-state";
import type { UsageRange } from "@/domains/usage/api";
import { useUsageSummary } from "@/domains/usage/hooks";

const UsageChart = dynamic(
  () => import("@/components/usage-chart").then((module) => module.UsageChart),
  { ssr: false },
);

const PERIODS = ["Hour", "Day", "Week", "Month"] as const;
type Period = (typeof PERIODS)[number];

const PERIOD_RANGES: Record<Period, UsageRange> = {
  Hour: "24h",
  Day: "7d",
  Week: "30d",
  Month: "month",
};

export function UsageChartCard() {
  const [period, setPeriod] = useState<Period>("Day");
  const [axisLabels, setAxisLabels] = useState<string[]>([]);
  const range = PERIOD_RANGES[period];
  const summary = useUsageSummary({ range });
  const series = summary.data?.series ?? [];

  useEffect(() => {
    const frame = requestAnimationFrame(() => setAxisLabels(chartAxisLabels(range)));
    return () => cancelAnimationFrame(frame);
  }, [range]);

  return (
    <div className="w-full rounded-xl bg-secondary p-4">
      <div className="flex items-center justify-between gap-4">
        <h2 className="text-lg font-medium text-primary">
          Tokens per {period.toLowerCase()}
        </h2>
        <div className="flex items-center gap-1 rounded-full bg-tertiary p-1">
          {PERIODS.map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setPeriod(item)}
              className={`cursor-pointer rounded-full px-3 py-1 text-xs transition-colors ${
                period === item
                  ? "bg-accent text-background"
                  : "text-primary/60 hover:text-primary"
              }`}
            >
              {item}
            </button>
          ))}
        </div>
      </div>
      <div className="mt-4 overflow-x-auto">
        <UsageChart series={series} range={range} isLoading={summary.isPending} />
        {series.length > 0 && (
          <div className="ml-[44px] -mt-2 flex w-[calc(100%-44px)] text-[10px] text-primary/40">
            {axisLabels.map((label, index) => (
              <span key={index} className="flex-1 text-center">
                {label}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
