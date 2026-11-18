import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

export type BillingSubscription = {
  planId: string;
  status: string;
  nextBillingDate?: string;
  pastDueEndsAt?: string;
  cancelAtNextBillingDate?: boolean;
  cancelledAt?: string;
};

export type CreditPack = {
  id: string;
  label: string;
  priceUsd: number;
  creditsMicros: number;
  available: boolean;
};

export type BillingInvoice = {
  paymentId: string;
  createdAt: string;
  totalAmount: number;
  currency: string;
  status?: string;
  invoiceId?: string;
  description: string;
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
  | { planId: string; creditPackId?: never; amountUsd?: never }
  | { planId?: never; creditPackId: string; amountUsd?: never }
  | { planId?: never; creditPackId?: never; amountUsd: number };

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

export async function getInvoices(): Promise<{ invoices: BillingInvoice[] }> {
  return apiFetch<{ invoices: BillingInvoice[] }>(API_ROUTES.billingInvoices);
}

export function invoiceDownloadUrl(paymentId: string): string {
  return `${API_ROUTES.billingInvoices}/${paymentId}/download`;
}

export type PaymentMethod = {
  id: string;
  brand?: string;
  lastFour?: string;
  expiryMonth?: number;
  expiryYear?: number;
};

export async function getPaymentMethods(): Promise<{ paymentMethods: PaymentMethod[] }> {
  return apiFetch<{ paymentMethods: PaymentMethod[] }>(API_ROUTES.billingPaymentMethods);
}

export async function setupPaymentMethod(): Promise<{ url: string; mode: string }> {
  return apiFetch<{ url: string; mode: string }>(API_ROUTES.billingPaymentMethodSetup, {
    method: "POST",
  });
}

export type AutoRecharge = {
  enabled: boolean;
  amountCents: number;
  thresholdMicros: number;
  disabledByFailures: boolean;
};

export async function getAutoRecharge(): Promise<AutoRecharge> {
  return apiFetch<AutoRecharge>(API_ROUTES.billingAutoRecharge);
}

export async function updateAutoRecharge(request: {
  enabled: boolean;
  amountUsd?: number;
  thresholdUsd?: number;
}): Promise<AutoRecharge> {
  return apiFetch<AutoRecharge>(API_ROUTES.billingAutoRecharge, {
    method: "PUT",
    body: request,
  });
}
