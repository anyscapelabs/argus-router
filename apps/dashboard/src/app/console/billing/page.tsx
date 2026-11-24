"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState, type ReactNode } from "react";
import { GoCreditCard, GoGear, GoHistory, GoInfo } from "react-icons/go";
import { UpgradePlansModal } from "@/components/upgrade-plans-modal";
import { useBilling, useBillingPortal } from "@/domains/billing/hooks";
import { usePlans } from "@/domains/plans/hooks";

const TABS = ["Overview", "Invoice"] as const;
type Tab = (typeof TABS)[number];

function MenuButton({
  icon,
  title,
  description,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="flex w-full max-w-sm cursor-pointer items-center gap-3 rounded-xl border border-border-secondary p-3 text-left transition-colors hover:bg-tertiary disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent"
    >
      <span className="text-primary/60">{icon}</span>
      <span className="min-w-0">
        <span className="block text-sm font-medium text-primary">{title}</span>
        <span className="block text-xs text-primary/60">{description}</span>
      </span>
    </button>
  );
}

export default function BillingPage() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const [tab, setTab] = useState<Tab>("Overview");
  const [upgradeOpen, setUpgradeOpen] = useState(false);
  const portal = useBillingPortal();
  const billing = useBilling();
  const plans = usePlans();
  const planName = plans.data?.plans.find((plan) => plan.id === billing.data?.planId)?.name ?? "Free";
  const hasPaidPlan =
    billing.isSuccess && !!billing.data?.planId && billing.data.planId !== "free";

  useEffect(() => {
    if (searchParams.get("upgrade") === "1") {
      setUpgradeOpen(true);
      const params = new URLSearchParams(searchParams.toString());
      params.delete("upgrade");
      const qs = params.toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    }
  }, [searchParams, pathname, router]);

  return (
    <div className="mx-auto w-full max-w-5xl">
      <h1 className="text-2xl font-medium text-primary">Billing and Invoices</h1>
      <div className="mt-4 flex gap-2">
        {TABS.map((item) => (
          <button
            key={item}
            type="button"
            onClick={() => setTab(item)}
            className={`cursor-pointer rounded-full px-2 py-1 text-sm font-medium transition-colors ${
              tab === item
                ? "bg-tertiary text-primary"
                : "text-primary/60 hover:text-primary"
            }`}
          >
            {item}
          </button>
        ))}
      </div>

      {tab === "Overview" && (
        <div className="mt-6 flex flex-col gap-4">
          <div className="min-h-40 w-full rounded-xl bg-secondary p-4">
            <div className="flex items-center justify-between gap-4">
              <p className="text-xs font-medium text-primary/80">Current Plan</p>
              <Link
                href="/console/usage"
                className="cursor-pointer rounded-full border border-border-secondary px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/10"
              >
                View Usage and spending
              </Link>
            </div>
            <p className="mt-2 text-2xl font-medium text-primary">{planName}</p>
            <div className="mt-4 flex gap-2">
              <button
                type="button"
                onClick={() => setUpgradeOpen(true)}
                className="cursor-pointer rounded-full bg-accent px-3 py-1.5 text-xs font-medium text-background transition-opacity hover:opacity-80"
              >
                Upgrade Plan
              </button>
              {hasPaidPlan && (
                <button
                  type="button"
                  onClick={() => portal.mutate()}
                  disabled={portal.isPending}
                  className="cursor-pointer rounded-full bg-tertiary px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {portal.isPending ? "Opening…" : "Cancel plan"}
                </button>
              )}
            </div>
          </div>
          <div className="grid w-fit grid-cols-1 gap-2 sm:grid-cols-2">
          <MenuButton
            icon={<GoGear className="size-5" aria-hidden="true" />}
            title="Preferences"
            disabled={portal.isPending}
            description="Manage your billing preferences"
            onClick={() => portal.mutate()}
          />
          <MenuButton
            icon={<GoInfo className="size-5" aria-hidden="true" />}
            title="Manage billing information"
            disabled={portal.isPending}
            description="Update your billing details"
            onClick={() => portal.mutate()}
          />
          <MenuButton
            icon={<GoHistory className="size-5" aria-hidden="true" />}
            title="Billing history"
            description="View past invoices and payments"
            onClick={() => setTab("Invoice")}
          />
          <MenuButton
            icon={<GoCreditCard className="size-5" aria-hidden="true" />}
            title="Payment Methods"
            disabled={portal.isPending}
            description="add or change payment method"
            onClick={() => portal.mutate()}
          />
        </div>
        </div>
      )}

      {tab === "Invoice" && (
        <div className="mt-6 flex w-full flex-col gap-3 rounded-xl bg-secondary p-4">
          <p className="text-sm font-medium text-primary/80">Invoices</p>
          <p className="text-xs text-primary/60">
            Invoices and payment receipts are managed in the billing portal.
          </p>
          <button
            type="button"
            onClick={() => portal.mutate()}
            disabled={portal.isPending}
            className="self-start rounded-full bg-accent px-2 py-1 text-sm font-medium text-background transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {portal.isPending ? "Opening…" : "Open billing portal"}
          </button>
        </div>
      )}
      <UpgradePlansModal
        open={upgradeOpen}
        onClose={() => setUpgradeOpen(false)}
        currentPlanId={billing.data?.planId ?? "free"}
        availablePlanIds={billing.data?.availablePlanIds ?? []}
      />
    </div>
  );
}
