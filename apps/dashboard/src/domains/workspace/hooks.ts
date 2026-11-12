"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

const WORKSPACE_KEY = ["workspace"] as const;

export type Workspace = {
  id: string;
  name: string;
  slug: string;
  role: string;
  initials: string;
  avatarUrl: string;
};

export function useWorkspace({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: WORKSPACE_KEY,
    queryFn: () => apiFetch<Workspace>(API_ROUTES.workspace),
    enabled,
  });
}

export function useRenameWorkspace() {
  const qc = useQueryClient();
  return useMutation<Workspace, Error, string>({
    mutationFn: (name) =>
      apiFetch<Workspace>(API_ROUTES.workspace, {
        method: "PATCH",
        body: { name },
      }),
    onSuccess: (workspace) => {
      qc.setQueryData(WORKSPACE_KEY, workspace);
      toast.success("Workspace name saved");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}
