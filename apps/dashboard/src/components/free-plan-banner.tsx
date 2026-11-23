"use client";

import { useBilling, useCheckout } from "@/domains/billing/hooks";

export function FreePlanBanner() {
  const billing = useBilling();
  const checkout = useCheckout();
  const planId = billing.data?.planId;
  if (!billing.isSuccess || (planId !== "free" && planId !== "starter")) return null;

  return (
    <div className="flex w-full flex-col gap-4 rounded-xl bg-secondary p-4">
      <div>
        <p className="text-base font-medium text-primary/80">Get more usage with Max</p>
        <p className="mt-2 text-sm text-primary/60">
          Max raises your usage limits in Argus Agent
        </p>
      </div>
      <button
        type="button"
        onClick={() => checkout.mutate({ planId: "max" })}
        disabled={checkout.isPending}
        className="self-start rounded-full bg-accent px-4 py-2 text-sm font-medium text-background transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {checkout.isPending ? "Opening checkout…" : "Upgrade to Max"}
      </button>
    </div>
  );
}
