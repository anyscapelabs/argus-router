"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { FaDownload, FaSpinner } from "react-icons/fa";

import { Skeleton } from "@/components/skeleton";
import { EmptyState } from "@/components/empty-state";
import { invoiceDownloadUrl } from "@/domains/billing/api";
import {
  useBilling,
  useBillingPortal,
  useCheckout,
  useInvoices,
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

function formatMoney(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat("en-US", { style: "currency", currency }).format(
      amount / 100,
    );
  } catch {
    return `${(amount / 100).toFixed(2)} ${currency}`;
  }
}

const MIN_CUSTOM_AMOUNT = 1;
const MAX_CUSTOM_AMOUNT = 500;

export default function BillingPage() {
  const billing = useBilling();
  const plans = usePlans();
  const portal = useBillingPortal();
  const checkout = useCheckout();
  const invoices = useInvoices(Boolean(billing.data?.billingConfigured));
  const [buyOpen, setBuyOpen] = useState(false);
  const [selectedPackId, setSelectedPackId] = useState<string | null>(null);
  const [customAmount, setCustomAmount] = useState("");

  const currentPlanId = billing.data?.planId ?? "free";
  const currentPlan = plans.data?.plans.find((plan) => plan.id === currentPlanId);
  const subscription = billing.data?.subscription;
  const hasManagedSubscription = Boolean(
    subscription && MANAGED_STATUSES.has(subscription.status),
  );
  const renewalDate = formatDate(subscription?.nextBillingDate);
  const graceDate = formatDate(subscription?.pastDueEndsAt);
  const cancelScheduled = Boolean(subscription?.cancelAtNextBillingDate && renewalDate);
  const planActionLabel =
    currentPlanId === "free" || currentPlanId === "starter" ? "Upgrade" : "View plans";
  const creditPacks = billing.data?.creditPacks ?? [];
  const selectedPack = creditPacks.find((pack) => pack.id === selectedPackId);
  const customAmountValue = Number(customAmount);
  const customAmountValid =
    Number.isFinite(customAmountValue) &&
    customAmountValue >= MIN_CUSTOM_AMOUNT &&
    customAmountValue <= MAX_CUSTOM_AMOUNT;
  const canCheckout = Boolean(
    !checkout.isPending && (selectedPack?.available || customAmountValid),
  );

  const onContinueCheckout = () => {
    if (selectedPack?.available) {
      checkout.mutate({ creditPackId: selectedPack.id });
      return;
    }
    checkout.mutate({ amountUsd: Math.round(customAmountValue * 100) / 100 });
  };

  useEffect(() => {
    if (!buyOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !checkout.isPending) setBuyOpen(false);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [buyOpen, checkout.isPending]);

  const selectPack = (packId: string, priceUsd: number) => {
    setSelectedPackId(packId);
    setCustomAmount(String(priceUsd));
  };

  const onCustomAmountChange = (value: string) => {
    setCustomAmount(value);
    const amount = Number(value);
    const match = creditPacks.find((pack) => pack.priceUsd === amount);
    setSelectedPackId(match ? match.id : null);
  };

  return (
    <div className="flex min-h-full flex-1 flex-col px-2 pb-8">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-base font-medium text-primary">Billing</h1>
          <p className="mt-1 text-sm text-white/60">
            Manage your plan, prepaid credits, payment methods and invoices.
          </p>
        </div>
        {billing.data?.billingConfigured && (
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
            <Link
              href="/console/subscription"
              className="cursor-pointer rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
            >
              {planActionLabel}
            </Link>
          )}
        </div>
      </div>

      {billing.isError && (
        <div className="mt-3 rounded-md border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-200/80">
          {billing.error.message}
        </div>
      )}

      <section className="mt-6">
        <h2 className="text-sm font-medium text-primary">Credits</h2>
        <p className="mt-1 text-xs text-white/50">Used after included plan usage.</p>
        <div className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-3 sm:p-4">
          <div className="flex flex-wrap items-end justify-between gap-3">
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
            <button
              type="button"
              onClick={() => {
                setSelectedPackId(null);
                setCustomAmount("");
                setBuyOpen(true);
              }}
              className="cursor-pointer rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
            >
              Buy credits
            </button>
          </div>
        </div>
      </section>

      {billing.data?.billingConfigured && (
        <section className="mt-6">
          <h2 className="text-sm font-medium text-primary">Invoices</h2>
          <p className="mt-1 text-xs text-white/50">Payments made on this account.</p>
          <div className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-3 sm:p-4">
            {invoices.isLoading ? (
              <div className="space-y-2">
                {Array.from({ length: 3 }, (_, index) => (
                  <Skeleton key={index} className="h-9 w-full" />
                ))}
              </div>
            ) : invoices.isError ? (
              <p className="text-xs text-red-200/80">{invoices.error.message}</p>
            ) : invoices.data && invoices.data.invoices.length > 0 ? (
              <ul className="divide-y divide-white/5">
                {invoices.data.invoices.map((invoice) => (
                  <li
                    key={invoice.paymentId}
                    className="flex flex-wrap items-center justify-between gap-2 py-2 first:pt-0 last:pb-0"
                  >
                    <div>
                      <p className="text-sm text-primary">{invoice.description}</p>
                      <p className="mt-0.5 text-[11px] text-white/45">
                        {formatDate(invoice.createdAt) ?? invoice.createdAt}
                      </p>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-xs font-medium text-white/80">
                        {formatMoney(invoice.totalAmount, invoice.currency)}
                      </span>
                      {invoice.invoiceId && (
                        <a
                          href={invoiceDownloadUrl(invoice.paymentId)}
                          className="flex items-center gap-1 rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-white/80 transition-colors hover:bg-white/10 hover:text-primary"
                        >
                          <FaDownload className="size-3" />
                          Download
                        </a>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              <EmptyState className="py-6" message="No payments yet" />
            )}
          </div>
        </section>
      )}

      {buyOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => {
            if (!checkout.isPending) setBuyOpen(false);
          }}
          role="presentation"
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Buy credits"
            className="w-full max-w-md rounded-md border border-border-secondary bg-[#262626] p-4 shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-sm font-medium text-primary">Buy credits</h2>
            <p className="mt-1 text-xs text-white/50">Select an amount to add to your balance.</p>

            <div className="mt-4">
              <label className="text-xs font-medium uppercase tracking-wide text-white/45">
                Custom amount
              </label>
              <div className="mt-1 flex items-center rounded-sm border border-border-secondary bg-tertiary px-2.5 py-1.5">
                <span className="text-sm text-white/50">$</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  inputMode="decimal"
                  value={customAmount}
                  onChange={(e) => onCustomAmountChange(e.target.value)}
                  placeholder="0"
                  className="ml-1 w-full bg-transparent text-sm text-primary outline-none placeholder:text-white/30"
                />
              </div>
            </div>

            <div className="mt-4">
              <p className="text-xs font-medium uppercase tracking-wide text-white/45">Amounts</p>
              <div className="mt-2 grid gap-2 sm:grid-cols-2">
                {billing.isLoading &&
                  Array.from({ length: 2 }, (_, index) => (
                    <Skeleton key={index} className="h-16 rounded-md" />
                  ))}
                {!billing.isLoading &&
                  creditPacks.map((pack) => {
                    const selected = selectedPackId === pack.id;
                    return (
                      <button
                        key={pack.id}
                        type="button"
                        onClick={() => selectPack(pack.id, pack.priceUsd)}
                        disabled={!pack.available}
                        className={`flex cursor-pointer items-center justify-between gap-3 rounded-md border p-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                          selected
                            ? "border-accent bg-accent/10"
                            : "border-border-secondary bg-tertiary hover:bg-white/10"
                        }`}
                      >
                        <span>
                          <span className="block text-sm font-medium text-primary">{pack.label}</span>
                          <span className="mt-0.5 block text-[11px] text-white/45">One-time purchase</span>
                        </span>
                        <span className="text-xs font-medium text-white/80">
                          {USD.format(pack.priceUsd)}
                        </span>
                      </button>
                    );
                  })}
                {!billing.isLoading && creditPacks.length === 0 && (
                  <p className="text-xs text-white/50">No credit amounts available.</p>
                )}
              </div>
              {customAmount && !selectedPackId && !customAmountValid && (
                <p className="mt-2 text-[11px] text-white/45">
                  Enter an amount between {USD.format(MIN_CUSTOM_AMOUNT)} and{" "}
                  {USD.format(MAX_CUSTOM_AMOUNT)}.
                </p>
              )}
              {customAmount && !selectedPackId && customAmountValid && (
                <p className="mt-2 text-[11px] text-white/45">
                  Continuing with a custom amount.
                </p>
              )}
            </div>

            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => {
                  if (!checkout.isPending) setBuyOpen(false);
                }}
                disabled={checkout.isPending}
                className="cursor-pointer rounded-sm px-2 py-1 text-xs text-white/60 transition-colors hover:text-primary disabled:cursor-not-allowed disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={onContinueCheckout}
                disabled={!canCheckout}
                className="inline-flex cursor-pointer items-center justify-center gap-1.5 rounded-sm bg-accent px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4] disabled:cursor-not-allowed disabled:opacity-50"
              >
                {checkout.isPending && <FaSpinner className="size-3 animate-spin" />}
                {checkout.isPending ? "Opening…" : "Continue"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
