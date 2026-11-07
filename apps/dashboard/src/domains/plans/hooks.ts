"use client";

import { useQuery } from "@tanstack/react-query";
import { getPlans, type PlansResult } from "./api";

export function usePlans() {
  return useQuery<PlansResult, Error>({
    queryKey: ["plans"] as const,
    queryFn: getPlans,
  });
}