"use client";

import { useEffect, useRef, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { makeQueryClient } from "./query-client";

/**
 * Single QueryClient per browser session — ref-kept so hot reloads and route
 * changes never re-create it mid-flight.
 */
export function QueryProvider({ children }: { children: React.ReactNode }) {
  const ref = useRef<ReturnType<typeof makeQueryClient> | undefined>(undefined);
  if (ref.current === undefined) {
    ref.current = makeQueryClient();
  }
  const [client] = useState(ref.current);

  useEffect(() => {
    return () => {
      client.clear();
    };
  }, [client]);

  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
