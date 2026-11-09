"use client";

import { GoSync } from "react-icons/go";
import { useModels } from "@/domains/models/hooks";
import { usePlans, useRefreshPlans } from "@/domains/plans/hooks";

export default function BillingPage() {
  const models = useModels();
  const plans = usePlans();
  const refresh = useRefreshPlans();
  const currentPlan = models.data?.plan ?? "free";

  return (
    <div className="flex min-h-full flex-1 flex-col px-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-primary">Billing &amp; Plans</h2>
        <button
          type="button"
          onClick={() => refresh.mutate()}
          disabled={refresh.isPending}
          className="flex size-6 cursor-pointer items-center justify-center rounded-sm border border-border-secondary bg-tertiary text-white/60 transition-colors hover:bg-white/10 hover:text-primary disabled:opacity-50"
          aria-label="Refresh plans"
          title="Re-seed plans from JSON"
        >
          <GoSync className="size-3.5" />
        </button>
      </div>
      <p className="mt-1 text-xs text-white/60">
        Choose the plan that fits your usage. Prices are per month.
      </p>
      <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {plans.isLoading && (
          <p className="text-xs text-white/60">Loading plans…</p>
        )}
        {plans.data?.plans.map((plan) => {
          const isCurrent = plan.id === currentPlan;
          return (
            <div
              key={plan.id}
              className={`rounded-md border p-3 ${
                isCurrent ? "border-[#3b82f6] bg-white/5" : "border-border-secondary bg-tertiary"
              }`}
            >
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-medium text-primary">{plan.name}</h3>
                {isCurrent && (
                  <span className="rounded-sm bg-[#3b82f6]/20 px-1.5 py-0.5 text-[10px] text-[#3b82f6]">
                    Current
                  </span>
                )}
              </div>
              <p className="mt-1 text-lg font-semibold text-primary">
                ${plan.price}
                <span className="text-xs font-normal text-white/40">/mo</span>
              </p>
              <p className="mt-2 text-xs text-white/60">
                {plan.models.length} models included
              </p>
              <p className="mt-1 text-xs text-emerald-400">
                Live Voice is available for plan
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}