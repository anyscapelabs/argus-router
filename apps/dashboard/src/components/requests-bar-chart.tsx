"use client";

import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

function themeColor(fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue("--accent")
    .trim();
  return value || fallback;
}

function withAlpha(hex: string, alpha: number): string {
  const compact = hex.replace("#", "");
  if (compact.length === 3) {
    return `rgba(${parseInt(compact[0], 16) * 17}, ${parseInt(compact[1], 16) * 17}, ${parseInt(compact[2], 16) * 17}, ${alpha})`;
  }
  if (compact.length === 6) {
    return `rgba(${parseInt(compact.slice(0, 2), 16)}, ${parseInt(compact.slice(2, 4), 16)}, ${parseInt(compact.slice(4, 6), 16)}, ${alpha})`;
  }
  return hex;
}

export default function RequestsBarChart({
  modelCounts,
}: {
  modelCounts: { model: string; count: number }[];
}) {
  const chartRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);

  useEffect(() => {
    const el = chartRef.current;
    if (!el || modelCounts.length === 0) return;
    const data = [modelCounts.map((_, i) => i), modelCounts.map((m) => m.count)] as [number[], number[]];

    const options = (width: number): uPlot.Options => {
      const accent = themeColor("#e5e5e5");
      return {
        width,
        height: 192,
        scales: {
          y: { range: [0, Math.max(...modelCounts.map((m) => m.count))] },
        },
        series: [
          {},
          {
            label: "Requests",
            stroke: accent,
            fill: withAlpha(accent, 1),
            paths: uPlot.paths.bars!({ size: [0.12], gap: 2 }),
          },
        ],
        axes: [
          {
            stroke: "rgba(255,255,255,0.55)",
            grid: { show: false },
            ticks: { show: false },
            values: (_, vals) => vals.map((v) => modelCounts[v]?.model ?? ""),
          },
          {
            stroke: "rgba(255,255,255,0.55)",
            grid: { stroke: "rgba(255,255,255,0.07)" },
            ticks: { show: false },
          },
        ],
      };
    };

    const settle = () => {
      const width = Math.max(120, Math.floor(el.clientWidth));
      if (plotRef.current) {
        plotRef.current.setSize({ width, height: 192 });
      } else {
        plotRef.current = new uPlot(options(width), data, el);
      }
    };

    settle();
    const observer = new ResizeObserver(settle);
    observer.observe(el);
    return () => {
      observer.disconnect();
      plotRef.current?.destroy();
      plotRef.current = null;
    };
  }, [modelCounts]);

  return <div ref={chartRef} className="mt-4 h-56 w-full" />;
}
