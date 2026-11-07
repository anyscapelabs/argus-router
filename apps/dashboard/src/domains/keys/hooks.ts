"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createKey, listKeys, revokeKey, type CreateKeyResult } from "./api";

/**
 * Agent keys: what the desktop app pairs with. The BetterAuth session of the
 * browser creates, lists and revokes them; the desktop authenticates with
 * them.
 */
export function useApiKeys() {
  return useQuery({
    queryKey: ["api-keys"] as const,
    queryFn: listKeys,
  });
}

export function useCreateApiKey() {
  const qc = useQueryClient();
  return useMutation<CreateKeyResult, Error, string>({
    mutationFn: (name) => createKey(name),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["api-keys"] });
    },
  });
}

export function useRevokeApiKey() {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (keyId) => revokeKey(keyId),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["api-keys"] });
    },
  });
}
