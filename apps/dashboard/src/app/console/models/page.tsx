"use client";

import { useState } from "react";
import { GoKebabHorizontal, GoSync, GoSearch } from "react-icons/go";
import { CiImageOn } from "react-icons/ci";
import { RxText } from "react-icons/rx";
import { IoTerminalOutline } from "react-icons/io5";
import { LuBrain } from "react-icons/lu";
import { useQueryClient } from "@tanstack/react-query";

import {
  useModels,
  useRefreshModels,
  useToggleModel,
} from "@/domains/models/hooks";
import type { ModelView } from "@/domains/models/api";

const filters = ["All", "Enabled", "Free"] as const;

function price(value: string | null): string {
  if (value === null || value === undefined) return "-";
  const amount = Number(value);
  if (Number.isNaN(amount)) return "-";
  return `$${amount.toFixed(2)}`;
}

function relativeTime(timestamp: number | undefined): string {
  if (!timestamp) return "-";
  const seconds = Math.round((Date.now() - timestamp) / 1000);
  if (seconds < 10) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h ago`;
  return `${Math.round(seconds / 86400)}d ago`;
}

function Toggle({
  on,
  busy,
  disabled,
  label,
  onClick,
}: {
  on: boolean;
  busy?: boolean;
  disabled?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={busy || disabled}
      className={`relative inline-block h-5 w-9 rounded-full transition-colors ${
        disabled ? "cursor-not-allowed opacity-40" : "cursor-pointer"
      } ${on ? "bg-[#3b82f6]" : "bg-white/20"} ${busy ? "opacity-50" : ""}`}
      aria-label={label}
    >
      <span
        className={`absolute top-0.5 left-0.5 block size-4 rounded-full bg-white transition-transform ${
          on ? "translate-x-4" : ""
        }`}
      />
    </button>
  );
}

function FeatureChip({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <span
      className="flex size-5 items-center justify-center rounded-sm border border-border-secondary text-white/60"
      title={title}
    >
      {children}
    </span>
  );
}

export default function ModelsPage() {
  const models = useModels();
  const toggle = useToggleModel();
  const refresh = useRefreshModels();
  const queryClient = useQueryClient();

  const [activeFilter, setActiveFilter] = useState<(typeof filters)[number]>("All");
  const [search, setSearch] = useState("");

  const rows = models.data?.models ?? [];
  const filtered = rows.filter((m) => {
    if (activeFilter === "Enabled" && !m.enabled) return false;
    if (activeFilter === "Free" && !m.free) return false;
    if (search) {
      const needle = search.toLowerCase();
      if (!m.name.toLowerCase().includes(needle) && !m.id.toLowerCase().includes(needle)) {
        return false;
      }
    }
    return true;
  });

  return (
    <div className="flex min-h-full flex-1 flex-col px-2">
      <div className="flex gap-6">
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="text-sm font-medium text-primary">Models</h2>
              <p className="mt-1 text-xs text-white/60">
                Only enabled models are available to members. Prices per 1M tokens unless noted.
              </p>
            </div>
            <div className="relative">
              <GoSearch className="pointer-events-none absolute top-1/2 left-2 size-3 -translate-y-1/2 text-white/40" />
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search"
                className="w-48 rounded-sm border border-border-secondary bg-tertiary py-1 pr-2 pl-7 text-xs text-primary placeholder:text-white/40 outline-none focus:border-white/30"
              />
            </div>
          </div>

          <div className="mt-4 flex gap-1">
            {filters.map((f) => (
              <button
                key={f}
                type="button"
                onClick={() => setActiveFilter(f)}
                className={`cursor-pointer rounded-sm px-2 py-1 text-xs transition-colors ${
                  activeFilter === f
                    ? "bg-white/10 font-medium text-primary"
                    : "text-white/60 hover:bg-white/10 hover:text-primary"
                }`}
              >
                {f}
              </button>
            ))}
          </div>

          <div className="mt-4 overflow-x-auto rounded-md border border-border-secondary">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-border-secondary bg-tertiary text-white/40">
                  <th className="rounded-tl-md px-2 py-2 font-medium">Model</th>
                  <th className="px-2 py-2 font-medium">Features</th>
                  <th className="px-2 py-2 font-medium">Input</th>
                  <th className="px-2 py-2 font-medium">Output</th>
                  <th className="px-2 py-2 font-medium">Cache Read</th>
                  <th className="px-2 py-2 font-medium">Cache Write</th>
                  <th className="rounded-tr-md px-2 py-2 font-medium">Enabled</th>
                </tr>
              </thead>
              <tbody>
                {models.isLoading && (
                  <tr>
                    <td colSpan={7} className="px-2 py-3 text-white/60">
                      Loading models…
                    </td>
                  </tr>
                )}
                {models.isError && (
                  <tr>
                    <td colSpan={7} className="px-2 py-3 text-white/60">
                      {models.error?.message ?? "Failed to load models"}
                    </td>
                  </tr>
                )}
                {!models.isLoading &&
                  !models.isError &&
                  filtered.length === 0 && (
                    <tr>
                      <td colSpan={7} className="px-2 py-3 text-white/60">
                        No models match.
                      </td>
                    </tr>
                  )}
                {filtered.map((m) => (
                  <Row
                    key={m.id}
                    model={m}
                    busy={toggle.isPending && toggle.variables?.modelId === m.id}
                    onToggle={() =>
                      toggle.mutate(
                        { modelId: m.id, enabled: !m.enabled },
                        {
                          onError: async () => {
                            await queryClient.invalidateQueries({ queryKey: ["models"] });
                          },
                        },
                      )
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <aside className="w-64 shrink-0">
          <div className="flex items-start justify-between gap-2">
            <div>
              <h3 className="text-sm font-medium text-primary">Argus Agent</h3>
              <p className="mt-0.5 text-xs text-white/60">
                {models.data ? `${capitalize(models.data.plan)} plan` : "-"}
              </p>
            </div>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => refresh.mutate()}
                disabled={refresh.isPending}
                className="flex size-6 cursor-pointer items-center justify-center rounded-sm border border-border-secondary bg-tertiary text-white/60 transition-colors hover:bg-white/10 hover:text-primary disabled:opacity-50"
                aria-label="Refresh catalog"
                title="Re-sync the catalog from our embedded catalog"
              >
                <GoSync className="size-3.5" />
              </button>
              <button
                type="button"
                className="flex size-6 cursor-pointer items-center justify-center rounded-sm border border-border-secondary bg-tertiary text-white/60 transition-colors hover:bg-white/10 hover:text-primary"
                aria-label="Menu"
              >
                <GoKebabHorizontal className="size-3.5" />
              </button>
            </div>
          </div>

          <div className="mt-4 border-t border-border-secondary" />

          <div className="mt-4 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs text-white/60">Published models</span>
              <span className="text-xs text-white/80">{rows.length}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-white/60">Enabled</span>
              <span className="text-xs text-white/80">
                {rows.filter((m) => m.enabled).length}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-white/60">Last updated</span>
              <span className="text-xs text-white/80">
                {refresh.isSuccess
                  ? "just now"
                  : relativeTime(models.dataUpdatedAt)}
              </span>
            </div>
            {refresh.isError && (
              <p className="text-xs text-red-400/80">
                {refresh.error?.message ?? "Catalog sync failed"}
              </p>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
}

function Row({
  model,
  busy,
  onToggle,
}: {
  model: ModelView;
  busy: boolean;
  onToggle: () => void;
}) {
  return (
    <tr className="border-b border-border-secondary/50">
      <td className="px-2 py-1">
        <div className="flex items-center gap-1.5">
          <span className="text-primary">{model.name}</span>
          {model.free && (
            <span className="rounded-sm bg-white/10 px-1 py-0.5 text-[10px] text-white/70">
              Free
            </span>
          )}
        </div>
        <div className="mt-0.5 text-[11px] text-white/40">{model.id}</div>
      </td>
      <td className="px-2 py-1">
        <div className="flex items-center gap-1">
          <FeatureChip title="Text">
            <RxText className="size-3.5" />
          </FeatureChip>
          {model.capabilities.vision && (
            <FeatureChip title="Image">
              <CiImageOn className="size-3.5" />
            </FeatureChip>
          )}
          {model.capabilities.tools && (
            <FeatureChip title="Tool use">
              <IoTerminalOutline className="size-3.5" />
            </FeatureChip>
          )}
          {model.capabilities.reasoning && (
            <FeatureChip title="Reasoning">
              <LuBrain className="size-3.5" />
            </FeatureChip>
          )}
        </div>
      </td>
      <td className="px-2 py-2 text-white/70">{price(model.pricing.input)}</td>
      <td className="px-2 py-2 text-white/70">{price(model.pricing.output)}</td>
      <td className="px-2 py-2 text-white/70">{price(model.pricing.cacheRead)}</td>
      <td className="px-2 py-2 text-white/70">{price(model.pricing.cacheWrite)}</td>
      <td className="px-2 py-1">
       <Toggle
                     on={model.enabled}
                     busy={busy}
                     disabled={!model.enabled}
                     label={`Toggle ${model.name}`}
                     onClick={onToggle}
                   />
      </td>
    </tr>
  );
}

function capitalize(text: string): string {
  if (text.length === 0) return text;
  return text[0].toUpperCase() + text.slice(1);
}
