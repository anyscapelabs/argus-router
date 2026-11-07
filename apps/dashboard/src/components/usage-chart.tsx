"use client";

import { useMemo } from "react";
import { barY, defineChart } from "@tanstack/charts";
import { Chart as RendererChart } from "@tanstack/charts/react/core";
import { motion } from "@tanstack/charts/motion";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleBand, scaleLinear } from "d3-scale";

const rows = Array.from({ length: 24 }, (_, i) => ({
  hour: `${i}`,
  value: Math.abs(Math.sin(i * 0.5) * 10 + 2),
  requests: Math.floor(Math.abs(Math.sin(i * 0.5) * 5 + 1)),
}));

export function UsageChart({ metric = "cost" }: { metric?: "cost" | "tokens" }) {
  const definition = useMemo(() => {
    const marks = [
      barY(rows, {
        id: "usage-bars",
        x: "hour",
        y: "value",
        fill: "#e5e5e5",
        radius: 2,
      }),
    ];
    return defineChart(
      {
        marks,
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
          content: (points: any) => ({
            title: `${new Date().toLocaleDateString()} ${points[0]?.xValue ?? ""}:00`,
            rows: [
              {
                label: metric === "cost" ? "Cost" : "Tokens",
                value: Number(points[0]?.yValue ?? 0).toFixed(2),
                color: points[0]?.color,
              },
              {
                label: "Requests",
                value: String(rows[Number(points[0]?.xValue ?? 0)]?.requests ?? 0),
                color: points[0]?.color,
              },
            ],
          }),
        },
      },
    );
  }, [metric]);

  const renderer = motion({ initial: "always" });
  const width = 750;
  const height = 240;
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