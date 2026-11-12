import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

export type BillingSubscription = {
  planId: string;
  status: string;
  nextBillingDate?: string;
  pastDueEndsAt?: string;
};

export type CreditPack = {
  id: string;
  label: string;
  priceUsd: number;
  creditsMicros: number;
  available: boolean;
};

export type BillingSummary = {
  planId: string;
  subscription?: BillingSubscription | null;
  creditBalanceMicros: number;
  creditPacks: CreditPack[];
  availablePlanIds: string[];
  billingConfigured: boolean;
  hasBillingCustomer: boolean;
};

export type CheckoutTarget =
  | { planId: string; creditPackId?: never }
  | { planId?: never; creditPackId: string };

export async function getBilling(): Promise<BillingSummary> {
  return apiFetch<BillingSummary>(API_ROUTES.billing);
}

export async function createCheckout(
  target: CheckoutTarget,
): Promise<{ checkoutUrl: string }> {
  return apiFetch<{ checkoutUrl: string }>(API_ROUTES.billingCheckout, {
    method: "POST",
    body: target,
  });
}

export async function createBillingPortal(): Promise<{ portalUrl: string }> {
  return apiFetch<{ portalUrl: string }>(API_ROUTES.billingPortal, {
    method: "POST",
  });
}
