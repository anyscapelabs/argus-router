import { createAuthClient } from "better-auth/client";

import { API_ROUTES } from "@/config/api";

export const authClient = createAuthClient({
  baseURL: API_ROUTES.auth,
  fetchOptions: {
    credentials: "include",
  },
});

export function dashboardPath(path: string) {
  return `${window.location.origin}${path}`;
}
