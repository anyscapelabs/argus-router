import { createAuthClient } from "better-auth/client";
import { magicLinkClient } from "better-auth/client/plugins";

import { API_ROUTES } from "@/config/api";

export const authClient = createAuthClient({
  baseURL: API_ROUTES.auth,
  plugins: [magicLinkClient()],
  fetchOptions: {
    credentials: "include",
  },
});

export function dashboardPath(path: string) {
  return `${window.location.origin}${path}`;
}
