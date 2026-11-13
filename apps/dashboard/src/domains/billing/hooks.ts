"use client";

import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";

import {
  createBillingPortal,
  createCheckout,
  getBilling,
  getInvoices,
  type CheckoutTarget,
} from "./api";

const BILLING_KEY = ["billing"] as const;

export function useBilling() {
  return useQuery({
    queryKey: BILLING_KEY,
    queryFn: getBilling,
  });
}

export function useInvoices(enabled: boolean) {
  return useQuery({
    queryKey: [...BILLING_KEY, "invoices"],
    queryFn: getInvoices,
    enabled,
  });
}

export function useCheckout() {
  return useMutation<{ checkoutUrl: string }, Error, CheckoutTarget>({
    mutationFn: createCheckout,
    onSuccess: ({ checkoutUrl }) => window.location.assign(checkoutUrl),
    onError: (error) => toast.error(error.message),
  });
}

export function useBillingPortal() {
  return useMutation({
    mutationFn: createBillingPortal,
    onSuccess: ({ portalUrl }) => window.location.assign(portalUrl),
    onError: (error: Error) => toast.error(error.message),
  });
}
