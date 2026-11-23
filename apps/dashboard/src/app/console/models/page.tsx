"use client";

import { Skeleton } from "@/components/skeleton";
import type { ModelTier } from "@/domains/models/api";
import { useModels } from "@/domains/models/hooks";

const GRID = "grid-cols-[1.8fr_0.8fr_0.8fr_0.8fr_1.4fr]";

const TIER_ORDER: Record<ModelTier, number> = {
  lite: 0,
  standard: 1,
  premium: 2,
  ultra: 3,
};

function baseName(name: string) {
  return name.replace(/\s*\((?:Bedrock|bedrock)\)\s*$/, "");
}

const COMPACT = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 1,
});

export default function ModelsPage() {
  const models = useModels();
  const allModels = models.data?.models ?? [];
  const nameCounts = allModels.reduce<Record<string, number>>((counts, model) => {
    const base = baseName(model.name ?? model.id);
    counts[base] = (counts[base] ?? 0) + 1;
    return counts;
  }, {});
  const rows = allModels
    .slice()
    .sort((a, b) => (TIER_ORDER[a.tier] ?? 99) - (TIER_ORDER[b.tier] ?? 99));

  return (
    <div className="mx-auto w-full max-w-5xl">
      <h1 className="text-2xl font-medium text-primary">Models</h1>
      <p className="mt-0.5 text-sm text-primary/60">
        The models you can use in Argus Agent and API
      </p>
      <div className="mt-6 pb-10">
        <div className={`grid ${GRID} gap-4 border-b border-border p-1 text-xs text-primary/60`}>
          <span>Model</span>
          <span>Context</span>
          <span>Max output</span>
          <span>Tier</span>
          <span>Features</span>
        </div>
        {models.isPending &&
          Array.from({ length: 5 }, (_, row) => (
            <div key={row} className={`grid ${GRID} gap-4 border-b border-border/50 p-1 py-3`}>
              {Array.from({ length: 5 }, (_, col) => (
                <Skeleton key={col} className="rounded-full px-2 py-1" />
              ))}
            </div>
          ))}
        {rows.map((model) => {
          const features = Object.entries(model.capabilities ?? {}).filter(([, value]) => value).map(([name]) => name);
          return (
            <div
              key={model.id}
              className={`grid ${GRID} gap-4 border-b border-border/50 p-1 py-3 text-sm transition-colors duration-200 hover:bg-tertiary`}
            >
              <span className="truncate text-primary">
                {(model.name ?? model.id) && nameCounts[baseName(model.name ?? model.id)] === 1
                  ? baseName(model.name ?? model.id)
                  : (model.name ?? model.id)}
              </span>
              <span className="tabular-nums text-primary/70">
                {model.context ? COMPACT.format(model.context) : "—"}
              </span>
              <span className="tabular-nums text-primary/70">
                {model.maxOutput ? COMPACT.format(model.maxOutput) : "—"}
              </span>
              <span className="truncate text-primary/70">{model.tier ?? "—"}</span>
              <span className="flex flex-wrap gap-1">
                {features.length === 0 && <span className="text-primary/60">—</span>}
                {features.map((feature) => (
                  <span
                    key={feature}
                    className="rounded-full bg-primary/10 px-2 py-0.5 text-xs capitalize text-primary/70"
                  >
                    {feature}
                  </span>
                ))}
              </span>
            </div>
          );
        })}
        {models.isSuccess && rows.length === 0 && (
          <p className="mt-6 text-center text-sm text-primary/60">No models yet.</p>
        )}
        {models.isError && <p className="mt-4 text-sm text-danger">{models.error.message}</p>}
      </div>
    </div>
  );
}
