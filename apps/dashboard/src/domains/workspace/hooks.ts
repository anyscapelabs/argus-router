"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

const WORKSPACE_KEY = ["workspace"] as const;

/**
 * The workspace the signed-in account belongs to. The API provisions a
 * "Default" one on first read, so a fresh account gets the same shape as an
 * established one — `WorkspaceAvatar` renders `avatarUrl`, with `initials` as
 * the fallback when the remote image cannot load.
 */
export type Workspace = {
  id: string;
  name: string;
  slug: string;
  /** The caller's role in this workspace. */
  role: string;
  initials: string;
  avatarUrl: string;
};

/**
 * `GET /api/v1/workspace` — session-protected; a 401 rejects as `ApiError`
 * status 401 (no retry). Pass `{ enabled: false }` to hold the request until a
 * session is known, matching `useProfile`.
 */
export function useWorkspace({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: WORKSPACE_KEY,
    queryFn: () => apiFetch<Workspace>(API_ROUTES.workspace),
    enabled,
  });
}

/**
 * `PATCH /api/v1/workspace` — the blur-save target for the General settings
 * field. The API answers with the full workspace (name, initials and avatar
 * already recomputed), so the cache is written with that response instead of
 * invalidated: the top bar changes on the same frame the field settles, with
 * no second request.
 */
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
