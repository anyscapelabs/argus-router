"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { getSettings, updateSettings, type SettingsPatch, type UserSettings } from "./api";

const SETTINGS_KEY = ["settings"] as const;

export function useSettings({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: getSettings,
    enabled,
  });
}

export function useUpdateSettings() {
  const qc = useQueryClient();
  return useMutation<UserSettings, Error, SettingsPatch, { previous?: UserSettings }>({
    mutationFn: (patch) => updateSettings(patch),
    onMutate: async (patch) => {
      await qc.cancelQueries({ queryKey: SETTINGS_KEY });
      const previous = qc.getQueryData<UserSettings>(SETTINGS_KEY);
      if (previous) {
        qc.setQueryData<UserSettings>(SETTINGS_KEY, { ...previous, ...patch });
      }
      return { previous };
    },
    onSuccess: (settings) => {
      qc.setQueryData(SETTINGS_KEY, settings);
    },
    onError: (error, _patch, context) => {
      if (context?.previous) {
        qc.setQueryData(SETTINGS_KEY, context.previous);
      }
      toast.error(error.message);
    },
  });
}
