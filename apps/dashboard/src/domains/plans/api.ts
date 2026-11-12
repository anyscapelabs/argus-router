import { API_ROUTES } from "@/config/api";

export type PlanLimits = {
  monthlyTokens: string;
  weeklyTokens: string;
  dailyRequests: string;
  ratePerMin: string;
};

export type Plan = {
  id: string;
  name: string;
  price: string;
  tagline?: string;
  limits?: PlanLimits;
  modelNames?: string[];
  modelCount?: string;
  models: string[];
};

export type PlansResult = {
  plans: Plan[];
};

export async function getPlans(): Promise<PlansResult> {
  const response = await fetch(API_ROUTES.plans, {
    method: "GET",
    credentials: "include",
  });
  if (!response.ok) throw new Error("Failed to load plans");
  return response.json() as Promise<PlansResult>;
}

export async function refreshPlans(): Promise<{ refreshed: boolean; count: number }> {
  const response = await fetch(`${API_ROUTES.plans}/refresh`, {
    method: "POST",
    credentials: "include",
  });
  if (!response.ok) throw new Error("Failed to refresh plans");
  return response.json() as Promise<{ refreshed: boolean; count: number }>;
}
