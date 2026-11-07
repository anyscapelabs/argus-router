"use client";

import { barY, defineChart } from "@tanstack/charts";
import { Chart as RendererChart } from "@tanstack/charts/react/core";
import { motion } from "@tanstack/charts/motion";
import { scaleBand, scaleLinear } from "d3-scale";

const rows = Array.from({ length: 24 }, (_, i) => ({
  hour: `${i}`,
  value: Math.abs(Math.sin(i * 0.5) * 10 + 2),
}));

const marks = [
  barY(rows, {
    id: "usage-bars",
    x: "hour",
    y: "value",
    fill: "#e5e5e5",
    radius: 2,
  }),
];

const definition = defineChart(
  {
    marks,
    scales: {
      x: { scale: () => scaleBand<string>().paddingInner(0.2).paddingOuter(0.1) },
      y: { scale: scaleLinear, grid: false, axis: false },
    },
    margin: { top: 4, right: 4, bottom: 4, left: 4 },
  },
  {},
);

const renderer = motion({ initial: "always" });

export function UsageChart() {
  const width = 640;
  const height = 224;
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