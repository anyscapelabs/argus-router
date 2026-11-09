"use client";

import { FiExternalLink } from "react-icons/fi";

import { usePlans } from "@/domains/plans/hooks";
import { useModels } from "@/domains/models/hooks";
import { PlanCard, PlanCardSkeleton } from "@/components/plan-card";

export default function SubscriptionPage() {
  const plansQuery = usePlans();
  const modelsQuery = useModels();
  const plans = plansQuery.data?.plans ?? [];
  const currentPlanId = modelsQuery.data?.plan;

  return (
    <div className="flex min-h-full flex-1 flex-col px-2">
      <div className="flex gap-6">
        <div className="min-w-0 flex-1">
          <h2 className="text-sm font-medium text-primary">Subscription</h2>
          <p className="mt-1 text-xs font-medium text-white/60">
            Manage your plan and billing details.
          </p>
        </div>
        <button
          type="button"
          className="inline-flex h-fit shrink-0 cursor-pointer items-center gap-1 rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-white/10"
        >
          Learn more
          <FiExternalLink className="size-3" />
        </button>
      </div>

      {plansQuery.isError && (
        <p className="mt-2 text-xs text-white/60">
          {plansQuery.error?.message ?? "Failed to load plans"}
        </p>
      )}

      <div className="mt-2 grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-4">
        {plansQuery.isLoading &&
          Array.from({ length: 4 }).map((_, i) => <PlanCardSkeleton key={i} />)}
        {plans.map((plan) => (
          <PlanCard key={plan.id} plan={plan} isCurrent={plan.id === currentPlanId} />
        ))}
      </div>
    </div>
  );
}
