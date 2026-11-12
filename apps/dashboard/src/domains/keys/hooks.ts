"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createKey, listKeys, revokeKey, type CreateKeyResult } from "./api";

export function useApiKeys() {
  return useQuery({
    queryKey: ["api-keys"] as const,
    queryFn: listKeys,
  });
}

export function useCreateApiKey() {
  const qc = useQueryClient();
  return useMutation<CreateKeyResult, Error, { name: string; expiresIn?: number | null }>({
    mutationFn: ({ name, expiresIn }) => createKey(name, expiresIn),
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
