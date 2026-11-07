import { QueryClient } from "@tanstack/react-query";
import { ApiError } from "@/api/http-client";

/**
 * Default staleness keeps the versioned API calls predictable:
 * session/profile stay fresh per navigation window, agents revalidate on focus.
 */
export function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        gcTime: 5 * 60_000,
        refetchOnWindowFocus: true,
        retry: (attempt, error) => {
          if (error instanceof ApiError) {
            if (error.status === 401) return false;
            return error.status >= 500 && attempt < 3;
          }
          return attempt < 3;
        },
      },
      mutations: {
        retry: false,
      },
    },
  });
}
