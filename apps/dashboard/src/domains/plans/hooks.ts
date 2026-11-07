"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getPlans, refreshPlans, type PlansResult } from "./api";

export function usePlans() {
  return useQuery<PlansResult, Error>({
    queryKey: ["plans"] as const,
    queryFn: getPlans,
  });
}

export function useRefreshPlans() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: refreshPlans,
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["plans"] });
      await qc.invalidateQueries({ queryKey: ["models"] });
    },
  });
}