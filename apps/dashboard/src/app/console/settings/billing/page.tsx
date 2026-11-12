"use client";

import { FaSpinner } from "react-icons/fa";

import { Skeleton } from "@/components/skeleton";
import {
  useBilling,
  useBillingPortal,
  useCheckout,
} from "@/domains/billing/hooks";
import { usePlans } from "@/domains/plans/hooks";

const USD = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 6,
});

const DATE = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "numeric",
  timeZone: "UTC",
});

const MANAGED_STATUSES = new Set([
  "pending",
  "active",
  "past_due",
  "on_hold",
  "paused",
]);

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
  const plans = usePlans();
  const checkout = useCheckout();
  const portal = useBillingPortal();

  const currentPlanId = billing.data?.planId;
  const currentPlan = plans.data?.plans.find((plan) => plan.id === currentPlanId);
  const subscription = billing.data?.subscription;
  const hasManagedSubscription = Boolean(
    subscription && MANAGED_STATUSES.has(subscription.status),
  );
  const renewalDate = formatDate(subscription?.nextBillingDate);
  const graceDate = formatDate(subscription?.pastDueEndsAt);
  const productsConfigured = Boolean(
    billing.data?.availablePlanIds.length ||
      billing.data?.creditPacks.some((pack) => pack.available),
  );

  return (
    <div className="flex min-h-full flex-1 flex-col px-2 pb-8">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-base font-medium text-primary">Billing</h1>
          <p className="mt-1 text-sm text-white/60">
            Manage your plan, prepaid credits, payment methods and invoices.
          </p>
        </div>
        {billing.data?.hasBillingCustomer && (
          <button
            type="button"
            onClick={() => portal.mutate()}
            disabled={portal.isPending}
            className="flex cursor-pointer items-center gap-1.5 rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5 text-xs font-medium text-white/80 transition-colors hover:bg-white/10 hover:text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {portal.isPending && <FaSpinner className="size-3 animate-spin" />}
            {portal.isPending ? "Opening…" : "Payment methods & invoices"}
          </button>
        )}
      </div>

      <div className="mt-4 rounded-md border border-border-secondary bg-[#262626] p-3 sm:p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-white/45">
              Current plan
            </p>
            {billing.isLoading || plans.isLoading ? (
              <Skeleton className="mt-2 h-7 w-28" />
            ) : (
              <div className="mt-1 flex items-center gap-2">
                <p className="text-2xl font-medium text-primary">
                  {currentPlan?.name ?? statusLabel(currentPlanId ?? "free")}
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
            <button
              type="button"
              onClick={() => document.getElementById("plans")?.scrollIntoView({ behavior: "smooth" })}
              className="cursor-pointer rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
            >
              View plans
            </button>
          )}
        </div>
      </div>

      {billing.isError && (
        <div className="mt-3 rounded-md border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-200/80">
          {billing.error.message}
        </div>
      )}

      {billing.data && (!billing.data.billingConfigured || !productsConfigured) && (
        <div className="mt-3 rounded-md border border-amber-300/15 bg-amber-300/5 px-3 py-2 text-xs text-amber-100/70">
          Checkout is not configured yet. Add the Dodo Payments API key and product IDs to
          enable purchases.
        </div>
      )}

      <section id="plans" className="mt-6 scroll-mt-6">
        <div className="flex items-end justify-between gap-3">
          <div>
            <h2 className="text-sm font-medium text-primary">Plans</h2>
            <p className="mt-1 text-xs text-white/50">
              Choose the model access and included usage that fits your workload.
            </p>
          </div>
        </div>

        {hasManagedSubscription ? (
          <div className="mt-2 flex flex-wrap items-center justify-between gap-3 rounded-md border border-border-secondary bg-[#262626] p-3">
            <p className="text-xs text-white/60">
              Upgrades, downgrades and cancellations are handled in the billing portal.
            </p>
            <button
              type="button"
              onClick={() => portal.mutate()}
              disabled={portal.isPending}
              className="cursor-pointer rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5 text-xs font-medium text-white/80 transition-colors hover:bg-white/10 disabled:opacity-50"
            >
              Open portal
            </button>
          </div>
        ) : (
          <div className="mt-2 grid gap-2 md:grid-cols-3">
            {plans.isLoading &&
              Array.from({ length: 3 }, (_, index) => (
                <Skeleton key={index} className="h-44 rounded-md" />
              ))}
            {plans.data?.plans
              .filter((plan) => plan.id !== "free")
              .map((plan) => {
                const available = billing.data?.availablePlanIds.includes(plan.id) ?? false;
                const pending =
                  checkout.isPending && checkout.variables?.planId === plan.id;
                return (
                  <div
                    key={plan.id}
                    className="flex min-h-44 flex-col rounded-md border border-border-secondary bg-[#262626] p-3"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <h3 className="text-sm font-medium text-primary">{plan.name}</h3>
                        <p className="mt-0.5 text-xs text-white/45">{plan.tagline}</p>
                      </div>
                      <p className="text-sm font-medium text-primary">
                        ${plan.price}
                        <span className="text-[10px] font-normal text-white/45">/mo</span>
                      </p>
                    </div>
                    <div className="mt-3 space-y-1 text-xs text-white/60">
                      <p>{plan.limits?.weeklyTokens ?? "Weekly token allowance"}</p>
                      <p>{plan.limits?.dailyRequests ?? "Daily request allowance"}</p>
                      <p>{plan.modelCount ?? `${plan.models.length} models`}</p>
                    </div>
                    <button
                      type="button"
                      onClick={() => checkout.mutate({ planId: plan.id })}
                      disabled={!available || checkout.isPending}
                      className="mt-auto flex cursor-pointer items-center justify-center gap-1.5 rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4] disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      {pending && <FaSpinner className="size-3 animate-spin" />}
                      {pending ? "Opening…" : available ? `Choose ${plan.name}` : "Unavailable"}
                    </button>
                  </div>
                );
              })}
          </div>
        )}
      </section>

      <section className="mt-6">
        <h2 className="text-sm font-medium text-primary">Prepaid credits</h2>
        <p className="mt-1 text-xs text-white/50">
          Credits are used automatically after your weekly or daily included usage is
          exhausted.
        </p>
        <div className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-3 sm:p-4">
          <div className="flex flex-wrap items-end justify-between gap-3 border-b border-border-secondary pb-3">
            <div>
              <p className="text-xs font-medium uppercase tracking-wide text-white/45">
                Available balance
              </p>
              {billing.isLoading ? (
                <Skeleton className="mt-2 h-7 w-24" />
              ) : (
                <p className="mt-1 text-2xl font-medium text-primary">
                  {USD.format((billing.data?.creditBalanceMicros ?? 0) / 1_000_000)}
                </p>
              )}
            </div>
            <p className="max-w-xs text-right text-[11px] leading-4 text-white/40">
              Charged from measured provider usage. Credits do not expire or reset.
            </p>
          </div>

          <div className="mt-3 grid gap-2 sm:grid-cols-2">
            {billing.isLoading &&
              Array.from({ length: 2 }, (_, index) => (
                <Skeleton key={index} className="h-16 rounded-md" />
              ))}
            {billing.data?.creditPacks.map((pack) => {
              const pending =
                checkout.isPending && checkout.variables?.creditPackId === pack.id;
              return (
                <button
                  key={pack.id}
                  type="button"
                  onClick={() => checkout.mutate({ creditPackId: pack.id })}
                  disabled={!pack.available || checkout.isPending}
                  className="flex cursor-pointer items-center justify-between gap-3 rounded-md border border-border-secondary bg-tertiary p-3 text-left transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <span>
                    <span className="block text-sm font-medium text-primary">{pack.label}</span>
                    <span className="mt-0.5 block text-[11px] text-white/45">One-time purchase</span>
                  </span>
                  <span className="flex items-center gap-1.5 text-xs font-medium text-white/80">
                    {pending && <FaSpinner className="size-3 animate-spin" />}
                    {pending ? "Opening…" : USD.format(pack.priceUsd)}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </section>
    </div>
  );
}
