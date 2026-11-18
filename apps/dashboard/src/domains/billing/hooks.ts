"use client";

import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";

import {
  createBillingPortal,
  createCheckout,
  getAutoRecharge,
  getBilling,
  getInvoices,
  getPaymentMethods,
  setupPaymentMethod,
  updateAutoRecharge,
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

const PAYMENT_METHODS_KEY = ["billing", "payment-methods"] as const;
const AUTO_RECHARGE_KEY = ["billing", "auto-recharge"] as const;

export function usePaymentMethods(enabled = true) {
  return useQuery({
    queryKey: PAYMENT_METHODS_KEY,
    queryFn: getPaymentMethods,
    enabled,
  });
}

export function usePaymentMethodSetup() {
  return useMutation({
    mutationFn: setupPaymentMethod,
    onSuccess: ({ url }) => window.open(url, "_blank", "noopener,noreferrer"),
    onError: (error: Error) => toast.error(error.message),
  });
}

export function useAutoRecharge(enabled = true) {
  return useQuery({
    queryKey: AUTO_RECHARGE_KEY,
    queryFn: getAutoRecharge,
    enabled,
  });
}

export function useUpdateAutoRecharge() {
  return useMutation({
    mutationFn: updateAutoRecharge,
    onSuccess: (updated) => {
      toast.success(
        updated.enabled
          ? "Auto recharge is on."
          : "Auto recharge is off.",
      );
    },
    onError: (error: Error) => toast.error(error.message),
  });
}
