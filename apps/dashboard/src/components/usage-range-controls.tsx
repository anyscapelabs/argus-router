"use client";

import { GoClock, GoSync } from "react-icons/go";
import type { UsageRange } from "@/domains/usage/api";

export function RangeControls({
  range,
  onRangeChange,
  onRefresh,
  isFetching,
}: {
  range: UsageRange;
  onRangeChange: (next: UsageRange) => void;
  onRefresh: () => void;
  isFetching: boolean;
}) {
  return (
    <div className="flex gap-1">
      <button
        type="button"
        onClick={() =>
          onRangeChange(range === "24h" ? "7d" : range === "7d" ? "30d" : "24h")
        }
        className="flex cursor-pointer items-center gap-1 rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs text-primary transition-colors hover:bg-white/10"
      >
        <GoClock className="size-3" />
        {range}
      </button>
      <button
        type="button"
        onClick={onRefresh}
        disabled={isFetching}
        className={`flex cursor-pointer items-center gap-1 rounded-sm border border-border-secondary px-2 py-1 text-xs text-primary transition-colors ${
          isFetching ? "bg-white/10" : "bg-tertiary hover:bg-white/10"
        }`}
      >
        <GoSync className={`size-3 ${isFetching ? "animate-spin" : ""}`} />
        Refresh
      </button>
    </div>
  );
}
