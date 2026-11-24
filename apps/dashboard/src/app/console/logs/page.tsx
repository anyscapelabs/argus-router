"use client";

import { useSearchParams } from "next/navigation";
import {
  Suspense,
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { HiMiniArrowPath } from "react-icons/hi2";
import {
  LogsDatePicker,
  getPresetRange,
  type LogsDateRange,
} from "@/components/logs-date-picker";
import { PageLoading } from "@/components/page-loading";
import { Skeleton } from "@/components/skeleton";
import { useApiKeys } from "@/domains/keys/hooks";
import { useUsage } from "@/domains/usage/hooks";
import type { UsageRow } from "@/domains/usage/api";

const TABS = ["Chat", "Voice", "Web Search"] as const;
type Tab = (typeof TABS)[number];

const KeyNamesContext = createContext<Map<string, string>>(new Map());

function KeyName({ keyId }: { keyId: string | null }) {
  const names = useContext(KeyNamesContext);
  if (!keyId) return <>-</>;
  return <span>{names.get(keyId) ?? keyId}</span>;
}

type LogColumn = {
  label: string;
  className?: string;
  render: (row: UsageRow) => ReactNode;
};

const CHAT_COLUMNS: LogColumn[] = [
  { label: "Time", render: (row) => <span suppressHydrationWarning>{formatTime(row.createdAt)}</span> },
  { label: "Status", render: (row) => <StatusPill row={row} /> },
  { label: "Model", render: (row) => row.model ?? "-" },
  { label: "API Key", render: (row) => <KeyName keyId={row.keyId} /> },
  { label: "Duration", render: (row) => formatLatency(row.latencyMs) },
  { label: "Input tokens", className: "text-right", render: (row) => formatNumber(row.inputTokens) },
  { label: "Output tokens", className: "text-right", render: (row) => formatNumber(row.outputTokens) },
];

const VOICE_COLUMNS: LogColumn[] = [
  { label: "Time", render: (row) => <span suppressHydrationWarning>{formatTime(row.createdAt)}</span> },
  { label: "Status", render: (row) => <StatusPill row={row} /> },
  { label: "Model", render: (row) => row.model ?? "-" },
  { label: "API Key", render: (row) => <KeyName keyId={row.keyId} /> },
  { label: "Duration", render: (row) => formatLatency(row.latencyMs) },
  { label: "Turns", render: (row) => row.details?.turns ?? "—" },
];

const SEARCH_COLUMNS: LogColumn[] = [
  { label: "Time", render: (row) => <span suppressHydrationWarning>{formatTime(row.createdAt)}</span> },
  { label: "Status", render: (row) => <StatusPill row={row} /> },
  {
    label: "Query",
    render: (row) =>
      row.details?.query ? (
        <span title={row.details.query}>{row.details.query}</span>
      ) : (
        "—"
      ),
  },
  { label: "Results", render: (row) => row.details?.resultCount ?? "—" },
  { label: "Duration", render: (row) => formatLatency(row.latencyMs) },
];

const TABLES: Record<Tab, { grid: string; columns: LogColumn[] }> = {
  Chat: {
    grid: "grid-cols-[1.2fr_0.6fr_1.2fr_1.4fr_0.8fr_1fr_1fr]",
    columns: CHAT_COLUMNS,
  },
  Voice: {
    grid: "grid-cols-[1.2fr_1fr_1.2fr_1.2fr_0.8fr_0.6fr]",
    columns: VOICE_COLUMNS,
  },
  "Web Search": {
    grid: "grid-cols-[1.2fr_1fr_2fr_0.7fr_0.8fr]",
    columns: SEARCH_COLUMNS,
  },
};

function formatTime(value: string | null) {
  if (!value) return "—";
  return new Date(value).toLocaleString("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function formatLatency(value: number | null) {
  return value != null ? `${(value / 1000).toFixed(1)}s` : "—";
}

function formatNumber(value: number | null) {
  return (value ?? 0).toLocaleString();
}

function StatusPill({ row }: { row: UsageRow }) {
  const failed = row.status !== 200;
  return (
    <span
      title={failed ? row.errorCode ?? undefined : undefined}
      className={`inline-flex rounded-full bg-primary/10 px-2 py-0.5 text-xs ${
        failed ? "text-warning" : "text-primary/70"
      }`}
    >
      {failed ? row.errorCode ?? row.status ?? "-" : row.status}
    </span>
  );
}

function LogsTable({
  tab,
  rows,
  isLoading,
  errorMessage,
}: {
  tab: Tab;
  rows: UsageRow[];
  isLoading: boolean;
  errorMessage: string | null;
}) {
  const { grid, columns } = TABLES[tab];

  return (
    <div>
      <div className={`grid ${grid} gap-4 border-b border-border p-1 text-xs text-primary/60`}>
        {columns.map((column) => (
          <span key={column.label} className={column.className}>
            {column.label}
          </span>
        ))}
      </div>
      {isLoading &&
        Array.from({ length: 5 }, (_, row) => (
          <div key={row} className={`grid ${grid} gap-4 border-b border-border/50 p-1 py-3`}>
            {columns.map((column) => (
              <Skeleton key={column.label} className="rounded-full px-2 py-1" />
            ))}
          </div>
        ))}
      {errorMessage && <p className="mt-4 text-sm text-danger">{errorMessage}</p>}
      {!isLoading && !errorMessage && rows.length === 0 && (
        <p className="mt-6 text-center text-sm text-primary/60">No logs yet.</p>
      )}
      {rows.map((row) => (
        <div
          key={row.id}
          className={`grid ${grid} gap-4 border-b border-border/50 p-1 py-3 text-sm transition-colors duration-200 hover:bg-tertiary`}
        >
          {columns.map((column) => (
            <span key={column.label} className={`min-w-0 truncate ${column.className ?? ""}`}>
              {column.render(row)}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

function LogsPageContent() {
  const keyId = useSearchParams().get("keyId") ?? undefined;
  const [tab, setTab] = useState<Tab>("Chat");
  const [refreshing, setRefreshing] = useState(false);
  const [dateRange, setDateRange] = useState<LogsDateRange>(() => getPresetRange("Last 7 days"));
  const since = dateRange.start.toISOString();
  const until = dateRange.end
    ? new Date(
        dateRange.end.getFullYear(),
        dateRange.end.getMonth(),
        dateRange.end.getDate(),
        23,
        59,
        59,
        999,
      ).toISOString()
    : undefined;
  const keys = useApiKeys();
  const keyNames = useMemo(
    () =>
      new Map(
        (keys.data?.apiKeys ?? []).map((key) => [key.id, key.name ?? "Untitled"] as const),
      ),
    [keys.data],
  );
  const common = { keyId, since, until, limit: 50 };
  const chat = useUsage({ ...common, logType: "chat", enabled: tab === "Chat" });
  const voice = useUsage({ ...common, logType: "voice", enabled: tab === "Voice" });
  const search = useUsage({ ...common, logType: "web_search", enabled: tab === "Web Search" });
  const active = tab === "Chat" ? chat : tab === "Voice" ? voice : search;

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
      <div>
        <h1 className="text-2xl font-medium text-primary">Logs</h1>
        <p className="mt-0.5 text-sm text-primary/60">Recent requests through the gateway.</p>
      </div>
      <div className="flex items-center justify-between gap-2">
        <div className="flex gap-2">
          {TABS.map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={`cursor-pointer rounded-full px-4 py-1.5 text-sm transition-colors ${
                tab === t
                  ? "bg-accent text-background"
                  : "bg-tertiary text-primary/60 hover:text-primary"
              }`}
            >
              {t}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <LogsDatePicker range={dateRange} onRangeChange={setDateRange} />
          <button
            type="button"
            aria-label="Refresh logs"
            disabled={refreshing}
            onClick={async () => {
              setRefreshing(true);
              try {
                await active.refetch();
              } finally {
                setRefreshing(false);
              }
            }}
            className="inline-flex cursor-pointer items-center justify-center rounded-full bg-tertiary p-1.5 text-primary transition-colors hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <HiMiniArrowPath className={`size-4 ${refreshing ? "animate-spin" : ""}`} aria-hidden="true" />
          </button>
        </div>
      </div>
      <KeyNamesContext.Provider value={keyNames}>
        <LogsTable
          tab={tab}
          rows={active.data?.rows ?? []}
          isLoading={active.isPending}
          errorMessage={active.isError ? active.error.message : null}
        />
      </KeyNamesContext.Provider>
    </div>
  );
}

export default function LogsPage() {
  return (
    <Suspense fallback={<PageLoading />}>
      <LogsPageContent />
    </Suspense>
  );
}
