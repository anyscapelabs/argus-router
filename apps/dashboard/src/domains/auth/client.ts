import { createAuthClient } from "better-auth/client";

import { API_ROUTES } from "@/config/api";

export const authClient = createAuthClient({
  baseURL: API_ROUTES.auth,
  fetchOptions: {
    credentials: "include",
  },
});

/**
 * Absolute redirect target for post-OAuth landings — the Rust API builds its
 * `Location` headers against the API origin, so social sign-in needs the
 * dashboard's full URL.
 */
export function dashboardPath(path: string) {
  return `${window.location.origin}${path}`;
}
