import { API_ROUTES } from "@/config/api";

export type Plan = {
  id: string;
  name: string;
  price: string;
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