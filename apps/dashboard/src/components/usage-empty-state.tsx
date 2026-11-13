"use client";

import { EmptyState } from "@/components/empty-state";
import type { UsageRange } from "@/domains/usage/api";

export function UsageEmptyState({ className = "" }: { className?: string }) {
  return <EmptyState message="No usage available in this range" className={className} />;
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
  const step = range === "7d" ? 1 : 5;
  const dayStart = new Date(now);
  dayStart.setHours(0, 0, 0, 0);
  const windowStart = new Date(dayStart.getTime() - (days - 1) * 24 * 60 * 60 * 1000);
  return Array.from({ length: days }, (_, i) => {
    const d = new Date(windowStart.getTime() + i * 24 * 60 * 60 * 1000);
    return i % step === 0 ? `${d.getMonth() + 1}/${d.getDate()}` : "";
  });
}
