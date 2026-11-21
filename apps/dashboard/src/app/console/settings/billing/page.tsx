"use client";

import { FaSpinner } from "react-icons/fa";

import { Skeleton } from "@/components/skeleton";
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

const DATE = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "numeric",
  timeZone: "UTC",
});

function formatDate(value?: string) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : DATE.format(date);
}

function statusLabel(value: string) {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export default function BillingPage() {
  const billing = useBilling();
  const models = useModels();
  const plansQuery = usePlans();
  const checkout = useCheckout();
  const portal = useBillingPortal();

  const plans = plansQuery.data?.plans ?? [];
  const currentPlanId = models.data?.plan ?? billing.data?.planId ?? "free";
  const currentPlan = plans.find((plan) => plan.id === currentPlanId);
  const subscription = billing.data?.subscription;
  const hasManagedSubscription = Boolean(
    subscription && MANAGED_STATUSES.has(subscription.status),
  );
  const canManage = Boolean(billing.data?.hasBillingCustomer);
  const renewalDate = formatDate(subscription?.nextBillingDate);
  const graceDate = formatDate(subscription?.pastDueEndsAt);
  const cancelScheduled = Boolean(subscription?.cancelAtNextBillingDate && renewalDate);
  const availablePlanIds = billing.data?.availablePlanIds ?? [];

  return (
    <div className="flex min-h-full flex-1 flex-col px-2 pb-8">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-base font-medium text-primary">Billing</h1>
          <p className="mt-1 text-sm text-white/60">
            Manage your plan and subscription.
          </p>
        </div>
        {canManage && (
          <button
            type="button"
            onClick={() => portal.mutate()}
            disabled={portal.isPending}
            className="flex cursor-pointer items-center gap-1.5 rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5 text-xs font-medium text-white/80 transition-colors hover:bg-white/10 hover:text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {portal.isPending && <FaSpinner className="size-3 animate-spin" />}
            {portal.isPending ? "Opening…" : "Manage subscription"}
          </button>
        )}
      </div>

      <div className="mt-4 rounded-md border border-border-secondary bg-[#262626] p-3 sm:p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-white/45">
              Current plan
            </p>
            {billing.isLoading || plansQuery.isLoading ? (
              <Skeleton className="mt-2 h-7 w-28" />
            ) : (
              <div className="mt-1 flex items-center gap-2">
                <p className="text-2xl font-medium text-primary">
                  {currentPlan?.name ?? statusLabel(currentPlanId)}
                </p>
                {subscription && (
                  <span className="rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] font-medium text-white/60">
                    {statusLabel(subscription.status)}
                  </span>
                )}
              </div>
            )}
            {subscription?.status === "past_due" && graceDate ? (
              <p className="mt-1 text-xs text-amber-300/80">
                Payment is past due. Access continues through {graceDate}.
              </p>
            ) : cancelScheduled ? (
              <p className="mt-1 text-xs text-amber-300/80">
                Cancellation scheduled for {renewalDate}. Access continues until then.
              </p>
            ) : renewalDate && hasManagedSubscription ? (
              <p className="mt-1 text-xs text-white/50">Next billing date: {renewalDate}</p>
            ) : (
              <p className="mt-1 text-xs text-white/50">
                Included usage resets on the plan schedule.
              </p>
            )}
          </div>
          {hasManagedSubscription ? (
            <button
              type="button"
              onClick={() => portal.mutate()}
              disabled={portal.isPending}
              className="flex cursor-pointer items-center gap-1.5 rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4] disabled:cursor-not-allowed disabled:opacity-50"
            >
              {portal.isPending && <FaSpinner className="size-3 animate-spin" />}
              Manage plan
            </button>
          ) : (
            <span className="text-xs text-white/50">Plans below</span>
          )}
        </div>
      </div>

      {(billing.isError || plansQuery.isError) && (
        <div className="mt-3 rounded-md border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-200/80">
          {billing.error?.message ?? plansQuery.error?.message}
        </div>
      )}

      <div className="mt-6 grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-4">
        {plansQuery.isLoading &&
          Array.from({ length: 4 }).map((_, i) => <PlanCardSkeleton key={i} />)}
        {plans.map((plan) => {
          const isCurrent = plan.id === currentPlanId;
          const canCheckout =
            plan.id !== "free" && !hasManagedSubscription && availablePlanIds.includes(plan.id);

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
