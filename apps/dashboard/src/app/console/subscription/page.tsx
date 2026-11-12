"use client";

import { FiExternalLink } from "react-icons/fi";

import { PlanCard, PlanCardSkeleton } from "@/components/plan-card";
import { useBilling, useBillingPortal, useCheckout } from "@/domains/billing/hooks";
import { useModels } from "@/domains/models/hooks";
import { usePlans } from "@/domains/plans/hooks";

const MANAGED_STATUSES = new Set([
  "pending",
  "active",
  "past_due",
  "on_hold",
  "paused",
]);

export default function SubscriptionPage() {
  const plansQuery = usePlans();
  const modelsQuery = useModels();
  const billing = useBilling();
  const checkout = useCheckout();
  const portal = useBillingPortal();

  const plans = plansQuery.data?.plans ?? [];
  const currentPlanId = modelsQuery.data?.plan ?? billing.data?.planId;
  const subscription = billing.data?.subscription;
  const hasManagedSubscription = Boolean(
    subscription && MANAGED_STATUSES.has(subscription.status),
  );
  const availablePlanIds = billing.data?.availablePlanIds ?? [];

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
        {plans.map((plan) => {
          const isCurrent = plan.id === currentPlanId;
          const canCheckout =
            plan.id !== "free" && !hasManagedSubscription && availablePlanIds.includes(plan.id);
          const canManage = Boolean(billing.data?.hasBillingCustomer);

          let onAction: (() => void) | undefined;
          let actionLabel = isCurrent ? "Manage" : "Upgrade";
          let disabled = false;
          let pending = false;

          if (isCurrent) {
            onAction = canManage ? () => portal.mutate() : undefined;
            disabled = !canManage;
            pending = portal.isPending && canManage;
          } else if (hasManagedSubscription) {
            actionLabel = "Manage plan";
            onAction = canManage ? () => portal.mutate() : undefined;
            disabled = !canManage;
            pending = portal.isPending && canManage;
          } else if (plan.id === "free") {
            disabled = true;
          } else {
            onAction = canCheckout ? () => checkout.mutate({ planId: plan.id }) : undefined;
            disabled = !canCheckout;
            pending = checkout.isPending && checkout.variables?.planId === plan.id;
          }

          return (
            <PlanCard
              key={plan.id}
              plan={plan}
              isCurrent={isCurrent}
              onAction={onAction}
              actionLabel={actionLabel}
              pending={pending}
              disabled={disabled}
            />
          );
        })}
      </div>
    </div>
  );
}
