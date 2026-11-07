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
      x: {
        scale: () => scaleBand<string>().paddingInner(0).paddingOuter(0),
        axis: false,
      },
      y: { scale: scaleLinear, grid: true, axis: true },
    },
    margin: { top: 5, right: 5, bottom: 35, left: 30 },
  },
  {},
);

const renderer = motion({ initial: "always" });

export function UsageChart() {
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