"use client";

import { useState } from "react";
import { Skeleton } from "@/components/skeleton";
import { useUsage } from "@/domains/usage/hooks";
import type { UsageRow } from "@/domains/usage/api";

const GRID = "grid-cols-[1.4fr_1.4fr_0.8fr_0.8fr]";
const PAGE_SIZE = 10;

function formatTokens(value: number | null) {
  return (value ?? 0).toLocaleString();
}

function formatDate(value: string | null) {
  if (!value) return "—";
  return new Date(value).toLocaleString("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function EventRow({ row }: { row: UsageRow }) {
  return (
    <div
      className={`grid ${GRID} gap-4 border-b border-border/50 p-1 py-3 text-sm transition-colors duration-200 hover:bg-tertiary`}
    >
      <span className="truncate text-primary/70" suppressHydrationWarning>
        {formatDate(row.createdAt)}
      </span>
      <span className="truncate text-primary">Argus Agent</span>
      <span className="truncate text-primary/60">{row.planId ?? "—"}</span>
      <span className="text-right tabular-nums text-primary/70">
        {formatTokens((row.inputTokens ?? 0) + (row.outputTokens ?? 0))}
      </span>
    </div>
  );
}

export function UsageEvents() {
  const [page, setPage] = useState(1);
  const usage = useUsage({ limit: 50 });
  const rows = usage.data?.rows ?? [];
  const pageCount = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const pageRows = rows.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  return (
    <div className="min-h-96 w-full pb-10">
      <h2 className="text-lg font-medium text-primary">Usage Events</h2>
      <div className={`mt-4 grid ${GRID} gap-4 border-b border-border p-1 text-xs text-primary/60`}>
        <span>Date</span>
        <span>Product</span>
        <span>Plan</span>
        <span className="text-right">Tokens</span>
      </div>
      {usage.isPending &&
        Array.from({ length: 5 }, (_, row) => (
          <div key={row} className={`grid ${GRID} gap-4 border-b border-border/50 p-1 py-3`}>
            {Array.from({ length: 4 }, (_, col) => (
              <Skeleton key={col} className="rounded-full px-2 py-1" />
            ))}
          </div>
        ))}
      {pageRows.map((row) => (
        <EventRow key={row.id} row={row} />
      ))}
      {usage.isSuccess && rows.length === 0 && (
        <p className="mt-24 text-center text-sm text-primary/60">No usage events yet.</p>
      )}
      {rows.length > 0 && (
        <div className="mt-4 flex items-center justify-between text-xs text-primary/60">
          <span>
            Showing {(currentPage - 1) * PAGE_SIZE + 1}–
            {Math.min(currentPage * PAGE_SIZE, rows.length)} of {rows.length}
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setPage((value) => Math.max(1, value - 1))}
              disabled={currentPage <= 1}
              className="cursor-pointer rounded-full border border-border-secondary bg-tertiary px-3 py-1 text-xs text-primary transition-colors hover:bg-primary/10 disabled:cursor-default disabled:opacity-40"
            >
              Previous
            </button>
            <span className="tabular-nums">
              Page {currentPage} of {pageCount}
            </span>
            <button
              type="button"
              onClick={() => setPage((value) => Math.min(pageCount, value + 1))}
              disabled={currentPage >= pageCount}
              className="cursor-pointer rounded-full border border-border-secondary bg-tertiary px-3 py-1 text-xs text-primary transition-colors hover:bg-primary/10 disabled:cursor-default disabled:opacity-40"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
