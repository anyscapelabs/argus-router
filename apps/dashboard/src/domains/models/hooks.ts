"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  getModels,
  refreshModels,
  toggleModelAllowlist,
  type ModelsResult,
} from "./api";

/**
 * The signed-in owner's plan and its model catalog (enablement included).
 * The models page renders straight from this; toggles and the catalog
 * refresh both invalidate it.
 */
export function useModels() {
  return useQuery<ModelsResult, Error>({
    queryKey: ["models"] as const,
    queryFn: getModels,
  });
}

export function useToggleModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ modelId, enabled }: { modelId: string; enabled: boolean }) =>
      toggleModelAllowlist(modelId, enabled),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["models"] });
    },
  });
}

export function useRefreshModels() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: refreshModels,
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["models"] });
    },
  });
}
